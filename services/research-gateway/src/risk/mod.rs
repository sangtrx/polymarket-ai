pub mod classifier;

#[cfg(test)]
mod tests {
    use super::classifier::classify_coverage_row;
    use domain::coverage::{CoverageClass, CoverageMatrixRow};
    use domain::risk_prioritization::RiskSeverity;
    use domain::traceability::{EvidenceAnchor, EvidenceType};

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

    fn row(coverage_class: CoverageClass, reason_code: &str) -> CoverageMatrixRow {
        CoverageMatrixRow {
            canonical_requirement_id: "project_md.requirements.1".to_string(),
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

    #[test]
    fn severity_excludes_covered_rows() {
        let covered = row(CoverageClass::Covered, "deterministic_anchor_match");
        assert!(
            classify_coverage_row(&covered).is_none(),
            "covered rows must not be prioritized",
        );
    }

    #[test]
    fn severity_unknown_reason_fails_closed_with_machine_code() {
        let unresolved = row(CoverageClass::Missing, "totally_unknown_reason");
        let scored = classify_coverage_row(&unresolved).expect("missing row should be scored");
        assert_eq!(scored.severity, RiskSeverity::High);
        assert_eq!(scored.reason_code, "risk_weight_unmapped_reason");
    }

    #[test]
    fn severity_classification_is_deterministic_for_same_payload() {
        let unresolved = row(CoverageClass::Partial, "semantic_fallback_used");
        let first = classify_coverage_row(&unresolved).expect("row should score");
        let second = classify_coverage_row(&unresolved).expect("row should score");
        assert_eq!(first, second);
        assert_eq!(first.severity_weight, 40);
        assert_eq!(first.reason_weight, 10);
        assert_eq!(first.evidence_penalty, 0);
    }
}
