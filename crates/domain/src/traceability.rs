use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceabilityContractError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    Code,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceAnchor {
    pub evidence_type: EvidenceType,
    pub file_path: String,
    pub symbol: Option<String>,
    pub section: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkOutcome {
    Linked,
    Ambiguous,
    MissingEvidence,
    StaleEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceabilityLink {
    pub canonical_requirement_id: String,
    pub anchors: Vec<EvidenceAnchor>,
    pub rationale: String,
    pub confidence: LinkConfidence,
    pub outcome: LinkOutcome,
}

pub fn build_traceability_link(
    _canonical_requirement_id: impl Into<String>,
    _anchors: Vec<EvidenceAnchor>,
    _rationale: impl Into<String>,
    _confidence: LinkConfidence,
    _outcome: LinkOutcome,
) -> Result<TraceabilityLink, TraceabilityContractError> {
    Err(TraceabilityContractError {
        code: "traceability_invalid_payload".to_string(),
        message: "not yet implemented".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_code_anchor() -> EvidenceAnchor {
        EvidenceAnchor {
            evidence_type: EvidenceType::Code,
            file_path: "crates/domain/src/traceability.rs".to_string(),
            symbol: Some("build_traceability_link".to_string()),
            section: None,
            line_start: Some(10),
            line_end: Some(25),
        }
    }

    #[test]
    fn rejects_empty_rationale_or_missing_anchor_fields() {
        let missing_anchor = EvidenceAnchor {
            evidence_type: EvidenceType::Code,
            file_path: "".to_string(),
            symbol: None,
            section: None,
            line_start: None,
            line_end: None,
        };
        let error = build_traceability_link(
            "project_md.requirements.1",
            vec![missing_anchor],
            "",
            LinkConfidence::High,
            LinkOutcome::Linked,
        )
        .expect_err("invalid payload must fail");
        assert_eq!(error.code, "traceability_invalid_payload");
    }

    #[test]
    fn accepts_only_high_medium_low_confidence() {
        assert_eq!(LinkConfidence::try_from("high"), Ok(LinkConfidence::High));
        assert_eq!(LinkConfidence::try_from("medium"), Ok(LinkConfidence::Medium));
        assert_eq!(LinkConfidence::try_from("low"), Ok(LinkConfidence::Low));
        let error = LinkConfidence::try_from("uncertain").expect_err("unknown value must fail");
        assert_eq!(error.code, "traceability_invalid_payload");
    }

    #[test]
    fn supports_many_to_many_links_and_explicit_outcomes() {
        let code_anchor = valid_code_anchor();
        let test_anchor = EvidenceAnchor {
            evidence_type: EvidenceType::Test,
            file_path: "tests/api/phase-2-traceability.test.mjs".to_string(),
            symbol: Some("traceability mapping".to_string()),
            section: None,
            line_start: Some(1),
            line_end: Some(40),
        };
        let link = build_traceability_link(
            "project_md.requirements.1",
            vec![code_anchor, test_anchor],
            "deterministic mapping confirmed in code and API test",
            LinkConfidence::High,
            LinkOutcome::Ambiguous,
        )
        .expect("valid mapping should pass");
        assert_eq!(link.anchors.len(), 2);
        assert_eq!(link.outcome, LinkOutcome::Ambiguous);
        assert!(matches!(LinkOutcome::MissingEvidence, LinkOutcome::MissingEvidence));
        assert!(matches!(LinkOutcome::StaleEvidence, LinkOutcome::StaleEvidence));
    }
}
