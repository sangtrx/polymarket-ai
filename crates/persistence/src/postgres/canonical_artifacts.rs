use domain::audit_artifacts::{
    CanonicalArtifactContractError, CanonicalArtifactItem, CanonicalSnapshot,
    canonical_requirement_id,
};
use sqlx::PgExecutor;
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_SNAPSHOT_SQL: &str = r#"
    INSERT INTO canonical_ingestion_snapshots (
        snapshot_id,
        commit_sha,
        ingested_at_utc,
        file_digests_json,
        aggregate_digest
    ) VALUES ($1, $2, $3::timestamptz, $4::jsonb, $5)
"#;

const UPSERT_ITEM_SQL: &str = r#"
    INSERT INTO canonical_artifact_items (
        canonical_requirement_id,
        snapshot_id,
        artifact_type,
        artifact_path,
        heading_slug,
        item_index,
        source_item_id,
        body
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    ON CONFLICT (canonical_requirement_id)
    DO UPDATE SET
        snapshot_id = EXCLUDED.snapshot_id,
        source_item_id = EXCLUDED.source_item_id,
        body = EXCLUDED.body
"#;

const INSERT_EQUIVALENCE_SQL: &str = r#"
    INSERT INTO canonical_item_equivalences (
        equivalence_id,
        snapshot_id,
        left_canonical_requirement_id,
        right_canonical_requirement_id,
        rationale
    ) VALUES ($1, $2, $3, $4, $5)
"#;

const INSERT_CONFLICT_SQL: &str = r#"
    INSERT INTO canonical_item_conflicts (
        conflict_id,
        snapshot_id,
        left_canonical_requirement_id,
        right_canonical_requirement_id,
        status,
        rationale
    ) VALUES ($1, $2, $3, $4, 'unresolved', $5)
"#;

const LIST_SNAPSHOTS_BY_COMMIT_SQL: &str = r#"
    SELECT snapshot_id, commit_sha, to_char(ingested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS ingested_at_utc
    FROM canonical_ingestion_snapshots
    WHERE lower(trim(commit_sha)) = lower(trim($1))
    ORDER BY ingested_at_utc DESC, snapshot_id ASC
    LIMIT $2
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalArtifactPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl CanonicalArtifactPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "canonical_artifact_invalid_payload",
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "canonical_artifact_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "canonical_artifact_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }
}

impl Display for CanonicalArtifactPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for CanonicalArtifactPersistenceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSnapshotRow {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub ingested_at_utc: String,
}

pub async fn insert_snapshot<'e, E>(
    executor: E,
    snapshot: &CanonicalSnapshot,
) -> Result<(), CanonicalArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    let snapshot_id = snapshot_id(snapshot).map_err(map_contract_error)?;
    let file_digests_json = serde_json::to_string(&snapshot.file_digests).map_err(|error| {
        CanonicalArtifactPersistenceError::invalid_payload(format!(
            "unable to serialize file digests: {error}"
        ))
    })?;

    sqlx::query(INSERT_SNAPSHOT_SQL)
        .bind(&snapshot_id)
        .bind(&snapshot.commit_sha)
        .bind(&snapshot.ingested_at_utc)
        .bind(file_digests_json)
        .bind(&snapshot.aggregate_digest)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_snapshot", error))?;

    Ok(())
}

pub async fn upsert_item<'e, E>(
    executor: E,
    snapshot_id: &str,
    item: &CanonicalArtifactItem,
) -> Result<(), CanonicalArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    let computed_canonical_requirement_id =
        canonical_requirement_id(&item.artifact_path, &item.heading_slug, item.item_index)
            .map_err(map_contract_error)?;
    if computed_canonical_requirement_id != item.canonical_requirement_id {
        return Err(CanonicalArtifactPersistenceError::invalid_payload(
            "canonical_requirement_id must match artifact path + heading slug + item index anchor",
        ));
    }

    sqlx::query(UPSERT_ITEM_SQL)
        .bind(&item.canonical_requirement_id)
        .bind(snapshot_id)
        .bind(item.artifact_type.as_str())
        .bind(&item.artifact_path)
        .bind(&item.heading_slug)
        .bind(i64::from(item.item_index))
        .bind(item.source_item_id.clone())
        .bind(&item.body)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_item", error))?;

    Ok(())
}

pub async fn insert_equivalence<'e, E>(
    executor: E,
    equivalence_id: &str,
    snapshot_id: &str,
    left_canonical_requirement_id: &str,
    right_canonical_requirement_id: &str,
    rationale: &str,
) -> Result<(), CanonicalArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    sqlx::query(INSERT_EQUIVALENCE_SQL)
        .bind(equivalence_id.trim().to_ascii_lowercase())
        .bind(snapshot_id)
        .bind(left_canonical_requirement_id)
        .bind(right_canonical_requirement_id)
        .bind(rationale)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_equivalence", error))?;
    Ok(())
}

pub async fn insert_conflict<'e, E>(
    executor: E,
    conflict_id: &str,
    snapshot_id: &str,
    left_canonical_requirement_id: &str,
    right_canonical_requirement_id: &str,
    rationale: &str,
) -> Result<(), CanonicalArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    sqlx::query(INSERT_CONFLICT_SQL)
        .bind(conflict_id.trim().to_ascii_lowercase())
        .bind(snapshot_id)
        .bind(left_canonical_requirement_id)
        .bind(right_canonical_requirement_id)
        .bind(rationale)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_conflict", error))?;
    Ok(())
}

pub async fn list_snapshots_by_commit<'e, E>(
    executor: E,
    commit_sha: &str,
    limit: i64,
) -> Result<Vec<CanonicalSnapshotRow>, CanonicalArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    if commit_sha.trim().is_empty() {
        return Err(CanonicalArtifactPersistenceError::invalid_payload(
            "commit_sha cannot be blank",
        ));
    }
    if limit <= 0 {
        return Err(CanonicalArtifactPersistenceError::invalid_payload(
            "limit must be greater than 0",
        ));
    }

    let rows = sqlx::query(LIST_SNAPSHOTS_BY_COMMIT_SQL)
        .bind(commit_sha.trim().to_ascii_lowercase())
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| classify_query_error("list_snapshots_by_commit", error))?;

    rows.into_iter()
        .map(|row| {
            Ok(CanonicalSnapshotRow {
                snapshot_id: sqlx::Row::try_get(&row, "snapshot_id").map_err(|error| {
                    CanonicalArtifactPersistenceError::query_failure(
                        "list_snapshots_by_commit.row_decode_snapshot_id",
                        error,
                    )
                })?,
                commit_sha: sqlx::Row::try_get(&row, "commit_sha").map_err(|error| {
                    CanonicalArtifactPersistenceError::query_failure(
                        "list_snapshots_by_commit.row_decode_commit_sha",
                        error,
                    )
                })?,
                ingested_at_utc: sqlx::Row::try_get(&row, "ingested_at_utc").map_err(|error| {
                    CanonicalArtifactPersistenceError::query_failure(
                        "list_snapshots_by_commit.row_decode_ingested_at_utc",
                        error,
                    )
                })?,
            })
        })
        .collect()
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> Result<String, CanonicalArtifactContractError> {
    canonical_requirement_id(
        &snapshot.commit_sha,
        &format!("{}-{}", snapshot.aggregate_digest, snapshot.ingested_at_utc),
        snapshot.items.len() as u32 + 1,
    )
}

fn map_contract_error(error: CanonicalArtifactContractError) -> CanonicalArtifactPersistenceError {
    CanonicalArtifactPersistenceError::invalid_payload(error.message)
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> CanonicalArtifactPersistenceError {
    if is_constraint_error(&error) {
        return CanonicalArtifactPersistenceError::constraint_violation(operation, error);
    }
    CanonicalArtifactPersistenceError::query_failure(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .map(|code| code.starts_with("23") || code == "55000")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::audit_artifacts::{
        CanonicalArtifactInput, CanonicalArtifactType, CanonicalSnapshotInput,
        build_canonical_snapshot,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    const CANONICAL_INGESTION_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260409000100_canonical_artifact_ingestion.sql");

    #[test]
    fn migration_contract_snapshot_unique_key() {
        assert!(CANONICAL_INGESTION_MIGRATION_SQL.contains("canonical_ingestion_snapshots"));
        assert!(
            CANONICAL_INGESTION_MIGRATION_SQL.contains("UNIQUE (commit_sha, ingested_at_utc)")
        );
    }

    #[test]
    fn migration_contract_item_guardrails() {
        assert!(CANONICAL_INGESTION_MIGRATION_SQL.contains("canonical_artifact_items"));
        assert!(CANONICAL_INGESTION_MIGRATION_SQL.contains("canonical_requirement_id"));
        assert!(CANONICAL_INGESTION_MIGRATION_SQL.contains("artifact_type IN"));
    }

    #[test]
    fn migration_contract_conflict_records_are_unresolved() {
        assert!(CANONICAL_INGESTION_MIGRATION_SQL.contains("canonical_item_conflicts"));
        assert!(CANONICAL_INGESTION_MIGRATION_SQL.contains("status = 'unresolved'"));
    }

    #[test]
    fn migration_contract_snapshot_immutability_guard_exists() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/20260409000200_canonical_snapshot_immutability.sql");
        let sql = std::fs::read_to_string(path).expect("immutability migration must exist");
        assert!(sql.contains("canonical_ingestion_snapshots"));
        assert!(sql.contains("TRIGGER") || sql.contains("RAISE EXCEPTION"));
    }

    #[test]
    fn migration_contract_snapshot_immutability_protects_metadata_columns() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/20260409000200_canonical_snapshot_immutability.sql");
        let sql = std::fs::read_to_string(path).expect("immutability migration must exist");
        assert!(sql.contains("commit_sha"));
        assert!(sql.contains("ingested_at_utc"));
        assert!(sql.contains("file_digests_json"));
        assert!(sql.contains("aggregate_digest"));
    }

    #[test]
    fn list_snapshots_query_orders_records_deterministically() {
        assert!(
            LIST_SNAPSHOTS_BY_COMMIT_SQL.contains("ORDER BY ingested_at_utc DESC, snapshot_id ASC")
        );
    }

    #[test]
    fn conflict_and_equivalence_records_persist_independently() {
        assert!(INSERT_EQUIVALENCE_SQL.contains("canonical_item_equivalences"));
        assert!(INSERT_CONFLICT_SQL.contains("canonical_item_conflicts"));
        assert!(!INSERT_EQUIVALENCE_SQL.contains("canonical_item_conflicts"));
        assert!(!INSERT_CONFLICT_SQL.contains("canonical_item_equivalences"));
    }

    #[test]
    fn db_errors_map_to_machine_readable_codes() {
        let error = classify_query_error(
            "list_snapshots_by_commit",
            sqlx::Error::Protocol("boom".into()),
        );
        assert_eq!(error.code, "canonical_artifact_query_failed");
        assert!(error.message.contains("list_snapshots_by_commit"));
    }

    #[test]
    fn snapshot_id_is_deterministic_for_replay() {
        let input = CanonicalSnapshotInput {
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
        let snapshot = build_canonical_snapshot(&input).expect("snapshot should build");
        let first = snapshot_id(&snapshot).expect("snapshot id should compute");
        let second = snapshot_id(&snapshot).expect("snapshot id should compute");
        assert_eq!(first, second);
        assert!(first.contains('.'));
    }

    #[test]
    fn snapshot_metadata_immutable_sql_is_insert_only() {
        assert!(INSERT_SNAPSHOT_SQL.contains("INSERT INTO canonical_ingestion_snapshots"));
        assert!(!INSERT_SNAPSHOT_SQL.contains("DO UPDATE SET"));
    }

    #[test]
    fn snapshot_metadata_immutable_conflict_uses_machine_readable_code() {
        let error = CanonicalArtifactPersistenceError::constraint_violation(
            "insert_snapshot",
            sqlx::Error::Protocol("conflict".into()),
        );
        assert_eq!(error.code, "canonical_artifact_constraint_violation");
        assert!(error.message.contains("insert_snapshot"));
    }

    #[test]
    fn upsert_item_rejects_id_anchor_drift() {
        let item = CanonicalArtifactItem {
            canonical_requirement_id: "wrong.id.1".to_string(),
            artifact_type: CanonicalArtifactType::Prd,
            artifact_path: ".planning/PROJECT.md".to_string(),
            heading_slug: "requirements".to_string(),
            item_index: 1,
            source_item_id: Some("REQ-1".to_string()),
            body: "Traceability baseline".to_string(),
        };

        let computed = canonical_requirement_id(&item.artifact_path, &item.heading_slug, 1)
            .expect("id should compute");
        assert_ne!(computed, item.canonical_requirement_id);
    }

    #[test]
    fn snapshot_file_digests_encode_as_json_object() {
        let file_digests = BTreeMap::from([
            (".planning/PROJECT.md".to_string(), "digest-a".to_string()),
            (".planning/ROADMAP.md".to_string(), "digest-b".to_string()),
        ]);
        let encoded = serde_json::to_string(&file_digests).expect("should encode map");
        let parsed: serde_json::Value = serde_json::from_str(&encoded).expect("should decode json");
        assert_eq!(parsed, json!(file_digests));
    }
}
