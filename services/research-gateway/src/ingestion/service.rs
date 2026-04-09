use crate::ingestion::artifact_discovery::{
    ArtifactDiscoveryManifest, ArtifactSourceKind, discover_artifact_sources,
};
use crate::ingestion::artifact_parser::parse_markdown_artifact;
use crate::ingestion::snapshot_builder::{SnapshotAssembly, SnapshotBuildInput, build_snapshot_assembly};
use domain::audit_artifacts::{CanonicalSnapshot, build_canonical_snapshot, canonical_requirement_id};
use persistence::postgres::canonical_artifacts::{
    insert_conflict as pg_insert_conflict, insert_equivalence as pg_insert_equivalence,
    upsert_item as pg_upsert_item, upsert_snapshot as pg_upsert_snapshot,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionMode {
    FullSnapshot,
    Delta,
}

#[derive(Debug, Clone)]
pub struct RunCanonicalIngestionInput {
    pub commit_sha: String,
    pub ingested_at_utc: String,
    pub repo_root: String,
    pub mode: IngestionMode,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CanonicalIngestionError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct CanonicalIngestionResult {
    pub commit_sha: String,
    pub ingested_at_utc: String,
    pub snapshot_digest: String,
    pub counts_by_artifact_type: BTreeMap<String, usize>,
    pub item_count: usize,
    pub unresolved_conflict_count: usize,
    pub warning_count: usize,
    pub canonical_requirement_ids: Vec<String>,
    pub unresolved_conflicts: Vec<CanonicalIngestionConflict>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CanonicalIngestionConflict {
    pub conflict_id: String,
    pub status: String,
    pub left_canonical_requirement_id: String,
    pub right_canonical_requirement_id: String,
    pub rationale: String,
}

pub trait CanonicalSnapshotPersistencePort: Send + Sync {
    fn persist_snapshot<'a>(
        &'a self,
        snapshot: &'a CanonicalSnapshot,
        assembly: &'a SnapshotAssembly,
    ) -> Pin<Box<dyn Future<Output = Result<(), CanonicalIngestionError>> + Send + 'a>>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryCanonicalSnapshotPersistence {
    snapshots: Arc<Mutex<Vec<CanonicalSnapshot>>>,
}

impl InMemoryCanonicalSnapshotPersistence {
    #[cfg(test)]
    pub fn snapshot_count(&self) -> usize {
        self.snapshots
            .lock()
            .expect("snapshot mutex should not be poisoned")
            .len()
    }
}

impl CanonicalSnapshotPersistencePort for InMemoryCanonicalSnapshotPersistence {
    fn persist_snapshot<'a>(
        &'a self,
        snapshot: &'a CanonicalSnapshot,
        _assembly: &'a SnapshotAssembly,
    ) -> Pin<Box<dyn Future<Output = Result<(), CanonicalIngestionError>> + Send + 'a>> {
        Box::pin(async move {
            self.snapshots
                .lock()
                .expect("snapshot mutex should not be poisoned")
                .push(snapshot.clone());
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct PostgresCanonicalSnapshotPersistence {
    pool: PgPool,
}

impl PostgresCanonicalSnapshotPersistence {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl CanonicalSnapshotPersistencePort for PostgresCanonicalSnapshotPersistence {
    fn persist_snapshot<'a>(
        &'a self,
        snapshot: &'a CanonicalSnapshot,
        assembly: &'a SnapshotAssembly,
    ) -> Pin<Box<dyn Future<Output = Result<(), CanonicalIngestionError>> + Send + 'a>> {
        Box::pin(async move {
            let mut transaction = self.pool.begin().await.map_err(|error| CanonicalIngestionError {
                code: "canonical_artifact_query_failed".to_string(),
                message: format!("unable to open canonical ingestion transaction: {error}"),
            })?;
            pg_upsert_snapshot(&mut *transaction, snapshot)
                .await
                .map_err(map_persistence_error)?;

            let snapshot_id = snapshot_id(snapshot)?;
            for item in &snapshot.items {
                pg_upsert_item(&mut *transaction, &snapshot_id, item)
                    .await
                    .map_err(map_persistence_error)?;
            }

            for equivalence in &assembly.equivalence_records {
                pg_insert_equivalence(
                    &mut *transaction,
                    &equivalence.equivalence_id,
                    &snapshot_id,
                    &equivalence.left_canonical_requirement_id,
                    &equivalence.right_canonical_requirement_id,
                    "semantic equivalence",
                )
                .await
                .map_err(map_persistence_error)?;
            }

            for conflict in &assembly.unresolved_conflicts {
                pg_insert_conflict(
                    &mut *transaction,
                    &conflict.conflict_id,
                    &snapshot_id,
                    &conflict.left_canonical_requirement_id,
                    &conflict.right_canonical_requirement_id,
                    &conflict.rationale,
                )
                .await
                .map_err(map_persistence_error)?;
            }

            transaction
                .commit()
                .await
                .map_err(|error| CanonicalIngestionError {
                    code: "canonical_artifact_query_failed".to_string(),
                    message: format!("unable to commit canonical ingestion transaction: {error}"),
                })?;
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct CanonicalIngestionService {
    persistence: Arc<dyn CanonicalSnapshotPersistencePort>,
}

impl CanonicalIngestionService {
    pub fn new(persistence: Arc<dyn CanonicalSnapshotPersistencePort>) -> Self {
        Self { persistence }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryCanonicalSnapshotPersistence::default()))
    }

    pub async fn run_canonical_ingestion(
        &self,
        input: RunCanonicalIngestionInput,
    ) -> Result<CanonicalIngestionResult, CanonicalIngestionError> {
        if !matches!(input.mode, IngestionMode::FullSnapshot) {
            return Err(CanonicalIngestionError {
                code: "canonical_ingestion_mode_unsupported".to_string(),
                message: "only full-snapshot ingestion is supported".to_string(),
            });
        }
        validate_commit_sha(&input.commit_sha)?;
        validate_ingested_at_utc(&input.ingested_at_utc)?;
        let repo_root = validate_repo_root(&input.repo_root)?;

        let manifest = discover_artifact_sources(&repo_root);
        let counts = counts_by_artifact_type(&manifest);
        require_required_artifact_classes(&counts)?;
        let (file_digests, parsed_artifacts) = parse_sources(&repo_root, &manifest)?;
        let aggregate_digest = aggregate_digest(&file_digests);

        let assembly = build_snapshot_assembly(SnapshotBuildInput {
            commit_sha: input.commit_sha.clone(),
            ingested_at_utc: input.ingested_at_utc.clone(),
            file_digests,
            aggregate_digest,
            parsed_artifacts,
        });
        if !assembly.successful {
            let failure = assembly.findings.first().ok_or_else(|| CanonicalIngestionError {
                code: "canonical_artifact_invalid_payload".to_string(),
                message: "snapshot assembly failed without a diagnostic".to_string(),
            })?;
            return Err(CanonicalIngestionError {
                code: failure.code.clone(),
                message: failure.message.clone(),
            });
        }
        let snapshot_input = assembly
            .snapshot_input
            .as_ref()
            .ok_or_else(|| CanonicalIngestionError {
                code: "canonical_artifact_invalid_payload".to_string(),
                message: "snapshot assembly missing snapshot input".to_string(),
            })?;
        let snapshot = build_canonical_snapshot(snapshot_input).map_err(|error| CanonicalIngestionError {
            code: "canonical_artifact_invalid_payload".to_string(),
            message: error.message,
        })?;

        self.persistence.persist_snapshot(&snapshot, &assembly).await?;
        Ok(CanonicalIngestionResult {
            commit_sha: snapshot.commit_sha,
            ingested_at_utc: snapshot.ingested_at_utc,
            snapshot_digest: snapshot.aggregate_digest,
            counts_by_artifact_type: counts,
            item_count: snapshot.items.len(),
            unresolved_conflict_count: assembly.unresolved_conflicts.len(),
            warning_count: snapshot.warnings.len(),
            canonical_requirement_ids: snapshot
                .items
                .iter()
                .map(|item| item.canonical_requirement_id.clone())
                .collect(),
            unresolved_conflicts: assembly
                .unresolved_conflicts
                .iter()
                .map(|conflict| CanonicalIngestionConflict {
                    conflict_id: conflict.conflict_id.clone(),
                    status: conflict.status.clone(),
                    left_canonical_requirement_id: conflict.left_canonical_requirement_id.clone(),
                    right_canonical_requirement_id: conflict.right_canonical_requirement_id.clone(),
                    rationale: conflict.rationale.clone(),
                })
                .collect(),
        })
    }
}

pub async fn run_canonical_ingestion(
    input: RunCanonicalIngestionInput,
) -> Result<CanonicalIngestionResult, CanonicalIngestionError> {
    CanonicalIngestionService::in_memory()
        .run_canonical_ingestion(input)
        .await
}

pub fn counts_by_artifact_type(manifest: &ArtifactDiscoveryManifest) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for source in &manifest.sources {
        let key = match source.kind {
            ArtifactSourceKind::Prd => "prd",
            ArtifactSourceKind::Architecture => "architecture",
            ArtifactSourceKind::Story => "story",
            ArtifactSourceKind::Roadmap => "roadmap",
        };
        *counts.entry(key.to_string()).or_insert(0) += 1;
    }
    counts
}

fn validate_commit_sha(commit_sha: &str) -> Result<(), CanonicalIngestionError> {
    let trimmed = commit_sha.trim();
    let valid = trimmed.len() >= 6
        && trimmed.len() <= 64
        && trimmed.chars().all(|ch| ch.is_ascii_hexdigit());
    if !valid {
        return Err(CanonicalIngestionError {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message: "commit_sha must be 6-64 hex characters".to_string(),
        });
    }
    Ok(())
}

fn validate_ingested_at_utc(value: &str) -> Result<(), CanonicalIngestionError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| CanonicalIngestionError {
        code: "canonical_ingestion_invalid_payload".to_string(),
        message: "ingested_at_utc must be RFC3339 UTC".to_string(),
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(CanonicalIngestionError {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message: "ingested_at_utc must use Z offset".to_string(),
        });
    }
    Ok(())
}

fn validate_repo_root(repo_root: &str) -> Result<PathBuf, CanonicalIngestionError> {
    let root = PathBuf::from(repo_root);
    if !root.exists() || !root.is_dir() {
        return Err(CanonicalIngestionError {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message: "repo_root must reference an existing directory".to_string(),
        });
    }
    Ok(root)
}

fn require_required_artifact_classes(
    counts: &BTreeMap<String, usize>,
) -> Result<(), CanonicalIngestionError> {
    for required in ["prd", "architecture", "story", "roadmap"] {
        if counts.get(required).copied().unwrap_or(0) == 0 {
            return Err(CanonicalIngestionError {
                code: "canonical_artifact_invalid_payload".to_string(),
                message: format!("missing required artifact class: {required}"),
            });
        }
    }
    Ok(())
}

fn parse_sources(
    repo_root: &Path,
    manifest: &ArtifactDiscoveryManifest,
) -> Result<(BTreeMap<String, String>, Vec<crate::ingestion::artifact_parser::ParsedArtifact>), CanonicalIngestionError>
{
    let mut file_digests = BTreeMap::new();
    let mut parsed_artifacts = Vec::new();
    for source in &manifest.sources {
        let absolute_path = repo_root.join(&source.relative_path);
        let markdown = std::fs::read_to_string(&absolute_path).map_err(|error| CanonicalIngestionError {
            code: "canonical_artifact_query_failed".to_string(),
            message: format!(
                "unable to read artifact `{}` from repo root: {error}",
                source.relative_path
            ),
        })?;
        file_digests.insert(source.relative_path.clone(), sha256_digest(markdown.as_bytes()));
        let parsed = parse_markdown_artifact(source, &markdown).map_err(|error| CanonicalIngestionError {
            code: error.code.to_string(),
            message: error.message,
        })?;
        parsed_artifacts.push(parsed);
    }
    Ok((file_digests, parsed_artifacts))
}

fn aggregate_digest(file_digests: &BTreeMap<String, String>) -> String {
    let mut serialized = String::new();
    for (path, digest) in file_digests {
        serialized.push_str(path);
        serialized.push(':');
        serialized.push_str(digest);
        serialized.push('\n');
    }
    sha256_digest(serialized.as_bytes())
}

fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> Result<String, CanonicalIngestionError> {
    canonical_requirement_id(
        &snapshot.commit_sha,
        &snapshot.aggregate_digest,
        snapshot.items.len() as u32 + 1,
    )
    .map_err(|error| CanonicalIngestionError {
        code: "canonical_artifact_invalid_payload".to_string(),
        message: error.message,
    })
}

fn map_persistence_error(
    error: persistence::postgres::canonical_artifacts::CanonicalArtifactPersistenceError,
) -> CanonicalIngestionError {
    CanonicalIngestionError {
        code: error.code.to_string(),
        message: error.message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::audit_artifacts::{CanonicalArtifactInput, CanonicalArtifactType, CanonicalSnapshotInput, build_canonical_snapshot};
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_workspace() -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ingestion-service-{stamp}"));
        fs::create_dir_all(&dir).expect("temp root should be created");
        dir
    }

    fn write_markdown(root: &Path, relative: &str, body: &str) {
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("parent dirs should be created");
        }
        fs::write(target, body).expect("file should be written");
    }

    #[tokio::test]
    async fn run_canonical_ingestion_ingests_all_required_artifact_types() {
        let workspace = test_workspace();
        write_markdown(
            &workspace,
            ".planning/PRD.md",
            "# PRD\n- [ ] REQ-1 ingest canonical artifacts",
        );
        write_markdown(
            &workspace,
            "docs/architecture.md",
            "# Architecture\n- [ ] ARCH-1 keep IDs stable",
        );
        write_markdown(
            &workspace,
            ".planning/stories/story-1.md",
            "# Story\n- [ ] ST-1 deterministic replay",
        );
        write_markdown(
            &workspace,
            ".planning/ROADMAP.md",
            "# Roadmap\n- [ ] RM-1 include metadata",
        );

        let output = run_canonical_ingestion(RunCanonicalIngestionInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: workspace.to_string_lossy().to_string(),
            mode: IngestionMode::FullSnapshot,
        })
        .await
        .expect("ingestion should succeed for valid snapshot input");

        assert_eq!(output.commit_sha, "abc123");
        assert_eq!(output.ingested_at_utc, "2026-04-09T00:00:00Z");
        assert!(!output.snapshot_digest.is_empty());
        assert_eq!(output.counts_by_artifact_type.get("prd"), Some(&1));
        assert_eq!(output.counts_by_artifact_type.get("architecture"), Some(&1));
        assert_eq!(output.counts_by_artifact_type.get("story"), Some(&1));
        assert_eq!(output.counts_by_artifact_type.get("roadmap"), Some(&1));
        assert!(output.item_count >= 4);
    }

    #[tokio::test]
    async fn run_canonical_ingestion_returns_machine_readable_error_for_schema_failures() {
        let workspace = test_workspace();
        write_markdown(
            &workspace,
            ".planning/PRD.md",
            "# PRD\n| ID | Acceptance |\n| --- |\n| ACC-1 | malformed row |",
        );

        let error = run_canonical_ingestion(RunCanonicalIngestionInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: workspace.to_string_lossy().to_string(),
            mode: IngestionMode::FullSnapshot,
        })
        .await
        .expect_err("schema failures must fail closed");

        assert_eq!(error.code, "canonical_artifact_invalid_payload");
    }

    #[tokio::test]
    async fn run_canonical_ingestion_rejects_non_full_snapshot_mode() {
        let workspace = test_workspace();
        write_markdown(
            &workspace,
            ".planning/PRD.md",
            "# PRD\n- [ ] REQ-1 ingest canonical artifacts",
        );
        write_markdown(
            &workspace,
            "docs/architecture.md",
            "# Architecture\n- [ ] ARCH-1 keep IDs stable",
        );
        write_markdown(
            &workspace,
            ".planning/stories/story-1.md",
            "# Story\n- [ ] ST-1 deterministic replay",
        );
        write_markdown(
            &workspace,
            ".planning/ROADMAP.md",
            "# Roadmap\n- [ ] RM-1 include metadata",
        );

        let error = run_canonical_ingestion(RunCanonicalIngestionInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: workspace.to_string_lossy().to_string(),
            mode: IngestionMode::Delta,
        })
        .await
        .expect_err("delta mode should be rejected in v1");

        assert_eq!(error.code, "canonical_ingestion_mode_unsupported");
    }

    #[tokio::test]
    async fn run_canonical_ingestion_is_deterministic_for_identical_inputs() {
        let workspace = test_workspace();
        write_markdown(
            &workspace,
            ".planning/PRD.md",
            "# PRD\n- [ ] REQ-1 ingest canonical artifacts",
        );
        write_markdown(
            &workspace,
            "docs/architecture.md",
            "# Architecture\n- [ ] ARCH-1 keep IDs stable",
        );
        write_markdown(
            &workspace,
            ".planning/stories/story-1.md",
            "# Story\n- [ ] ST-1 deterministic replay",
        );
        write_markdown(
            &workspace,
            ".planning/ROADMAP.md",
            "# Roadmap\n- [ ] RM-1 include metadata",
        );

        let first = run_canonical_ingestion(RunCanonicalIngestionInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: workspace.to_string_lossy().to_string(),
            mode: IngestionMode::FullSnapshot,
        })
        .await
        .expect("first run should pass");
        let second = run_canonical_ingestion(RunCanonicalIngestionInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: workspace.to_string_lossy().to_string(),
            mode: IngestionMode::FullSnapshot,
        })
        .await
        .expect("second run should pass");

        assert_eq!(first.snapshot_digest, second.snapshot_digest);
    }

    #[test]
    fn snapshot_identity_changes_when_ingested_at_differs() {
        let base_input = CanonicalSnapshotInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            file_digests: BTreeMap::from([(
                ".planning/PROJECT.md".to_string(),
                "digest-a".to_string(),
            )]),
            aggregate_digest: "digest-all".to_string(),
            artifacts: vec![CanonicalArtifactInput {
                artifact_type: CanonicalArtifactType::Prd,
                artifact_path: ".planning/PROJECT.md".to_string(),
                heading_slug: "requirements".to_string(),
                item_index: 1,
                source_item_id: "REQ-1".to_string(),
                body: "Traceability baseline".to_string(),
            }],
        };
        let later_input = CanonicalSnapshotInput {
            ingested_at_utc: "2026-04-09T00:00:01Z".to_string(),
            ..base_input.clone()
        };
        let base_snapshot = build_canonical_snapshot(&base_input).expect("snapshot should build");
        let later_snapshot = build_canonical_snapshot(&later_input).expect("snapshot should build");

        let base_id = snapshot_id(&base_snapshot).expect("snapshot id should compute");
        let later_id = snapshot_id(&later_snapshot).expect("snapshot id should compute");

        assert_ne!(base_id, later_id);
    }
}
