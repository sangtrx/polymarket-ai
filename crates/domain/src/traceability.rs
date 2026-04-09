use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceabilityContractError {
    pub code: String,
    pub message: String,
}

impl TraceabilityContractError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "traceability_invalid_payload".to_string(),
            message: message.into(),
        }
    }
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

impl TryFrom<&str> for LinkConfidence {
    type Error = TraceabilityContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(TraceabilityContractError::invalid_payload(
                "confidence must be one of: high|medium|low",
            )),
        }
    }
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
    pub reason_code: Option<String>,
}

pub fn build_traceability_link(
    canonical_requirement_id: impl Into<String>,
    anchors: Vec<EvidenceAnchor>,
    rationale: impl Into<String>,
    confidence: LinkConfidence,
    outcome: LinkOutcome,
    reason_code: Option<String>,
) -> Result<TraceabilityLink, TraceabilityContractError> {
    let canonical_requirement_id = canonical_requirement_id.into().trim().to_string();
    if canonical_requirement_id.is_empty() {
        return Err(TraceabilityContractError::invalid_payload(
            "canonical_requirement_id is required",
        ));
    }

    let rationale = rationale.into().trim().to_string();
    if rationale.is_empty() {
        return Err(TraceabilityContractError::invalid_payload(
            "rationale is required",
        ));
    }

    if anchors.is_empty() {
        return Err(TraceabilityContractError::invalid_payload(
            "at least one evidence anchor is required",
        ));
    }

    let normalized_anchors: Result<Vec<EvidenceAnchor>, TraceabilityContractError> = anchors
        .into_iter()
        .map(|anchor| validate_anchor(anchor))
        .collect();
    let normalized_anchors = normalized_anchors?;
    if !normalized_anchors
        .iter()
        .any(|anchor| matches!(anchor.evidence_type, EvidenceType::Code))
    {
        return Err(TraceabilityContractError::invalid_payload(
            "at least one code evidence anchor is required",
        ));
    }

    let reason_code = reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    Ok(TraceabilityLink {
        canonical_requirement_id,
        anchors: normalized_anchors,
        rationale,
        confidence,
        outcome,
        reason_code,
    })
}

fn validate_anchor(anchor: EvidenceAnchor) -> Result<EvidenceAnchor, TraceabilityContractError> {
    let file_path = anchor.file_path.trim().to_string();
    let symbol = anchor
        .symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let section = anchor
        .section
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if file_path.is_empty() {
        return Err(TraceabilityContractError::invalid_payload(
            "anchor file_path is required",
        ));
    }
    if symbol.is_none() && section.is_none() && anchor.line_start.is_none() {
        return Err(TraceabilityContractError::invalid_payload(
            "anchor must include symbol, section, or line_start",
        ));
    }
    if anchor.line_end.is_some() && anchor.line_start.is_none() {
        return Err(TraceabilityContractError::invalid_payload(
            "line_end requires line_start",
        ));
    }
    if let (Some(start), Some(end)) = (anchor.line_start, anchor.line_end)
        && end < start
    {
        return Err(TraceabilityContractError::invalid_payload(
            "line_end must be greater than or equal to line_start",
        ));
    }

    Ok(EvidenceAnchor {
        evidence_type: anchor.evidence_type,
        file_path,
        symbol,
        section,
        line_start: anchor.line_start,
        line_end: anchor.line_end,
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
            Some("deterministic_anchor_match".to_string()),
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
            Some("ambiguous_multiple_candidates".to_string()),
        )
        .expect("valid mapping should pass");
        assert_eq!(link.anchors.len(), 2);
        assert_eq!(link.outcome, LinkOutcome::Ambiguous);
        assert_eq!(
            link.reason_code.as_deref(),
            Some("ambiguous_multiple_candidates")
        );
        assert!(matches!(LinkOutcome::MissingEvidence, LinkOutcome::MissingEvidence));
        assert!(matches!(LinkOutcome::StaleEvidence, LinkOutcome::StaleEvidence));
    }
}
