use domain::reporting_export::{
    REQUIRED_FR36_ARTIFACT_TYPES, REQUIRED_READINESS_ARTIFACT_TYPES, ReportingExportArtifactType,
};
use sha2::{Digest, Sha256};

pub fn required_fr36_artifact_types() -> Vec<ReportingExportArtifactType> {
    REQUIRED_FR36_ARTIFACT_TYPES.to_vec()
}

pub fn required_readiness_artifact_types() -> Vec<ReportingExportArtifactType> {
    REQUIRED_READINESS_ARTIFACT_TYPES.to_vec()
}

pub fn deterministic_artifact_order(
    artifacts: impl IntoIterator<Item = ReportingExportArtifactType>,
) -> Vec<ReportingExportArtifactType> {
    let mut ordered = artifacts.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|artifact| artifact.as_str());
    ordered
}

pub fn artifact_source_for_type(artifact_type: ReportingExportArtifactType) -> &'static str {
    match artifact_type {
        ReportingExportArtifactType::PromotionDecisions => "governance_promotion_decisions",
        ReportingExportArtifactType::ValidationEvidence => "reporting_validation_evidence",
        ReportingExportArtifactType::ReconciliationSummary => "execution_reconciliation_summaries",
        ReportingExportArtifactType::AccessAudits => "privileged_access_audits",
        ReportingExportArtifactType::IncidentPostmortems => "incident_postmortems",
        ReportingExportArtifactType::ReadinessReportJson => "ci_readiness_report",
        ReportingExportArtifactType::ReadinessReportMarkdown => "ci_readiness_report",
    }
}

pub fn compose_package_reference(job_id: &str) -> String {
    format!("s3://reporting-exports/{job_id}/manifest.json")
}

pub fn compose_artifact_reference(
    job_id: &str,
    artifact_type: ReportingExportArtifactType,
) -> String {
    match artifact_type {
        ReportingExportArtifactType::ReadinessReportJson => {
            format!("s3://reporting-exports/{job_id}/readiness-report.json")
        }
        ReportingExportArtifactType::ReadinessReportMarkdown => {
            format!("s3://reporting-exports/{job_id}/readiness-report.md")
        }
        _ => format!(
            "s3://reporting-exports/{job_id}/{}.json",
            artifact_type.as_str()
        ),
    }
}

pub fn compute_manifest_checksum(job_id: &str, correlation_id: &str, as_of_utc: &str) -> String {
    compute_checksum(&[job_id, correlation_id, as_of_utc, "manifest"])
}

pub fn compute_artifact_checksum(
    job_id: &str,
    artifact_type: ReportingExportArtifactType,
    source: &str,
    as_of_utc: &str,
    correlation_id: &str,
) -> String {
    compute_checksum(&[
        job_id,
        artifact_type.as_str(),
        source,
        as_of_utc,
        correlation_id,
    ])
}

fn compute_checksum(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_artifact_set_is_stable() {
        let ordered = required_fr36_artifact_types();
        assert_eq!(ordered, REQUIRED_FR36_ARTIFACT_TYPES.to_vec());
    }

    #[test]
    fn deterministic_sorting_applies_tie_breaking_by_artifact_name() {
        let ordered = deterministic_artifact_order([
            ReportingExportArtifactType::ValidationEvidence,
            ReportingExportArtifactType::AccessAudits,
            ReportingExportArtifactType::PromotionDecisions,
        ]);
        assert_eq!(
            ordered,
            vec![
                ReportingExportArtifactType::AccessAudits,
                ReportingExportArtifactType::PromotionDecisions,
                ReportingExportArtifactType::ValidationEvidence,
            ]
        );
    }

    #[test]
    fn checksum_generation_is_deterministic() {
        let first = compute_artifact_checksum(
            "export-job-001",
            ReportingExportArtifactType::PromotionDecisions,
            "governance_promotion_decisions",
            "2026-04-07T00:00:00Z",
            "corr-export-001",
        );
        let second = compute_artifact_checksum(
            "export-job-001",
            ReportingExportArtifactType::PromotionDecisions,
            "governance_promotion_decisions",
            "2026-04-07T00:00:00Z",
            "corr-export-001",
        );
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn readiness_artifact_references_use_canonical_filenames() {
        assert_eq!(
            compose_artifact_reference(
                "export-job-001",
                ReportingExportArtifactType::ReadinessReportJson
            ),
            "s3://reporting-exports/export-job-001/readiness-report.json"
        );
        assert_eq!(
            compose_artifact_reference(
                "export-job-001",
                ReportingExportArtifactType::ReadinessReportMarkdown
            ),
            "s3://reporting-exports/export-job-001/readiness-report.md"
        );
    }
}
