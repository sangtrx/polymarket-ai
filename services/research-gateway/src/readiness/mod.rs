use crate::risk::service::RiskPrioritizationResult;
use domain::readiness::{
    ReadinessReasonCode, ReadinessState, WaiverRecord, WaiverState, parse_utc_timestamp,
    validate_waiver,
};
use domain::risk_prioritization::RiskSeverity;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RunReadinessInput {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub repo_root: String,
    pub risk_prioritization: Option<RiskPrioritizationResult>,
    pub waivers: Vec<WaiverRecord>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessServiceError {
    pub code: String,
    pub message: String,
}

impl ReadinessServiceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ReadinessReasonCode::InvalidPayload.code().to_string(),
            message: message.into(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReadinessReasonCode::DependencyUnavailable.code().to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessSeverityCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessRiskItem {
    pub canonical_requirement_id: String,
    pub severity: RiskSeverity,
    pub risk_score: i32,
    pub priority_rank: usize,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessExplainability {
    pub severity_counts: ReadinessSeverityCounts,
    pub waived_count: usize,
    pub unwaived_count: usize,
    pub top_unresolved_risks: Vec<ReadinessRiskItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessEvaluationResult {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub readiness_state: ReadinessState,
    pub reason_code: String,
    pub explainability: ReadinessExplainability,
}

pub async fn run_readiness(
    input: RunReadinessInput,
) -> Result<ReadinessEvaluationResult, ReadinessServiceError> {
    validate_payload(&input)?;
    let risk_prioritization = input.risk_prioritization.ok_or_else(|| {
        ReadinessServiceError::dependency_unavailable(
            "risk prioritization snapshot is required to evaluate readiness",
        )
    })?;

    let mut active_waiver_ids = BTreeSet::new();
    for waiver in &input.waivers {
        validate_waiver(waiver).map_err(|error| ReadinessServiceError::invalid_payload(error.message))?;
        let state = domain::readiness::resolve_waiver_state(waiver, &input.generated_at_utc)
            .map_err(|error| ReadinessServiceError::dependency_unavailable(error.message))?;
        if state == WaiverState::Active {
            active_waiver_ids.insert(waiver.canonical_requirement_id.clone());
        }
    }

    let mut waived_count = 0usize;
    let mut unwaived = Vec::new();
    for row in &risk_prioritization.rows {
        if active_waiver_ids.contains(&row.canonical_requirement_id) {
            waived_count += 1;
        } else {
            unwaived.push(row);
        }
    }

    let severity_counts = ReadinessSeverityCounts {
        critical: unwaived
            .iter()
            .filter(|row| row.severity == RiskSeverity::Critical)
            .count(),
        high: unwaived
            .iter()
            .filter(|row| row.severity == RiskSeverity::High)
            .count(),
        medium: unwaived
            .iter()
            .filter(|row| row.severity == RiskSeverity::Medium)
            .count(),
        low: unwaived
            .iter()
            .filter(|row| row.severity == RiskSeverity::Low)
            .count(),
    };

    let (readiness_state, reason_code) = if severity_counts.critical > 0 {
        (
            ReadinessState::NotReady,
            ReadinessReasonCode::NotReady.code().to_string(),
        )
    } else if severity_counts.high > 0 || severity_counts.medium > 0 {
        (
            ReadinessState::Caution,
            ReadinessReasonCode::Caution.code().to_string(),
        )
    } else {
        (
            ReadinessState::Ready,
            ReadinessReasonCode::Ready.code().to_string(),
        )
    };

    let top_unresolved_risks = unwaived
        .iter()
        .take(5)
        .map(|row| ReadinessRiskItem {
            canonical_requirement_id: row.canonical_requirement_id.clone(),
            severity: row.severity,
            risk_score: row.risk_score,
            priority_rank: row.priority_rank,
            reason_code: row.reason_code.clone(),
        })
        .collect::<Vec<_>>();

    Ok(ReadinessEvaluationResult {
        snapshot_id: input.snapshot_id,
        commit_sha: input.commit_sha,
        generated_at_utc: input.generated_at_utc,
        readiness_state,
        reason_code,
        explainability: ReadinessExplainability {
            severity_counts,
            waived_count,
            unwaived_count: unwaived.len(),
            top_unresolved_risks,
        },
    })
}

fn validate_payload(input: &RunReadinessInput) -> Result<(), ReadinessServiceError> {
    if input.snapshot_id.trim().is_empty() {
        return Err(ReadinessServiceError::invalid_payload(
            "snapshot_id is required",
        ));
    }
    let commit_sha = input.commit_sha.trim();
    if commit_sha.len() < 6
        || commit_sha.len() > 64
        || !commit_sha
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ReadinessServiceError::invalid_payload(
            "commit_sha must be 6-64 hex characters",
        ));
    }
    parse_utc_timestamp("generated_at_utc", &input.generated_at_utc)
        .map_err(|error| ReadinessServiceError::invalid_payload(error.message))?;
    if !Path::new(input.repo_root.trim()).is_dir() {
        return Err(ReadinessServiceError::invalid_payload(
            "repo_root must reference an existing directory",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::risk::service::RiskFixItem;
    use domain::coverage::CoverageClass;
    use domain::risk_prioritization::RemediationFocus;
    use domain::traceability::{EvidenceAnchor, EvidenceType};

    fn risk_row(
        canonical_requirement_id: &str,
        severity: RiskSeverity,
        risk_score: i32,
        priority_rank: usize,
    ) -> RiskFixItem {
        RiskFixItem {
            canonical_requirement_id: canonical_requirement_id.to_string(),
            coverage_class: CoverageClass::Missing,
            severity,
            risk_score,
            priority_rank,
            reason_code: "risk_reason".to_string(),
            rationale: "rationale".to_string(),
            provenance: "tests".to_string(),
            code_anchor_count: 1,
            test_anchor_count: 0,
            ambiguous_anchor_count: 0,
            top_evidence_anchors: vec![EvidenceAnchor {
                evidence_type: EvidenceType::Code,
                file_path: "services/research-gateway/src/main.rs".to_string(),
                symbol: Some("run_phase5_chain".to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(1),
            }],
            remediation_focus: RemediationFocus::AddEvidence,
            severity_weight: 5,
            reason_weight: 5,
            evidence_penalty: 0,
        }
    }

    fn valid_input(rows: Vec<RiskFixItem>) -> RunReadinessInput {
        RunReadinessInput {
            snapshot_id: "readiness_abc123_2026-04-09T00:00:00Z".to_string(),
            commit_sha: "abc123".to_string(),
            generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: ".".to_string(),
            risk_prioritization: Some(RiskPrioritizationResult {
                snapshot_id: "risk_abc123_2026-04-09T00:00:00Z".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
                rows,
            }),
            waivers: Vec::new(),
        }
    }

    #[tokio::test]
    async fn signal_not_ready_when_unwaived_critical_exists() {
        let output = run_readiness(valid_input(vec![risk_row(
            "gate-03",
            RiskSeverity::Critical,
            100,
            1,
        )]))
        .await
        .expect("evaluation should succeed");
        assert_eq!(output.readiness_state, ReadinessState::NotReady);
        assert_eq!(output.reason_code, "readiness_not_ready");
        assert_eq!(output.explainability.severity_counts.critical, 1);
    }

    #[tokio::test]
    async fn signal_caution_when_unwaived_high_or_medium_exists() {
        let output = run_readiness(valid_input(vec![risk_row(
            "gate-02",
            RiskSeverity::High,
            80,
            1,
        )]))
        .await
        .expect("evaluation should succeed");
        assert_eq!(output.readiness_state, ReadinessState::Caution);
        assert_eq!(output.reason_code, "readiness_caution");
    }

    #[tokio::test]
    async fn signal_ready_when_no_unwaived_critical_high_or_medium_exists() {
        let output = run_readiness(valid_input(vec![risk_row(
            "gate-01",
            RiskSeverity::Low,
            10,
            1,
        )]))
        .await
        .expect("evaluation should succeed");
        assert_eq!(output.readiness_state, ReadinessState::Ready);
        assert_eq!(output.reason_code, "readiness_ready");
    }

    #[tokio::test]
    async fn signal_fails_closed_when_risk_snapshot_missing() {
        let mut input = valid_input(vec![risk_row("gate-01", RiskSeverity::Low, 10, 1)]);
        input.risk_prioritization = None;
        let error = run_readiness(input)
            .await
            .expect_err("missing risk snapshot must fail closed");
        assert_eq!(error.code, "readiness_dependency_unavailable");
    }

    #[tokio::test]
    async fn signal_waived_critical_does_not_trigger_not_ready() {
        let mut input = valid_input(vec![risk_row("gate-03", RiskSeverity::Critical, 100, 1)]);
        input.waivers.push(WaiverRecord {
            waiver_id: "waiver-001".to_string(),
            canonical_requirement_id: "gate-03".to_string(),
            owner: "ops-owner".to_string(),
            reason_code: "approved_exception".to_string(),
            justification: "temporary mitigation".to_string(),
            approved_by: "ops-approver".to_string(),
            created_at_utc: "2026-04-08T00:00:00Z".to_string(),
            expires_at_utc: "2026-04-12T00:00:00Z".to_string(),
            revoked_at_utc: None,
        });
        let output = run_readiness(input)
            .await
            .expect("waived critical should not fail readiness");
        assert_eq!(output.readiness_state, ReadinessState::Ready);
        assert_eq!(output.explainability.waived_count, 1);
        assert_eq!(output.explainability.unwaived_count, 0);
    }

    #[tokio::test]
    async fn signal_includes_deterministic_top_unresolved_risk_ordering() {
        let output = run_readiness(valid_input(vec![
            risk_row("gate-01", RiskSeverity::High, 90, 1),
            risk_row("gate-02", RiskSeverity::Medium, 70, 2),
            risk_row("gate-03", RiskSeverity::Low, 20, 3),
        ]))
        .await
        .expect("evaluation should succeed");
        assert_eq!(output.explainability.top_unresolved_risks.len(), 3);
        assert_eq!(
            output.explainability.top_unresolved_risks[0].canonical_requirement_id,
            "gate-01"
        );
        assert_eq!(
            output.explainability.top_unresolved_risks[1].canonical_requirement_id,
            "gate-02"
        );
    }

    #[tokio::test]
    async fn signal_fails_closed_with_machine_code_for_invalid_waiver_payload() {
        let mut input = valid_input(vec![risk_row("gate-03", RiskSeverity::Critical, 100, 1)]);
        input.waivers.push(WaiverRecord {
            waiver_id: "waiver-001".to_string(),
            canonical_requirement_id: "gate-03".to_string(),
            owner: "ops-owner".to_string(),
            reason_code: "approved_exception".to_string(),
            justification: "temporary mitigation".to_string(),
            approved_by: "ops-approver".to_string(),
            created_at_utc: "2026-04-08T00:00:00Z".to_string(),
            expires_at_utc: "2026-04-08T00:00:00Z".to_string(),
            revoked_at_utc: None,
        });
        let error = run_readiness(input)
            .await
            .expect_err("invalid waiver payload must fail closed");
        assert_eq!(error.code, "readiness_invalid_payload");
    }
}
