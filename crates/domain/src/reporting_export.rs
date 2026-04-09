use crate::governance::{GovernancePermission, GovernanceRole, RolePermissionMatrix};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub const DEFAULT_EXPORT_LIST_LIMIT: i64 = 100;
pub const MAX_EXPORT_LIST_LIMIT: i64 = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingExportReasonCode {
    Ready,
    InvalidPayload,
    Unauthorized,
    DependencyUnavailable,
    StaleEvidence,
    PersistenceUnavailable,
    IntegrityMismatch,
    NotFound,
    MissingIncidentContext,
    ArtifactUnavailable,
    DuplicateSuppressed,
    JobSucceeded,
    JobFailed,
}

impl ReportingExportReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "reporting_export_ready",
            Self::InvalidPayload => "reporting_export_invalid_payload",
            Self::Unauthorized => "reporting_export_unauthorized",
            Self::DependencyUnavailable => "reporting_export_dependency_unavailable",
            Self::StaleEvidence => "reporting_export_stale_evidence",
            Self::PersistenceUnavailable => "reporting_export_persistence_unavailable",
            Self::IntegrityMismatch => "reporting_export_integrity_mismatch",
            Self::NotFound => "reporting_export_not_found",
            Self::MissingIncidentContext => "reporting_export_missing_incident_context",
            Self::ArtifactUnavailable => "reporting_export_artifact_unavailable",
            Self::DuplicateSuppressed => "reporting_export_duplicate_suppressed",
            Self::JobSucceeded => "reporting_export_job_succeeded",
            Self::JobFailed => "reporting_export_job_failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingExportContractError> {
        match normalize_reporting_export_identifier(value).as_str() {
            "reporting_export_ready" => Ok(Self::Ready),
            "reporting_export_invalid_payload" => Ok(Self::InvalidPayload),
            "reporting_export_unauthorized" => Ok(Self::Unauthorized),
            "reporting_export_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "reporting_export_stale_evidence" => Ok(Self::StaleEvidence),
            "reporting_export_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "reporting_export_integrity_mismatch" => Ok(Self::IntegrityMismatch),
            "reporting_export_not_found" => Ok(Self::NotFound),
            "reporting_export_missing_incident_context" => Ok(Self::MissingIncidentContext),
            "reporting_export_artifact_unavailable" => Ok(Self::ArtifactUnavailable),
            "reporting_export_duplicate_suppressed" => Ok(Self::DuplicateSuppressed),
            "reporting_export_job_succeeded" => Ok(Self::JobSucceeded),
            "reporting_export_job_failed" => Ok(Self::JobFailed),
            other => Err(ReportingExportContractError::invalid_payload(format!(
                "unknown reporting export reason code `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingExportTriggerSource {
    ScheduledWeekly,
    OnDemand,
    IncidentTriggered,
}

impl ReportingExportTriggerSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScheduledWeekly => "scheduled_weekly",
            Self::OnDemand => "on_demand",
            Self::IncidentTriggered => "incident_triggered",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingExportContractError> {
        match normalize_reporting_export_identifier(value).as_str() {
            "scheduled_weekly" => Ok(Self::ScheduledWeekly),
            "on_demand" => Ok(Self::OnDemand),
            "incident_triggered" => Ok(Self::IncidentTriggered),
            _ => Err(ReportingExportContractError::invalid_payload_with_issues(
                "trigger_source must be one of: scheduled_weekly, on_demand, incident_triggered",
                vec![ReportingExportValidationIssue {
                    field: "trigger_source",
                    code: ReportingExportReasonCode::InvalidPayload.code(),
                    message: "trigger_source must be one of: scheduled_weekly, on_demand, incident_triggered"
                        .to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingExportJobState {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl ReportingExportJobState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingExportContractError> {
        match normalize_reporting_export_identifier(value).as_str() {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            _ => Err(ReportingExportContractError::invalid_payload_with_issues(
                "status must be one of: queued, running, succeeded, failed",
                vec![ReportingExportValidationIssue {
                    field: "status",
                    code: ReportingExportReasonCode::InvalidPayload.code(),
                    message: "status must be one of: queued, running, succeeded, failed"
                        .to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReportingExportArtifactType {
    PromotionDecisions,
    ValidationEvidence,
    ReconciliationSummary,
    AccessAudits,
    IncidentPostmortems,
    ReadinessReportJson,
    ReadinessReportMarkdown,
}

impl ReportingExportArtifactType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PromotionDecisions => "promotion_decisions",
            Self::ValidationEvidence => "validation_evidence",
            Self::ReconciliationSummary => "reconciliation_summary",
            Self::AccessAudits => "access_audits",
            Self::IncidentPostmortems => "incident_postmortems",
            Self::ReadinessReportJson => "readiness_report_json",
            Self::ReadinessReportMarkdown => "readiness_report_markdown",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingExportContractError> {
        match normalize_reporting_export_identifier(value).as_str() {
            "promotion_decisions" => Ok(Self::PromotionDecisions),
            "validation_evidence" => Ok(Self::ValidationEvidence),
            "reconciliation_summary" => Ok(Self::ReconciliationSummary),
            "access_audits" => Ok(Self::AccessAudits),
            "incident_postmortems" => Ok(Self::IncidentPostmortems),
            "readiness_report_json" => Ok(Self::ReadinessReportJson),
            "readiness_report_markdown" => Ok(Self::ReadinessReportMarkdown),
            _ => Err(ReportingExportContractError::invalid_payload_with_issues(
                "artifact_type must be one of: promotion_decisions, validation_evidence, reconciliation_summary, access_audits, incident_postmortems, readiness_report_json, readiness_report_markdown",
                vec![ReportingExportValidationIssue {
                    field: "artifact_type",
                    code: ReportingExportReasonCode::InvalidPayload.code(),
                    message: "artifact_type must be one of: promotion_decisions, validation_evidence, reconciliation_summary, access_audits, incident_postmortems, readiness_report_json, readiness_report_markdown".to_string(),
                }],
            )),
        }
    }
}

pub const REQUIRED_FR36_ARTIFACT_TYPES: [ReportingExportArtifactType; 5] = [
    ReportingExportArtifactType::PromotionDecisions,
    ReportingExportArtifactType::ValidationEvidence,
    ReportingExportArtifactType::ReconciliationSummary,
    ReportingExportArtifactType::AccessAudits,
    ReportingExportArtifactType::IncidentPostmortems,
];

pub const REQUIRED_READINESS_ARTIFACT_TYPES: [ReportingExportArtifactType; 2] = [
    ReportingExportArtifactType::ReadinessReportJson,
    ReportingExportArtifactType::ReadinessReportMarkdown,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingExportValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingExportContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingExportValidationIssue>,
}

impl ReportingExportContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ReportingExportValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn integrity_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::IntegrityMismatch.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn missing_incident_context(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::MissingIncidentContext.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportJobRecord {
    pub job_id: String,
    pub trigger_source: ReportingExportTriggerSource,
    pub status: ReportingExportJobState,
    pub reason_code: String,
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub schedule_id: Option<String>,
    pub schedule_window_key: Option<String>,
    pub report_run_id: Option<String>,
    pub incident_id: Option<String>,
    pub incident_severity: Option<String>,
    pub requested_at_utc: String,
    pub started_at_utc: Option<String>,
    pub finished_at_utc: Option<String>,
    pub package_reference: Option<String>,
    pub package_checksum: Option<String>,
    pub failure_metadata: Option<String>,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportArtifactRecord {
    pub artifact_id: String,
    pub job_id: String,
    pub artifact_type: ReportingExportArtifactType,
    pub source: String,
    pub as_of_utc: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub checksum: String,
    pub retrieval_reference: String,
    pub is_available: bool,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

pub fn normalize_reporting_export_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ReportingExportContractError> {
    let timestamp = OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| {
        ReportingExportContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![ReportingExportValidationIssue {
                field,
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    if timestamp.offset() != UtcOffset::UTC {
        return Err(ReportingExportContractError::invalid_payload_with_issues(
            format!("{field} must be UTC (offset Z)"),
            vec![ReportingExportValidationIssue {
                field,
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("{field} must be UTC (offset Z)"),
            }],
        ));
    }
    Ok(timestamp.to_offset(UtcOffset::UTC))
}

pub fn format_utc_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .expect("UTC RFC3339 formatting must succeed")
}

pub fn authorize_export_trigger(actor_role: &str) -> Result<(), ReportingExportContractError> {
    let role = GovernanceRole::parse(actor_role).map_err(|_| {
        ReportingExportContractError::unauthorized(
            "actor role is not authorized for report export mutations",
        )
    })?;
    let matrix = RolePermissionMatrix::canonical();
    let allowed = matrix
        .has_permission(role, GovernancePermission::ExecuteControlAction)
        .map_err(|_| {
            ReportingExportContractError::unauthorized(
                "authorization matrix rejected actor role for control action",
            )
        })?;
    if !allowed {
        return Err(ReportingExportContractError::unauthorized(
            "actor role is not authorized for report export mutations",
        ));
    }
    Ok(())
}

pub fn authorize_export_read(actor_role: &str) -> Result<(), ReportingExportContractError> {
    let role = GovernanceRole::parse(actor_role).map_err(|_| {
        ReportingExportContractError::unauthorized(
            "actor role is not authorized for report export reads",
        )
    })?;
    let matrix = RolePermissionMatrix::canonical();
    let allowed = matrix
        .has_permission(role, GovernancePermission::ReadAnalytics)
        .map_err(|_| {
            ReportingExportContractError::unauthorized(
                "authorization matrix rejected actor role for analytics read",
            )
        })?;
    if !allowed {
        return Err(ReportingExportContractError::unauthorized(
            "actor role is not authorized for report export reads",
        ));
    }
    Ok(())
}

pub fn validate_export_job_transition(
    previous: Option<ReportingExportJobState>,
    next: ReportingExportJobState,
) -> Result<(), ReportingExportContractError> {
    let is_valid = match previous {
        None => next == ReportingExportJobState::Queued,
        Some(current) if current == next => true,
        Some(ReportingExportJobState::Queued) => next == ReportingExportJobState::Running,
        Some(ReportingExportJobState::Running) => {
            next == ReportingExportJobState::Succeeded || next == ReportingExportJobState::Failed
        }
        Some(ReportingExportJobState::Succeeded | ReportingExportJobState::Failed) => false,
    };

    if is_valid {
        return Ok(());
    }
    Err(ReportingExportContractError::invalid_payload_with_issues(
        "export job transition must follow queued -> running -> succeeded|failed lifecycle",
        vec![ReportingExportValidationIssue {
            field: "status",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message:
                "export job transition must follow queued -> running -> succeeded|failed lifecycle"
                    .to_string(),
        }],
    ))
}

pub fn validate_export_job(job: &ExportJobRecord) -> Result<(), ReportingExportContractError> {
    let mut field_errors = Vec::new();

    validate_canonical_identifier_issue(&mut field_errors, "job_id", &job.job_id);
    validate_canonical_identifier_issue(&mut field_errors, "reason_code", &job.reason_code);
    validate_canonical_identifier_issue(&mut field_errors, "actor_id", &job.actor_id);
    validate_canonical_identifier_issue(&mut field_errors, "actor_role", &job.actor_role);
    validate_canonical_identifier_issue(&mut field_errors, "correlation_id", &job.correlation_id);

    validate_optional_identifier_issue(
        &mut field_errors,
        "schedule_id",
        job.schedule_id.as_deref(),
    );
    validate_optional_identifier_issue(
        &mut field_errors,
        "schedule_window_key",
        job.schedule_window_key.as_deref(),
    );
    validate_optional_identifier_issue(
        &mut field_errors,
        "report_run_id",
        job.report_run_id.as_deref(),
    );
    validate_optional_identifier_issue(
        &mut field_errors,
        "incident_id",
        job.incident_id.as_deref(),
    );
    validate_optional_identifier_issue(
        &mut field_errors,
        "incident_severity",
        job.incident_severity.as_deref(),
    );

    if ReportingExportReasonCode::parse(&job.reason_code).is_err() {
        field_errors.push(ReportingExportValidationIssue {
            field: "reason_code",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported reporting export reason code".to_string(),
        });
    }

    let requested_at =
        parse_timestamp_issue(&mut field_errors, "requested_at_utc", &job.requested_at_utc);
    let started_at = parse_optional_timestamp_issue(
        &mut field_errors,
        "started_at_utc",
        job.started_at_utc.as_deref(),
    );
    let finished_at = parse_optional_timestamp_issue(
        &mut field_errors,
        "finished_at_utc",
        job.finished_at_utc.as_deref(),
    );
    let created_at =
        parse_timestamp_issue(&mut field_errors, "created_at_utc", &job.created_at_utc);
    let updated_at =
        parse_timestamp_issue(&mut field_errors, "updated_at_utc", &job.updated_at_utc);

    if matches!(
        job.status,
        ReportingExportJobState::Queued | ReportingExportJobState::Running
    ) && job.finished_at_utc.is_some()
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "finished_at_utc",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "queued/running jobs must not include finished_at_utc".to_string(),
        });
    }

    if matches!(
        job.status,
        ReportingExportJobState::Running
            | ReportingExportJobState::Succeeded
            | ReportingExportJobState::Failed
    ) && job.started_at_utc.is_none()
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "started_at_utc",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "running/succeeded/failed jobs must include started_at_utc".to_string(),
        });
    }

    if matches!(
        job.status,
        ReportingExportJobState::Succeeded | ReportingExportJobState::Failed
    ) && job.finished_at_utc.is_none()
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "finished_at_utc",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "succeeded/failed jobs must include finished_at_utc".to_string(),
        });
    }

    if let (Some(requested_at), Some(started_at)) = (requested_at, started_at)
        && started_at < requested_at
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "started_at_utc",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "started_at_utc must be greater than or equal to requested_at_utc".to_string(),
        });
    }

    if let (Some(started_at), Some(finished_at)) = (started_at, finished_at)
        && finished_at < started_at
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "finished_at_utc",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "finished_at_utc must be greater than or equal to started_at_utc".to_string(),
        });
    }

    if let (Some(created_at), Some(updated_at)) = (created_at, updated_at)
        && updated_at < created_at
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "updated_at_utc",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "updated_at_utc must be greater than or equal to created_at_utc".to_string(),
        });
    }

    if let Some(checksum) = job.package_checksum.as_deref()
        && !checksum.is_empty()
        && !is_hex_checksum(checksum)
    {
        field_errors.push(ReportingExportValidationIssue {
            field: "package_checksum",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "package_checksum must be a 64-character lowercase hex digest".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(ReportingExportContractError::invalid_payload_with_issues(
            "export job failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_export_artifact(
    artifact: &ExportArtifactRecord,
) -> Result<(), ReportingExportContractError> {
    let mut field_errors = Vec::new();

    validate_canonical_identifier_issue(&mut field_errors, "artifact_id", &artifact.artifact_id);
    validate_canonical_identifier_issue(&mut field_errors, "job_id", &artifact.job_id);
    validate_canonical_identifier_issue(&mut field_errors, "source", &artifact.source);
    validate_canonical_identifier_issue(&mut field_errors, "reason_code", &artifact.reason_code);
    validate_canonical_identifier_issue(
        &mut field_errors,
        "correlation_id",
        &artifact.correlation_id,
    );

    if ReportingExportReasonCode::parse(&artifact.reason_code).is_err() {
        field_errors.push(ReportingExportValidationIssue {
            field: "reason_code",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported reporting export reason code".to_string(),
        });
    }

    parse_timestamp_issue(&mut field_errors, "as_of_utc", &artifact.as_of_utc);
    parse_timestamp_issue(
        &mut field_errors,
        "created_at_utc",
        &artifact.created_at_utc,
    );
    parse_timestamp_issue(
        &mut field_errors,
        "updated_at_utc",
        &artifact.updated_at_utc,
    );

    if artifact.retrieval_reference.trim().is_empty() {
        field_errors.push(ReportingExportValidationIssue {
            field: "retrieval_reference",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "retrieval_reference must not be empty".to_string(),
        });
    }

    if artifact.is_available {
        if !is_hex_checksum(&artifact.checksum) {
            field_errors.push(ReportingExportValidationIssue {
                field: "checksum",
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: "checksum must be a 64-character lowercase hex digest".to_string(),
            });
        }
    } else if artifact.checksum != "unavailable" {
        field_errors.push(ReportingExportValidationIssue {
            field: "checksum",
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: "checksum must be `unavailable` when is_available is false".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(ReportingExportContractError::invalid_payload_with_issues(
            "export artifact failed validation",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_optional_identifier_issue(
    issues: &mut Vec<ReportingExportValidationIssue>,
    field: &'static str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        validate_canonical_identifier_issue(issues, field, value);
    }
}

fn validate_canonical_identifier_issue(
    issues: &mut Vec<ReportingExportValidationIssue>,
    field: &'static str,
    value: &str,
) {
    let normalized = normalize_reporting_export_identifier(value);
    let (min_len, max_len) = identifier_length_bounds(field);
    if normalized.len() < min_len || normalized.len() > max_len {
        issues.push(ReportingExportValidationIssue {
            field,
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain {min_len}-{max_len} canonical characters"),
        });
        return;
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        issues.push(ReportingExportValidationIssue {
            field,
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: format!("{field} contains unsupported characters"),
        });
    }
}

fn identifier_length_bounds(field: &'static str) -> (usize, usize) {
    match field {
        "schedule_window_key" => (3, 240),
        _ => (3, 200),
    }
}

fn parse_timestamp_issue(
    issues: &mut Vec<ReportingExportValidationIssue>,
    field: &'static str,
    value: &str,
) -> Option<OffsetDateTime> {
    match parse_utc_timestamp(field, value) {
        Ok(timestamp) => Some(timestamp),
        Err(error) => {
            issues.extend(error.field_errors);
            None
        }
    }
}

fn parse_optional_timestamp_issue(
    issues: &mut Vec<ReportingExportValidationIssue>,
    field: &'static str,
    value: Option<&str>,
) -> Option<OffsetDateTime> {
    value.and_then(|candidate| parse_timestamp_issue(issues, field, candidate))
}

fn is_hex_checksum(value: &str) -> bool {
    let normalized = value.trim();
    normalized.len() == 64
        && normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_job(status: ReportingExportJobState) -> ExportJobRecord {
        ExportJobRecord {
            job_id: "export-job-001".to_string(),
            trigger_source: ReportingExportTriggerSource::OnDemand,
            status,
            reason_code: ReportingExportReasonCode::Ready.code().to_string(),
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            correlation_id: "corr-export-001".to_string(),
            schedule_id: None,
            schedule_window_key: None,
            report_run_id: None,
            incident_id: None,
            incident_severity: None,
            requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
            started_at_utc: if status != ReportingExportJobState::Queued {
                Some("2026-04-07T00:00:01Z".to_string())
            } else {
                None
            },
            finished_at_utc: if matches!(
                status,
                ReportingExportJobState::Succeeded | ReportingExportJobState::Failed
            ) {
                Some("2026-04-07T00:00:02Z".to_string())
            } else {
                None
            },
            package_reference: Some("s3://reporting-exports/export-job-001".to_string()),
            package_checksum: Some("a".repeat(64)),
            failure_metadata: None,
            created_at_utc: "2026-04-07T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
        }
    }

    fn sample_artifact() -> ExportArtifactRecord {
        ExportArtifactRecord {
            artifact_id: "export-artifact-001".to_string(),
            job_id: "export-job-001".to_string(),
            artifact_type: ReportingExportArtifactType::PromotionDecisions,
            source: "reporting_read_models".to_string(),
            as_of_utc: "2026-04-07T00:00:00Z".to_string(),
            reason_code: ReportingExportReasonCode::Ready.code().to_string(),
            correlation_id: "corr-export-001".to_string(),
            checksum: "b".repeat(64),
            retrieval_reference: "s3://reporting-exports/export-job-001/promotion.json".to_string(),
            is_available: true,
            created_at_utc: "2026-04-07T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn reason_codes_roundtrip_is_canonical() {
        for reason in [
            ReportingExportReasonCode::Ready,
            ReportingExportReasonCode::InvalidPayload,
            ReportingExportReasonCode::Unauthorized,
            ReportingExportReasonCode::DependencyUnavailable,
            ReportingExportReasonCode::StaleEvidence,
            ReportingExportReasonCode::PersistenceUnavailable,
            ReportingExportReasonCode::IntegrityMismatch,
            ReportingExportReasonCode::NotFound,
            ReportingExportReasonCode::MissingIncidentContext,
            ReportingExportReasonCode::ArtifactUnavailable,
            ReportingExportReasonCode::DuplicateSuppressed,
            ReportingExportReasonCode::JobSucceeded,
            ReportingExportReasonCode::JobFailed,
        ] {
            let parsed =
                ReportingExportReasonCode::parse(reason.code()).expect("reason code should parse");
            assert_eq!(parsed, reason);
        }
    }

    #[test]
    fn trigger_source_and_artifact_type_parsing_is_canonical() {
        assert_eq!(
            ReportingExportTriggerSource::parse(" Scheduled_Weekly ").expect("should parse"),
            ReportingExportTriggerSource::ScheduledWeekly
        );
        assert_eq!(
            ReportingExportArtifactType::parse("ACCESS_AUDITS").expect("should parse"),
            ReportingExportArtifactType::AccessAudits
        );
    }

    #[test]
    fn export_transition_lifecycle_is_fail_closed() {
        validate_export_job_transition(None, ReportingExportJobState::Queued)
            .expect("new export job should start queued");
        validate_export_job_transition(
            Some(ReportingExportJobState::Queued),
            ReportingExportJobState::Running,
        )
        .expect("queued should advance to running");
        validate_export_job_transition(
            Some(ReportingExportJobState::Running),
            ReportingExportJobState::Succeeded,
        )
        .expect("running should advance to terminal");

        let error = validate_export_job_transition(
            Some(ReportingExportJobState::Queued),
            ReportingExportJobState::Succeeded,
        )
        .expect_err("queued -> succeeded should be rejected");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
    }

    #[test]
    fn authorization_reuses_governance_role_matrix() {
        authorize_export_trigger("operational_control")
            .expect("control role should mutate exports");
        authorize_export_read("read_only_analytics")
            .expect("analytics role should read export evidence");

        let error = authorize_export_trigger("read_only_analytics")
            .expect_err("read-only role must not trigger exports");
        assert_eq!(error.code, ReportingExportReasonCode::Unauthorized.code());
    }

    #[test]
    fn utc_timestamp_parsing_rejects_non_utc_offsets() {
        let error = parse_utc_timestamp("requested_at_utc", "2026-04-07T00:00:00+01:00")
            .expect_err("non-utc offset should be rejected");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
    }

    #[test]
    fn export_job_validation_enforces_identifier_bounds_and_timestamps() {
        validate_export_job(&sample_job(ReportingExportJobState::Succeeded))
            .expect("sample job should validate");

        let mut invalid = sample_job(ReportingExportJobState::Running);
        invalid.job_id = "x".to_string();
        invalid.started_at_utc = Some("2026-04-06T23:59:59Z".to_string());
        let error = validate_export_job(&invalid).expect_err("invalid job should fail");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "job_id")
        );
    }

    #[test]
    fn export_artifact_validation_enforces_integrity_fields() {
        validate_export_artifact(&sample_artifact()).expect("sample artifact should validate");

        let mut invalid = sample_artifact();
        invalid.checksum = "bad-checksum".to_string();
        let error = validate_export_artifact(&invalid).expect_err("invalid checksum must fail");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "checksum")
        );
    }

    #[test]
    fn unavailable_artifact_requires_unavailable_checksum_and_reference() {
        let mut unavailable = sample_artifact();
        unavailable.is_available = false;
        unavailable.checksum = "b".repeat(64);
        unavailable.retrieval_reference = String::new();
        let error =
            validate_export_artifact(&unavailable).expect_err("unavailable artifact must validate");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "checksum")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "retrieval_reference")
        );
    }

    #[test]
    fn required_artifact_types_match_fr36_coverage_contract() {
        assert_eq!(REQUIRED_FR36_ARTIFACT_TYPES.len(), 5);
        assert_eq!(
            REQUIRED_FR36_ARTIFACT_TYPES,
            [
                ReportingExportArtifactType::PromotionDecisions,
                ReportingExportArtifactType::ValidationEvidence,
                ReportingExportArtifactType::ReconciliationSummary,
                ReportingExportArtifactType::AccessAudits,
                ReportingExportArtifactType::IncidentPostmortems,
            ]
        );
    }

    #[test]
    fn required_readiness_artifact_types_match_phase5_contract() {
        assert_eq!(REQUIRED_READINESS_ARTIFACT_TYPES.len(), 2);
        assert_eq!(
            REQUIRED_READINESS_ARTIFACT_TYPES,
            [
                ReportingExportArtifactType::ReadinessReportJson,
                ReportingExportArtifactType::ReadinessReportMarkdown,
            ]
        );
    }
}
