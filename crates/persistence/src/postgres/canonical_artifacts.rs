#[cfg(test)]
mod tests {
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
}
