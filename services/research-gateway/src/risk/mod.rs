pub mod classifier;
pub mod service;

#[cfg(test)]
mod tests {
    use super::classifier::classify_coverage_row;
    use super::service::{RunRiskPrioritizationInput, run_risk_prioritization};
    use crate::coverage::service::CoverageMatrixResult;
    use domain::coverage::{CoverageClass, CoverageMatrixRow};
    use domain::risk_prioritization::RiskSeverity;
    use domain::traceability::{EvidenceAnchor, EvidenceType};
    use serde_json::to_string;

    fn anchor(path: &str, symbol: &str, evidence_type: EvidenceType) -> EvidenceAnchor {
        EvidenceAnchor {
            evidence_type,
            file_path: path.to_string(),
            symbol: Some(symbol.to_string()),
            section: None,
            line_start: Some(1),
            line_end: Some(20),
        }
    }

    fn row(
        canonical_requirement_id: &str,
        coverage_class: CoverageClass,
        reason_code: &str,
    ) -> CoverageMatrixRow {
        CoverageMatrixRow {
            canonical_requirement_id: canonical_requirement_id.to_string(),
            coverage_class,
            reason_code: reason_code.to_string(),
            rationale: "coverage rationale".to_string(),
            code_anchors: vec![anchor(
                "services/research-gateway/src/coverage/service.rs",
                "run_coverage_classification",
                EvidenceType::Code,
            )],
            test_anchors: vec![anchor(
                "tests/api/phase-3-coverage-classification.test.mjs",
                "coverage-contract",
                EvidenceType::Test,
            )],
            ambiguous_candidates: vec![],
            provenance: "coverage_service".to_string(),
        }
    }

    fn risk_input(rows: Vec<CoverageMatrixRow>) -> RunRiskPrioritizationInput {
        RunRiskPrioritizationInput {
            snapshot_id: "risk_snapshot_1".to_string(),
            commit_sha: "abc123".to_string(),
            generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: ".".to_string(),
            coverage_matrix: CoverageMatrixResult {
                snapshot_id: "coverage_snapshot_1".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
                rows,
            },
        }
    }

    #[test]
    fn severity_excludes_covered_rows() {
        let covered = row(
            "project_md.requirements.1",
            CoverageClass::Covered,
            "deterministic_anchor_match",
        );
        assert!(
            classify_coverage_row(&covered).is_none(),
            "covered rows must not be prioritized",
        );
    }

    #[test]
    fn severity_unknown_reason_fails_closed_with_machine_code() {
        let unresolved = row(
            "project_md.requirements.1",
            CoverageClass::Missing,
            "totally_unknown_reason",
        );
        let scored = classify_coverage_row(&unresolved).expect("missing row should be scored");
        assert_eq!(scored.severity, RiskSeverity::High);
        assert_eq!(scored.reason_code, "risk_weight_unmapped_reason");
    }

    #[test]
    fn severity_classification_is_deterministic_for_same_payload() {
        let unresolved = row(
            "project_md.requirements.1",
            CoverageClass::Partial,
            "semantic_fallback_used",
        );
        let first = classify_coverage_row(&unresolved).expect("row should score");
        let second = classify_coverage_row(&unresolved).expect("row should score");
        assert_eq!(first, second);
        assert_eq!(first.severity_weight, 40);
        assert_eq!(first.reason_weight, 10);
        assert_eq!(first.evidence_penalty, 0);
    }

    #[tokio::test]
    async fn ranking_sorts_by_severity_desc_then_risk_score_desc_then_canonical_requirement_id_asc()
     {
        let output = run_risk_prioritization(risk_input(vec![
            row("req-z", CoverageClass::Missing, "missing_evidence"),
            row("req-c", CoverageClass::Partial, "semantic_fallback_used"),
            row("req-b", CoverageClass::Partial, "semantic_fallback_used"),
            row("req-a", CoverageClass::Partial, "semantic_fallback_used"),
        ]))
        .await
        .expect("risk prioritization should succeed");

        assert_eq!(
            output
                .rows
                .iter()
                .map(|item| item.canonical_requirement_id.as_str())
                .collect::<Vec<_>>(),
            vec!["req-z", "req-a", "req-b", "req-c"],
        );
    }

    #[tokio::test]
    async fn ranking_assigns_contiguous_one_based_priority_ranks() {
        let output = run_risk_prioritization(risk_input(vec![
            row("req-1", CoverageClass::Missing, "missing_evidence"),
            row("req-2", CoverageClass::Partial, "stale_link_invalidated"),
            row("req-3", CoverageClass::Partial, "semantic_fallback_used"),
        ]))
        .await
        .expect("risk prioritization should succeed");

        assert_eq!(
            output
                .rows
                .iter()
                .map(|item| item.priority_rank)
                .collect::<Vec<_>>(),
            vec![1, 2, 3],
        );
    }

    #[tokio::test]
    async fn guardrails_reject_invalid_payload_with_risk_invalid_payload_code() {
        let mut input = risk_input(vec![row(
            "req-1",
            CoverageClass::Missing,
            "missing_evidence",
        )]);
        input.commit_sha = "not-hex".to_string();
        let error = run_risk_prioritization(input)
            .await
            .expect_err("invalid payload must fail closed");
        assert_eq!(error.code, "risk_invalid_payload");
    }

    #[tokio::test]
    async fn guardrails_excludes_covered_rows_from_service_output() {
        let output = run_risk_prioritization(risk_input(vec![
            row("req-covered", CoverageClass::Covered, "deterministic_anchor_match"),
            row("req-missing", CoverageClass::Missing, "missing_evidence"),
        ]))
        .await
        .expect("risk prioritization should succeed");
        assert!(
            output
                .rows
                .iter()
                .all(|item| !matches!(item.coverage_class, CoverageClass::Covered)),
            "covered rows must never appear in prioritization output",
        );
    }

    #[tokio::test]
    async fn guardrails_unknown_reason_still_maps_to_high_severity() {
        let output = run_risk_prioritization(risk_input(vec![row(
            "req-unknown",
            CoverageClass::Missing,
            "brand_new_reason",
        )]))
        .await
        .expect("unknown reason should fail closed to a high-severity row");
        assert_eq!(output.rows.len(), 1);
        assert_eq!(output.rows[0].severity, RiskSeverity::High);
        assert_eq!(output.rows[0].reason_code, "risk_weight_unmapped_reason");
    }

    #[tokio::test]
    async fn guardrails_identical_input_returns_byte_equivalent_json_order() {
        let input = risk_input(vec![
            row("req-b", CoverageClass::Partial, "semantic_fallback_used"),
            row("req-a", CoverageClass::Partial, "semantic_fallback_used"),
            row("req-c", CoverageClass::Missing, "missing_evidence"),
        ]);
        let first = run_risk_prioritization(input.clone())
            .await
            .expect("first run should succeed");
        let second = run_risk_prioritization(input)
            .await
            .expect("second run should succeed");
        assert_eq!(
            to_string(&first).expect("first payload should serialize"),
            to_string(&second).expect("second payload should serialize"),
        );
    }
}
