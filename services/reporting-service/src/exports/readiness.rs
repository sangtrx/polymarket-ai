use crate::exports::artifacts::{deterministic_artifact_order, required_readiness_artifact_types};
use domain::reporting_export::{
    ReportingExportArtifactType, ReportingExportContractError, ReportingExportReasonCode,
    ReportingExportValidationIssue, parse_utc_timestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessSeverityBreakdown {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessRiskSummary {
    pub canonical_requirement_id: String,
    pub severity: String,
    pub risk_score: i32,
    pub priority_rank: usize,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessWaiverLedgerEntry {
    pub canonical_requirement_id: String,
    pub owner: String,
    pub reason_code: String,
    pub approved_by: String,
    pub expires_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessWaiverLedger {
    pub active: Vec<ReadinessWaiverLedgerEntry>,
    pub expired: Vec<ReadinessWaiverLedgerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessReportPayload {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub advisory_state: String,
    pub recommendation_reason_code: String,
    pub recommendation_rationale: String,
    pub severity_counts: ReadinessSeverityBreakdown,
    pub waived_count: usize,
    pub unwaived_count: usize,
    pub top_unresolved_risks: Vec<ReadinessRiskSummary>,
    pub waiver_ledger: ReadinessWaiverLedger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessComposedArtifact {
    pub artifact_type: ReportingExportArtifactType,
    pub payload: String,
}

pub fn compose_readiness_artifact_payloads(
    report: &ReadinessReportPayload,
) -> Result<Vec<ReadinessComposedArtifact>, ReportingExportContractError> {
    validate_readiness_report(report)?;
    let markdown = compose_readiness_report_markdown(report)?;
    let json = serde_json::to_string_pretty(report).map_err(|error| {
        ReportingExportContractError::invalid_payload(format!(
            "readiness report json serialization failed: {error}"
        ))
    })?;

    let mut artifacts = Vec::new();
    for artifact_type in deterministic_artifact_order(required_readiness_artifact_types()) {
        let payload = match artifact_type {
            ReportingExportArtifactType::ReadinessReportJson => json.clone(),
            ReportingExportArtifactType::ReadinessReportMarkdown => markdown.clone(),
            _ => continue,
        };
        artifacts.push(ReadinessComposedArtifact {
            artifact_type,
            payload,
        });
    }
    Ok(artifacts)
}

pub fn compose_readiness_report_markdown(
    report: &ReadinessReportPayload,
) -> Result<String, ReportingExportContractError> {
    validate_readiness_report(report)?;
    let mut markdown = String::new();
    markdown.push_str("# CI Readiness Report\n\n");
    markdown.push_str(&format!("- Snapshot: `{}`\n", report.snapshot_id));
    markdown.push_str(&format!("- Advisory State: `{}`\n", report.advisory_state));
    markdown.push_str(&format!(
        "- Recommendation Code: `{}`\n\n",
        report.recommendation_reason_code
    ));

    markdown.push_str("## Coverage Posture\n");
    markdown.push_str(&format!(
        "- Unwaived unresolved rows: {}\n",
        report.unwaived_count
    ));
    markdown.push_str(&format!(
        "- Waived unresolved rows: {}\n",
        report.waived_count
    ));
    markdown.push_str(&format!(
        "- Severity counts: critical={}, high={}, medium={}, low={}\n\n",
        report.severity_counts.critical,
        report.severity_counts.high,
        report.severity_counts.medium,
        report.severity_counts.low
    ));

    markdown.push_str("## Top Unresolved Risks\n");
    if report.top_unresolved_risks.is_empty() {
        markdown.push_str("- None\n");
    } else {
        for risk in &report.top_unresolved_risks {
            markdown.push_str(&format!(
                "- `{}` (`{}`, score {}, rank {}, reason `{}`)\n",
                risk.canonical_requirement_id,
                risk.severity,
                risk.risk_score,
                risk.priority_rank,
                risk.reason_code
            ));
        }
    }
    markdown.push('\n');

    markdown.push_str("## Waiver Ledger\n");
    markdown.push_str(&format!(
        "- Active waivers: {}\n",
        report.waiver_ledger.active.len()
    ));
    markdown.push_str(&format!(
        "- Expired waivers: {}\n",
        report.waiver_ledger.expired.len()
    ));
    markdown.push_str("- Revoked waivers: excluded from waiver reduction\n\n");

    markdown.push_str("## Recommendation Rationale\n");
    markdown.push_str(&report.recommendation_rationale);
    markdown.push('\n');
    Ok(markdown)
}

fn validate_readiness_report(
    report: &ReadinessReportPayload,
) -> Result<(), ReportingExportContractError> {
    let mut field_errors = Vec::new();
    for (field, value) in [
        ("snapshot_id", report.snapshot_id.trim()),
        ("commit_sha", report.commit_sha.trim()),
        ("advisory_state", report.advisory_state.trim()),
        (
            "recommendation_reason_code",
            report.recommendation_reason_code.trim(),
        ),
        (
            "recommendation_rationale",
            report.recommendation_rationale.trim(),
        ),
    ] {
        if value.is_empty() {
            field_errors.push(ReportingExportValidationIssue {
                field,
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("{field} is required"),
            });
        }
    }
    if let Err(error) = parse_utc_timestamp("generated_at_utc", &report.generated_at_utc) {
        field_errors.extend(error.field_errors);
    }
    for risk in &report.top_unresolved_risks {
        if risk.canonical_requirement_id.trim().is_empty() {
            field_errors.push(ReportingExportValidationIssue {
                field: "top_unresolved_risks.canonical_requirement_id",
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: "top_unresolved_risks.canonical_requirement_id is required".to_string(),
            });
        }
    }
    if !field_errors.is_empty() {
        return Err(ReportingExportContractError::invalid_payload_with_issues(
            "readiness report payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> ReadinessReportPayload {
        ReadinessReportPayload {
            snapshot_id: "readiness_abc123_2026-04-09T00:00:00Z".to_string(),
            commit_sha: "abc123".to_string(),
            generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
            advisory_state: "caution".to_string(),
            recommendation_reason_code: "readiness_caution".to_string(),
            recommendation_rationale:
                "High and medium unresolved risks remain; run readiness audit after remediation."
                    .to_string(),
            severity_counts: ReadinessSeverityBreakdown {
                critical: 0,
                high: 1,
                medium: 1,
                low: 0,
            },
            waived_count: 1,
            unwaived_count: 2,
            top_unresolved_risks: vec![ReadinessRiskSummary {
                canonical_requirement_id: "rptg-01".to_string(),
                severity: "high".to_string(),
                risk_score: 88,
                priority_rank: 1,
                reason_code: "risk_top_priority".to_string(),
            }],
            waiver_ledger: ReadinessWaiverLedger {
                active: vec![ReadinessWaiverLedgerEntry {
                    canonical_requirement_id: "rptg-02".to_string(),
                    owner: "ops-owner".to_string(),
                    reason_code: "approved_exception".to_string(),
                    approved_by: "ops-approver".to_string(),
                    expires_at_utc: "2026-04-12T00:00:00Z".to_string(),
                }],
                expired: vec![],
            },
        }
    }

    #[test]
    fn markdown_contains_required_sections() {
        let markdown =
            compose_readiness_report_markdown(&sample_report()).expect("markdown should compose");
        assert!(markdown.contains("## Coverage Posture"));
        assert!(markdown.contains("## Top Unresolved Risks"));
        assert!(markdown.contains("## Waiver Ledger"));
        assert!(markdown.contains("## Recommendation Rationale"));
    }

    #[test]
    fn composed_artifacts_are_deterministic() {
        let first = compose_readiness_artifact_payloads(&sample_report())
            .expect("first composition should succeed");
        let second = compose_readiness_artifact_payloads(&sample_report())
            .expect("second composition should succeed");
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_eq!(
            first[0].artifact_type,
            ReportingExportArtifactType::ReadinessReportJson
        );
        assert_eq!(
            first[1].artifact_type,
            ReportingExportArtifactType::ReadinessReportMarkdown
        );
    }
}
