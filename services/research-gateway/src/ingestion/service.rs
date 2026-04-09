use crate::ingestion::artifact_discovery::{ArtifactDiscoveryManifest, ArtifactSourceKind};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionMode {
    FullSnapshot,
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
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct CanonicalIngestionResult {
    pub commit_sha: String,
    pub ingested_at_utc: String,
    pub snapshot_digest: String,
    pub counts_by_artifact_type: BTreeMap<String, usize>,
}

pub async fn run_canonical_ingestion(
    _input: RunCanonicalIngestionInput,
) -> Result<CanonicalIngestionResult, CanonicalIngestionError> {
    Err(CanonicalIngestionError {
        code: "canonical_ingestion_not_implemented",
        message: "run_canonical_ingestion not implemented".to_string(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
