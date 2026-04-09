#[cfg(test)]
mod tests {
    const TRACEABILITY_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260409000300_traceability_mapping.sql");

    #[test]
    fn migration_contract_requires_rationale_and_confidence_enum() {
        assert!(TRACEABILITY_MIGRATION_SQL.contains("traceability_links"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("char_length(trim(rationale)) > 0"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("confidence IN ('high', 'medium', 'low')"));
    }

    #[test]
    fn migration_contract_preserves_many_to_many_evidence_links() {
        assert!(TRACEABILITY_MIGRATION_SQL.contains("traceability_link_anchors"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("evidence_type"));
    }

    #[test]
    fn migration_contract_supports_missing_evidence_outcomes() {
        assert!(TRACEABILITY_MIGRATION_SQL.contains("missing_evidence"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("stale_evidence"));
    }
}
