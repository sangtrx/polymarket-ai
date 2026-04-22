pub mod matcher;
pub mod service;

#[cfg(test)]
mod tests {
    use super::matcher::{
        EvidenceCandidate, MatchRequest, PreviousLink, match_requirement_to_evidence,
    };
    use domain::traceability::{EvidenceAnchor, EvidenceType, LinkConfidence, LinkOutcome};

    fn code_candidate(path: &str, symbol: &str, score: f32) -> EvidenceCandidate {
        EvidenceCandidate {
            anchor: EvidenceAnchor {
                evidence_type: EvidenceType::Code,
                file_path: path.to_string(),
                symbol: Some(symbol.to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(20),
            },
            rationale: format!("candidate from {path}"),
            provenance: "matcher-test".to_string(),
            score,
        }
    }

    #[test]
    fn deterministic_anchor_matching_is_preferred() {
        let result = match_requirement_to_evidence(MatchRequest {
            canonical_requirement_id: "project_md.requirements.1".to_string(),
            deterministic_candidates: vec![code_candidate(
                "services/research-gateway/src/traceability/service.rs",
                "run_traceability_mapping",
                1.0,
            )],
            semantic_candidates: vec![code_candidate(
                "services/research-gateway/src/traceability/matcher.rs",
                "semantic_probe",
                0.4,
            )],
            previous_links: vec![],
        });
        assert_eq!(result.reason_code, "deterministic_anchor_match");
    }

    #[test]
    fn semantic_fallback_emits_downgraded_confidence_and_reason_code() {
        let result = match_requirement_to_evidence(MatchRequest {
            canonical_requirement_id: "project_md.requirements.2".to_string(),
            deterministic_candidates: vec![],
            semantic_candidates: vec![code_candidate(
                "services/research-gateway/src/traceability/matcher.rs",
                "semantic_probe",
                0.6,
            )],
            previous_links: vec![],
        });
        assert_eq!(result.reason_code, "semantic_fallback_used");
        assert_eq!(result.confidence, LinkConfidence::Low);
    }

    #[test]
    fn ambiguous_and_missing_and_stale_outcomes_are_explicit() {
        let ambiguous = match_requirement_to_evidence(MatchRequest {
            canonical_requirement_id: "project_md.requirements.3".to_string(),
            deterministic_candidates: vec![
                code_candidate("services/research-gateway/src/a.rs", "alpha", 1.0),
                code_candidate("services/research-gateway/src/b.rs", "beta", 0.9),
            ],
            semantic_candidates: vec![],
            previous_links: vec![],
        });
        assert_eq!(ambiguous.outcome, LinkOutcome::Ambiguous);
        assert_eq!(ambiguous.ambiguous_candidates.len(), 2);

        let stale = match_requirement_to_evidence(MatchRequest {
            canonical_requirement_id: "project_md.requirements.4".to_string(),
            deterministic_candidates: vec![],
            semantic_candidates: vec![],
            previous_links: vec![PreviousLink {
                snapshot_id: "traceability_snapshot_0".to_string(),
                anchor: EvidenceAnchor {
                    evidence_type: EvidenceType::Code,
                    file_path: "services/research-gateway/src/missing.rs".to_string(),
                    symbol: Some("gone".to_string()),
                    section: None,
                    line_start: Some(1),
                    line_end: Some(5),
                },
            }],
        });
        assert_eq!(stale.outcome, LinkOutcome::StaleEvidence);

        let missing = match_requirement_to_evidence(MatchRequest {
            canonical_requirement_id: "project_md.requirements.5".to_string(),
            deterministic_candidates: vec![],
            semantic_candidates: vec![],
            previous_links: vec![],
        });
        assert_eq!(missing.outcome, LinkOutcome::MissingEvidence);
    }
}
