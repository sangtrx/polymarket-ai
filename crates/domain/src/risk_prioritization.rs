use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

pub const UNMAPPED_REASON_MACHINE_CODE: &str = "risk_weight_unmapped_reason";

const CRITICAL_WEIGHT: i32 = 100;
const HIGH_WEIGHT: i32 = 70;
const MEDIUM_WEIGHT: i32 = 40;
const LOW_WEIGHT: i32 = 10;

const MISSING_EVIDENCE_WEIGHT: i32 = 25;
const STALE_LINK_INVALIDATED_WEIGHT: i32 = 20;
const SEMANTIC_FALLBACK_WEIGHT: i32 = 10;
const DETERMINISTIC_ANCHOR_WEIGHT: i32 = 0;

const MISSING_CODE_ANCHOR_PENALTY: i32 = 15;
const MISSING_TEST_ANCHOR_PENALTY: i32 = 5;
const AMBIGUOUS_ANCHOR_PENALTY: i32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskPrioritizationContractError {
    pub code: String,
    pub message: String,
}

impl RiskPrioritizationContractError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "risk_invalid_payload".to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl RiskSeverity {
    pub fn weight(self) -> i32 {
        match self {
            Self::Critical => CRITICAL_WEIGHT,
            Self::High => HIGH_WEIGHT,
            Self::Medium => MEDIUM_WEIGHT,
            Self::Low => LOW_WEIGHT,
        }
    }
}

impl TryFrom<&str> for RiskSeverity {
    type Error = RiskPrioritizationContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(RiskPrioritizationContractError::invalid_payload(
                "severity must be one of: critical|high|medium|low",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemediationFocus {
    AddEvidence,
    RestoreTraceability,
    ConfirmSemanticCoverage,
    VerifyDeterministicLink,
    ReviewReasonMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskScoreBreakdown {
    pub severity: RiskSeverity,
    pub reason_code: String,
    pub remediation_focus: RemediationFocus,
    pub severity_weight: i32,
    pub reason_weight: i32,
    pub evidence_penalty: i32,
    pub risk_score: i32,
}

pub fn score_unresolved_row(
    reason_code: impl AsRef<str>,
    code_anchor_count: usize,
    test_anchor_count: usize,
    ambiguous_anchor_count: usize,
) -> RiskScoreBreakdown {
    let mapped_reason = map_reason_weight(reason_code.as_ref());
    let evidence_penalty = evidence_penalty(
        code_anchor_count,
        test_anchor_count,
        ambiguous_anchor_count,
    );
    let severity_weight = mapped_reason.severity.weight();
    let risk_score = severity_weight + mapped_reason.reason_weight + evidence_penalty;

    RiskScoreBreakdown {
        severity: mapped_reason.severity,
        reason_code: mapped_reason.reason_code,
        remediation_focus: mapped_reason.remediation_focus,
        severity_weight,
        reason_weight: mapped_reason.reason_weight,
        evidence_penalty,
        risk_score,
    }
}

fn evidence_penalty(
    code_anchor_count: usize,
    test_anchor_count: usize,
    ambiguous_anchor_count: usize,
) -> i32 {
    let mut penalty = 0;
    if code_anchor_count == 0 {
        penalty += MISSING_CODE_ANCHOR_PENALTY;
    }
    if test_anchor_count == 0 {
        penalty += MISSING_TEST_ANCHOR_PENALTY;
    }
    if ambiguous_anchor_count > 0 {
        penalty += AMBIGUOUS_ANCHOR_PENALTY;
    }
    penalty
}

struct MappedReasonWeight {
    severity: RiskSeverity,
    reason_code: String,
    remediation_focus: RemediationFocus,
    reason_weight: i32,
}

fn map_reason_weight(reason_code: &str) -> MappedReasonWeight {
    match reason_code.trim().to_ascii_lowercase().as_str() {
        "missing_evidence" => MappedReasonWeight {
            severity: RiskSeverity::Critical,
            reason_code: "missing_evidence".to_string(),
            remediation_focus: RemediationFocus::AddEvidence,
            reason_weight: MISSING_EVIDENCE_WEIGHT,
        },
        "stale_link_invalidated" => MappedReasonWeight {
            severity: RiskSeverity::High,
            reason_code: "stale_link_invalidated".to_string(),
            remediation_focus: RemediationFocus::RestoreTraceability,
            reason_weight: STALE_LINK_INVALIDATED_WEIGHT,
        },
        "semantic_fallback_used" => MappedReasonWeight {
            severity: RiskSeverity::Medium,
            reason_code: "semantic_fallback_used".to_string(),
            remediation_focus: RemediationFocus::ConfirmSemanticCoverage,
            reason_weight: SEMANTIC_FALLBACK_WEIGHT,
        },
        "deterministic_anchor_match" => MappedReasonWeight {
            severity: RiskSeverity::Low,
            reason_code: "deterministic_anchor_match".to_string(),
            remediation_focus: RemediationFocus::VerifyDeterministicLink,
            reason_weight: DETERMINISTIC_ANCHOR_WEIGHT,
        },
        _ => MappedReasonWeight {
            severity: RiskSeverity::High,
            reason_code: UNMAPPED_REASON_MACHINE_CODE.to_string(),
            remediation_focus: RemediationFocus::ReviewReasonMapping,
            reason_weight: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_severity_values() {
        assert_eq!(
            RiskSeverity::try_from("critical"),
            Ok(RiskSeverity::Critical)
        );
        assert_eq!(RiskSeverity::try_from("high"), Ok(RiskSeverity::High));
        assert_eq!(RiskSeverity::try_from("medium"), Ok(RiskSeverity::Medium));
        assert_eq!(RiskSeverity::try_from("low"), Ok(RiskSeverity::Low));
    }

    #[test]
    fn rejects_unknown_severity_values() {
        let error = RiskSeverity::try_from("urgent").expect_err("invalid severity must fail");
        assert_eq!(error.code, "risk_invalid_payload");
    }

    #[test]
    fn unknown_reason_maps_to_high_with_machine_code() {
        let breakdown = score_unresolved_row("brand_new_reason", 0, 0, 0);
        assert_eq!(breakdown.severity, RiskSeverity::High);
        assert_eq!(breakdown.reason_code, "risk_weight_unmapped_reason");
        assert_eq!(breakdown.remediation_focus, RemediationFocus::ReviewReasonMapping);
    }

    #[test]
    fn scoring_breakdown_emits_explainability_weights() {
        let breakdown = score_unresolved_row("missing_evidence", 0, 1, 2);
        assert_eq!(breakdown.severity, RiskSeverity::Critical);
        assert_eq!(breakdown.severity_weight, 100);
        assert_eq!(breakdown.reason_weight, 25);
        assert_eq!(breakdown.evidence_penalty, 25);
        assert_eq!(breakdown.risk_score, 150);
    }
}
