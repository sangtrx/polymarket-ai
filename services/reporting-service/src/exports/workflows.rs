use crate::exports::artifacts::{
    artifact_source_for_type, compose_artifact_reference, compose_package_reference,
    compute_artifact_checksum, compute_manifest_checksum, deterministic_artifact_order,
    required_fr36_artifact_types,
};
use domain::reporting_export::{
    DEFAULT_EXPORT_LIST_LIMIT, ExportArtifactRecord, ExportJobRecord, MAX_EXPORT_LIST_LIMIT,
    ReportingExportArtifactType, ReportingExportContractError, ReportingExportJobState,
    ReportingExportReasonCode, ReportingExportTriggerSource, ReportingExportValidationIssue,
    authorize_export_read, authorize_export_trigger, normalize_reporting_export_identifier,
    parse_utc_timestamp, validate_export_artifact, validate_export_job,
    validate_export_job_transition,
};
use persistence::postgres::export_jobs::{
    ReportExportPersistenceError, load_export_artifact_by_id, load_export_artifacts_for_job,
    load_export_job_by_id, load_export_job_by_weekly_binding, upsert_export_artifact,
    upsert_export_job,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};
use time::Duration;

const DEFAULT_INCIDENT_EXPORT_RUNBOOK: &str =
    "https://docs.example.com/operations/report-export-workflows";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportExportWorkflowError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ReportingExportValidationIssue>,
}

impl ReportExportWorkflowError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ReportingExportValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn integrity_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::IntegrityMismatch.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn missing_incident_context(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::MissingIncidentContext.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn artifact_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingExportReasonCode::ArtifactUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ReportExportWorkflowError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReportExportWorkflowError {}

#[derive(Debug, Clone)]
pub struct TriggerOnDemandExportInput {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub as_of_utc: String,
    pub reason_code: Option<String>,
    pub unavailable_artifact_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TriggerIncidentExportInput {
    pub actor_id: String,
    pub actor_role: String,
    pub incident_id: String,
    pub incident_severity: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub as_of_utc: String,
    pub reason_code: Option<String>,
    pub unavailable_artifact_types: Vec<String>,
    pub impacted_system: Option<String>,
    pub runbook_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DispatchWeeklyExportInput {
    pub actor_id: String,
    pub actor_role: String,
    pub schedule_id: String,
    pub schedule_window_key: String,
    pub report_run_id: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub as_of_utc: String,
    pub reason_code: Option<String>,
    pub unavailable_artifact_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct QueryExportJobInput {
    pub actor_id: String,
    pub actor_role: String,
    pub job_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListExportArtifactsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub job_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct GetExportArtifactInput {
    pub actor_id: String,
    pub actor_role: String,
    pub job_id: String,
    pub artifact_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportExportJobEvidence {
    pub job_id: String,
    pub trigger_source: String,
    pub status: String,
    pub reason_code: String,
    pub artifact_count: usize,
    pub missing_artifact_types: Vec<String>,
    pub package_reference: Option<String>,
    pub package_checksum: Option<String>,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub started_at_utc: Option<String>,
    pub finished_at_utc: Option<String>,
    pub updated_at_utc: String,
}

pub trait ReportExportOrchestrator: Send + Sync {
    fn warmup_status(&self) -> &'static str;
    fn trigger_on_demand_export(
        &self,
        input: TriggerOnDemandExportInput,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError>;
    fn trigger_incident_export(
        &self,
        input: TriggerIncidentExportInput,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError>;
    fn dispatch_weekly_export(
        &self,
        input: DispatchWeeklyExportInput,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError>;
    fn query_export_job(
        &self,
        input: QueryExportJobInput,
    ) -> Result<ExportJobRecord, ReportExportWorkflowError>;
    fn list_export_artifacts(
        &self,
        input: ListExportArtifactsInput,
    ) -> Result<Vec<ExportArtifactRecord>, ReportExportWorkflowError>;
    fn get_export_artifact(
        &self,
        input: GetExportArtifactInput,
    ) -> Result<ExportArtifactRecord, ReportExportWorkflowError>;
}

trait ReportExportRepositoryPort: Send + Sync {
    fn warmup_status(&self) -> &'static str;
    fn upsert_job(
        &self,
        job: ExportJobRecord,
    ) -> Result<ExportJobRecord, ReportExportWorkflowError>;
    fn load_job_by_id(
        &self,
        job_id: &str,
    ) -> Result<Option<ExportJobRecord>, ReportExportWorkflowError>;
    fn load_job_by_weekly_binding(
        &self,
        schedule_id: &str,
        schedule_window_key: &str,
        report_run_id: &str,
    ) -> Result<Option<ExportJobRecord>, ReportExportWorkflowError>;
    fn upsert_artifact(
        &self,
        artifact: ExportArtifactRecord,
    ) -> Result<ExportArtifactRecord, ReportExportWorkflowError>;
    fn load_artifacts_for_job(
        &self,
        job_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ExportArtifactRecord>, ReportExportWorkflowError>;
    fn load_artifact_by_id(
        &self,
        job_id: &str,
        artifact_id: &str,
    ) -> Result<Option<ExportArtifactRecord>, ReportExportWorkflowError>;
}

#[derive(Clone)]
pub struct ReportExportWorkflowService {
    repository: Arc<dyn ReportExportRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl ReportExportWorkflowService {
    fn new(repository: Arc<dyn ReportExportRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryReportExportRepository::default()))
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresReportExportRepository::new(pool)))
    }

    pub fn bootstrap_from_env() -> Result<Option<Self>, ReportExportWorkflowError> {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(error) => {
                return Err(ReportExportWorkflowError::dependency_unavailable(format!(
                    "failed to read DATABASE_URL: {error}"
                )));
            }
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_lazy(&database_url)
            .map_err(|error| {
                ReportExportWorkflowError::dependency_unavailable(format!(
                    "failed to initialize reporting export pool: {error}"
                ))
            })?;
        Ok(Some(Self::postgres(pool)))
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, ReportExportWorkflowError> {
        self.operation_lock.lock().map_err(|_| {
            ReportExportWorkflowError::persistence_unavailable(
                "report export operation lock poisoned by prior panic",
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_export(
        &self,
        trigger_source: ReportingExportTriggerSource,
        actor_id: String,
        actor_role: String,
        correlation_id: String,
        requested_at_utc: String,
        as_of_utc: String,
        reason_code: Option<String>,
        schedule_id: Option<String>,
        schedule_window_key: Option<String>,
        report_run_id: Option<String>,
        incident_id: Option<String>,
        incident_severity: Option<String>,
        unavailable_artifact_types: Vec<String>,
        impacted_system: Option<String>,
        runbook_url: Option<String>,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
        let _lock = self.lock_operations()?;

        validate_non_empty("actor_id", &actor_id)?;
        validate_non_empty("actor_role", &actor_role)?;
        validate_non_empty("correlation_id", &correlation_id)?;
        parse_utc_timestamp("requested_at_utc", &requested_at_utc).map_err(map_contract_error)?;
        parse_utc_timestamp("as_of_utc", &as_of_utc).map_err(map_contract_error)?;

        let normalized_correlation = normalize_reporting_export_identifier(&correlation_id);
        let normalized_actor_id = normalize_reporting_export_identifier(&actor_id);
        let normalized_actor_role = normalize_reporting_export_identifier(&actor_role);
        let unavailable_types = normalize_unavailable_artifact_types(&unavailable_artifact_types)?;

        if trigger_source == ReportingExportTriggerSource::ScheduledWeekly {
            let schedule_id = schedule_id.as_deref().ok_or_else(|| {
                ReportExportWorkflowError::invalid_payload(
                    "scheduled exports require schedule_id",
                    vec![ReportingExportValidationIssue {
                        field: "schedule_id",
                        code: ReportingExportReasonCode::InvalidPayload.code(),
                        message: "scheduled exports require schedule_id".to_string(),
                    }],
                )
            })?;
            let schedule_window_key = schedule_window_key.as_deref().ok_or_else(|| {
                ReportExportWorkflowError::invalid_payload(
                    "scheduled exports require schedule_window_key",
                    vec![ReportingExportValidationIssue {
                        field: "schedule_window_key",
                        code: ReportingExportReasonCode::InvalidPayload.code(),
                        message: "scheduled exports require schedule_window_key".to_string(),
                    }],
                )
            })?;
            let report_run_id = report_run_id.as_deref().ok_or_else(|| {
                ReportExportWorkflowError::invalid_payload(
                    "scheduled exports require report_run_id",
                    vec![ReportingExportValidationIssue {
                        field: "report_run_id",
                        code: ReportingExportReasonCode::InvalidPayload.code(),
                        message: "scheduled exports require report_run_id".to_string(),
                    }],
                )
            })?;
            if let Some(existing) = self.repository.load_job_by_weekly_binding(
                schedule_id,
                schedule_window_key,
                report_run_id,
            )? {
                let artifacts = self
                    .repository
                    .load_artifacts_for_job(&existing.job_id, Some(MAX_EXPORT_LIST_LIMIT))?;
                emit_export_telemetry(ExportJobTelemetryEvent {
                    event_name: "report_export_job_transition_v1",
                    trigger_source: existing.trigger_source.as_str(),
                    status: existing.status.as_str(),
                    reason_code: ReportingExportReasonCode::DuplicateSuppressed.code(),
                    job_id: &existing.job_id,
                    correlation_id: &existing.correlation_id,
                    timestamp_utc: &requested_at_utc,
                });
                return Ok(to_job_evidence(existing, artifacts));
            }
        }

        let initial_reason_code =
            parse_optional_reason_code(reason_code.as_deref(), ReportingExportReasonCode::Ready)?;
        let job_id = compose_export_job_id(
            trigger_source,
            schedule_id.as_deref(),
            schedule_window_key.as_deref(),
            report_run_id.as_deref(),
            &normalized_correlation,
            &requested_at_utc,
        );

        let queued = match self.repository.upsert_job(ExportJobRecord {
            job_id: job_id.clone(),
            trigger_source,
            status: ReportingExportJobState::Queued,
            reason_code: initial_reason_code.code().to_string(),
            actor_id: normalized_actor_id.clone(),
            actor_role: normalized_actor_role.clone(),
            correlation_id: normalized_correlation.clone(),
            schedule_id: schedule_id
                .as_deref()
                .map(normalize_reporting_export_identifier),
            schedule_window_key: schedule_window_key
                .as_deref()
                .map(normalize_reporting_export_identifier),
            report_run_id: report_run_id
                .as_deref()
                .map(normalize_reporting_export_identifier),
            incident_id: incident_id
                .as_deref()
                .map(normalize_reporting_export_identifier),
            incident_severity: incident_severity
                .as_deref()
                .map(normalize_reporting_export_identifier),
            requested_at_utc: requested_at_utc.clone(),
            started_at_utc: None,
            finished_at_utc: None,
            package_reference: None,
            package_checksum: None,
            failure_metadata: None,
            created_at_utc: requested_at_utc.clone(),
            updated_at_utc: requested_at_utc.clone(),
        }) {
            Ok(queued) => queued,
            Err(error)
                if trigger_source == ReportingExportTriggerSource::ScheduledWeekly
                    && is_weekly_idempotency_collision(&error) =>
            {
                if let (Some(schedule_id), Some(schedule_window_key), Some(report_run_id)) = (
                    schedule_id.as_deref(),
                    schedule_window_key.as_deref(),
                    report_run_id.as_deref(),
                ) && let Some(existing) = self.repository.load_job_by_weekly_binding(
                    schedule_id,
                    schedule_window_key,
                    report_run_id,
                )? {
                    let artifacts = self
                        .repository
                        .load_artifacts_for_job(&existing.job_id, Some(MAX_EXPORT_LIST_LIMIT))?;
                    emit_export_telemetry(ExportJobTelemetryEvent {
                        event_name: "report_export_job_transition_v1",
                        trigger_source: existing.trigger_source.as_str(),
                        status: existing.status.as_str(),
                        reason_code: ReportingExportReasonCode::DuplicateSuppressed.code(),
                        job_id: &existing.job_id,
                        correlation_id: &existing.correlation_id,
                        timestamp_utc: &requested_at_utc,
                    });
                    return Ok(to_job_evidence(existing, artifacts));
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        emit_export_telemetry(ExportJobTelemetryEvent {
            event_name: "report_export_job_transition_v1",
            trigger_source: queued.trigger_source.as_str(),
            status: queued.status.as_str(),
            reason_code: &queued.reason_code,
            job_id: &queued.job_id,
            correlation_id: &queued.correlation_id,
            timestamp_utc: &queued.updated_at_utc,
        });

        let running = self.repository.upsert_job(ExportJobRecord {
            status: ReportingExportJobState::Running,
            started_at_utc: Some(requested_at_utc.clone()),
            updated_at_utc: requested_at_utc.clone(),
            ..queued.clone()
        })?;
        emit_export_telemetry(ExportJobTelemetryEvent {
            event_name: "report_export_job_transition_v1",
            trigger_source: running.trigger_source.as_str(),
            status: running.status.as_str(),
            reason_code: &running.reason_code,
            job_id: &running.job_id,
            correlation_id: &running.correlation_id,
            timestamp_utc: &running.updated_at_utc,
        });

        let mut missing_artifacts = Vec::new();
        let mut persisted_artifacts = Vec::new();
        for artifact_type in deterministic_artifact_order(required_fr36_artifact_types()) {
            let source = artifact_source_for_type(artifact_type).to_string();
            let is_available = !unavailable_types.contains(&artifact_type);
            let artifact_reason_code = if is_available {
                ReportingExportReasonCode::Ready
            } else {
                ReportingExportReasonCode::DependencyUnavailable
            };
            if !is_available {
                missing_artifacts.push(artifact_type.as_str().to_string());
            }
            let artifact = self.repository.upsert_artifact(ExportArtifactRecord {
                artifact_id: compose_export_artifact_id(&running.job_id, artifact_type),
                job_id: running.job_id.clone(),
                artifact_type,
                source: normalize_reporting_export_identifier(&source),
                as_of_utc: as_of_utc.clone(),
                reason_code: artifact_reason_code.code().to_string(),
                correlation_id: running.correlation_id.clone(),
                checksum: if is_available {
                    compute_artifact_checksum(
                        &running.job_id,
                        artifact_type,
                        &source,
                        &as_of_utc,
                        &running.correlation_id,
                    )
                } else {
                    "unavailable".to_string()
                },
                retrieval_reference: compose_artifact_reference(&running.job_id, artifact_type),
                is_available,
                created_at_utc: requested_at_utc.clone(),
                updated_at_utc: requested_at_utc.clone(),
            })?;
            persisted_artifacts.push(artifact);
        }

        let finished_reason_code = if missing_artifacts.is_empty() {
            ReportingExportReasonCode::JobSucceeded
        } else {
            ReportingExportReasonCode::DependencyUnavailable
        };
        let finished_status = if missing_artifacts.is_empty() {
            ReportingExportJobState::Succeeded
        } else {
            ReportingExportJobState::Failed
        };
        let finished_at_utc = requested_at_utc.clone();
        let failure_metadata = if missing_artifacts.is_empty() {
            None
        } else {
            Some(
                serde_json::json!({
                    "missing_artifact_types": missing_artifacts,
                    "trigger_source": trigger_source.as_str(),
                    "reason_code": finished_reason_code.code(),
                })
                .to_string(),
            )
        };

        let finished = self.repository.upsert_job(ExportJobRecord {
            status: finished_status,
            reason_code: finished_reason_code.code().to_string(),
            finished_at_utc: Some(finished_at_utc.clone()),
            package_reference: Some(compose_package_reference(&running.job_id)),
            package_checksum: Some(compute_manifest_checksum(
                &running.job_id,
                &running.correlation_id,
                &as_of_utc,
            )),
            failure_metadata,
            updated_at_utc: finished_at_utc.clone(),
            ..running
        })?;
        emit_export_telemetry(ExportJobTelemetryEvent {
            event_name: "report_export_job_transition_v1",
            trigger_source: finished.trigger_source.as_str(),
            status: finished.status.as_str(),
            reason_code: &finished.reason_code,
            job_id: &finished.job_id,
            correlation_id: &finished.correlation_id,
            timestamp_utc: &finished.updated_at_utc,
        });

        if finished.trigger_source == ReportingExportTriggerSource::IncidentTriggered
            && finished.status == ReportingExportJobState::Failed
        {
            emit_incident_export_failure_alert(
                &finished,
                impacted_system
                    .as_deref()
                    .unwrap_or("reporting export workflow"),
                runbook_url
                    .as_deref()
                    .unwrap_or(DEFAULT_INCIDENT_EXPORT_RUNBOOK),
                &finished_at_utc,
            )?;
        }

        Ok(to_job_evidence(finished, persisted_artifacts))
    }
}

impl Default for ReportExportWorkflowService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ReportExportOrchestrator for ReportExportWorkflowService {
    fn warmup_status(&self) -> &'static str {
        self.repository.warmup_status()
    }

    fn trigger_on_demand_export(
        &self,
        input: TriggerOnDemandExportInput,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
        authorize_export_trigger(&input.actor_role).map_err(map_contract_error)?;
        self.execute_export(
            ReportingExportTriggerSource::OnDemand,
            input.actor_id,
            input.actor_role,
            input.correlation_id,
            input.requested_at_utc,
            input.as_of_utc,
            input.reason_code,
            None,
            None,
            None,
            None,
            None,
            input.unavailable_artifact_types,
            None,
            None,
        )
    }

    fn trigger_incident_export(
        &self,
        input: TriggerIncidentExportInput,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
        authorize_export_trigger(&input.actor_role).map_err(map_contract_error)?;
        if input.incident_id.trim().is_empty() || input.incident_severity.trim().is_empty() {
            return Err(ReportExportWorkflowError::missing_incident_context(
                "incident_id and incident_severity are required for incident-triggered exports",
            ));
        }
        self.execute_export(
            ReportingExportTriggerSource::IncidentTriggered,
            input.actor_id,
            input.actor_role,
            input.correlation_id,
            input.requested_at_utc,
            input.as_of_utc,
            input.reason_code,
            None,
            None,
            None,
            Some(input.incident_id),
            Some(input.incident_severity),
            input.unavailable_artifact_types,
            input.impacted_system,
            input.runbook_url,
        )
    }

    fn dispatch_weekly_export(
        &self,
        input: DispatchWeeklyExportInput,
    ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
        authorize_export_trigger(&input.actor_role).map_err(map_contract_error)?;
        self.execute_export(
            ReportingExportTriggerSource::ScheduledWeekly,
            input.actor_id,
            input.actor_role,
            input.correlation_id,
            input.requested_at_utc,
            input.as_of_utc,
            input.reason_code,
            Some(input.schedule_id),
            Some(input.schedule_window_key),
            Some(input.report_run_id),
            None,
            None,
            input.unavailable_artifact_types,
            None,
            None,
        )
    }

    fn query_export_job(
        &self,
        input: QueryExportJobInput,
    ) -> Result<ExportJobRecord, ReportExportWorkflowError> {
        authorize_export_read(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        parse_utc_timestamp("queried_at_utc", &input.queried_at_utc).map_err(map_contract_error)?;

        let job = self
            .repository
            .load_job_by_id(&input.job_id)?
            .ok_or_else(|| {
                ReportExportWorkflowError::not_found(format!(
                    "report export job `{}` not found",
                    input.job_id
                ))
            })?;
        emit_export_telemetry(ExportJobTelemetryEvent {
            event_name: "report_export_job_query_v1",
            trigger_source: job.trigger_source.as_str(),
            status: job.status.as_str(),
            reason_code: ReportingExportReasonCode::Ready.code(),
            job_id: &job.job_id,
            correlation_id: &normalize_reporting_export_identifier(&input.correlation_id),
            timestamp_utc: &input.queried_at_utc,
        });
        Ok(job)
    }

    fn list_export_artifacts(
        &self,
        input: ListExportArtifactsInput,
    ) -> Result<Vec<ExportArtifactRecord>, ReportExportWorkflowError> {
        authorize_export_read(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        parse_utc_timestamp("queried_at_utc", &input.queried_at_utc).map_err(map_contract_error)?;
        let normalized_job_id = normalize_reporting_export_identifier(&input.job_id);
        let job_exists = self.repository.load_job_by_id(&input.job_id)?.is_some();
        if !job_exists {
            return Err(ReportExportWorkflowError::not_found(format!(
                "report export job `{normalized_job_id}` not found"
            )));
        }
        let limit = normalize_limit(input.limit)?;
        let artifacts = self
            .repository
            .load_artifacts_for_job(&input.job_id, Some(limit))?;
        emit_export_telemetry(ExportJobTelemetryEvent {
            event_name: "report_export_artifact_list_v1",
            trigger_source: "mixed",
            status: "query",
            reason_code: ReportingExportReasonCode::Ready.code(),
            job_id: &normalized_job_id,
            correlation_id: &normalize_reporting_export_identifier(&input.correlation_id),
            timestamp_utc: &input.queried_at_utc,
        });
        Ok(artifacts)
    }

    fn get_export_artifact(
        &self,
        input: GetExportArtifactInput,
    ) -> Result<ExportArtifactRecord, ReportExportWorkflowError> {
        authorize_export_read(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        parse_utc_timestamp("queried_at_utc", &input.queried_at_utc).map_err(map_contract_error)?;

        let artifact = self
            .repository
            .load_artifact_by_id(&input.job_id, &input.artifact_id)?
            .ok_or_else(|| {
                ReportExportWorkflowError::not_found(format!(
                    "report export artifact `{}` not found for job `{}`",
                    input.artifact_id, input.job_id
                ))
            })?;
        if !artifact.is_available {
            return Err(ReportExportWorkflowError::artifact_unavailable(
                "requested artifact is unavailable for retrieval",
            ));
        }
        if artifact.checksum.len() != 64
            || !artifact
                .checksum
                .chars()
                .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
            || artifact.retrieval_reference.trim().is_empty()
        {
            return Err(ReportExportWorkflowError::integrity_mismatch(
                "artifact checksum/reference integrity mismatch",
            ));
        }
        emit_export_telemetry(ExportJobTelemetryEvent {
            event_name: "report_export_artifact_read_v1",
            trigger_source: "artifact",
            status: "query",
            reason_code: ReportingExportReasonCode::Ready.code(),
            job_id: &normalize_reporting_export_identifier(&input.job_id),
            correlation_id: &normalize_reporting_export_identifier(&input.correlation_id),
            timestamp_utc: &input.queried_at_utc,
        });
        Ok(artifact)
    }
}

#[derive(Debug, Clone)]
struct PostgresReportExportRepository {
    pool: PgPool,
}

impl PostgresReportExportRepository {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, ReportExportWorkflowError>
    where
        F: Future<Output = Result<T, ReportExportPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ReportExportWorkflowError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl ReportExportRepositoryPort for PostgresReportExportRepository {
    fn warmup_status(&self) -> &'static str {
        "report-export-adapter-initialized"
    }

    fn upsert_job(
        &self,
        job: ExportJobRecord,
    ) -> Result<ExportJobRecord, ReportExportWorkflowError> {
        self.run_with_runtime(upsert_export_job(&self.pool, &job))
    }

    fn load_job_by_id(
        &self,
        job_id: &str,
    ) -> Result<Option<ExportJobRecord>, ReportExportWorkflowError> {
        self.run_with_runtime(load_export_job_by_id(&self.pool, job_id))
    }

    fn load_job_by_weekly_binding(
        &self,
        schedule_id: &str,
        schedule_window_key: &str,
        report_run_id: &str,
    ) -> Result<Option<ExportJobRecord>, ReportExportWorkflowError> {
        self.run_with_runtime(load_export_job_by_weekly_binding(
            &self.pool,
            schedule_id,
            schedule_window_key,
            report_run_id,
        ))
    }

    fn upsert_artifact(
        &self,
        artifact: ExportArtifactRecord,
    ) -> Result<ExportArtifactRecord, ReportExportWorkflowError> {
        self.run_with_runtime(upsert_export_artifact(&self.pool, &artifact))
    }

    fn load_artifacts_for_job(
        &self,
        job_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ExportArtifactRecord>, ReportExportWorkflowError> {
        self.run_with_runtime(load_export_artifacts_for_job(&self.pool, job_id, limit))
    }

    fn load_artifact_by_id(
        &self,
        job_id: &str,
        artifact_id: &str,
    ) -> Result<Option<ExportArtifactRecord>, ReportExportWorkflowError> {
        self.run_with_runtime(load_export_artifact_by_id(&self.pool, job_id, artifact_id))
    }
}

#[derive(Debug, Default)]
struct InMemoryReportExportRepository {
    jobs: Mutex<BTreeMap<String, ExportJobRecord>>,
    artifacts: Mutex<BTreeMap<String, ExportArtifactRecord>>,
}

impl ReportExportRepositoryPort for InMemoryReportExportRepository {
    fn warmup_status(&self) -> &'static str {
        "report-export-adapter-in-memory"
    }

    fn upsert_job(
        &self,
        job: ExportJobRecord,
    ) -> Result<ExportJobRecord, ReportExportWorkflowError> {
        validate_export_job(&job).map_err(map_contract_error)?;
        let mut jobs = self
            .jobs
            .lock()
            .expect("in-memory report export jobs lock should not be poisoned");
        let previous_state = jobs.get(&job.job_id).map(|existing| existing.status);
        if previous_state.is_none() && job.status != ReportingExportJobState::Queued {
            return Err(ReportExportWorkflowError::invalid_payload(
                "new export jobs must start in queued state",
                vec![ReportingExportValidationIssue {
                    field: "status",
                    code: ReportingExportReasonCode::InvalidPayload.code(),
                    message: "new export jobs must start in queued state".to_string(),
                }],
            ));
        }
        if let Some(previous_state) = previous_state {
            validate_export_job_transition(Some(previous_state), job.status)
                .map_err(map_contract_error)?;
        }
        jobs.insert(job.job_id.clone(), job.clone());
        Ok(job)
    }

    fn load_job_by_id(
        &self,
        job_id: &str,
    ) -> Result<Option<ExportJobRecord>, ReportExportWorkflowError> {
        let normalized = normalize_reporting_export_identifier(job_id);
        let jobs = self
            .jobs
            .lock()
            .expect("in-memory report export jobs lock should not be poisoned");
        Ok(jobs.get(&normalized).cloned())
    }

    fn load_job_by_weekly_binding(
        &self,
        schedule_id: &str,
        schedule_window_key: &str,
        report_run_id: &str,
    ) -> Result<Option<ExportJobRecord>, ReportExportWorkflowError> {
        let normalized_schedule_id = normalize_reporting_export_identifier(schedule_id);
        let normalized_window_key = normalize_reporting_export_identifier(schedule_window_key);
        let normalized_report_run_id = normalize_reporting_export_identifier(report_run_id);
        let jobs = self
            .jobs
            .lock()
            .expect("in-memory report export jobs lock should not be poisoned");
        let mut matching = jobs
            .values()
            .filter(|job| {
                job.trigger_source == ReportingExportTriggerSource::ScheduledWeekly
                    && job.schedule_id.as_deref() == Some(normalized_schedule_id.as_str())
                    && job.schedule_window_key.as_deref() == Some(normalized_window_key.as_str())
                    && job.report_run_id.as_deref() == Some(normalized_report_run_id.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        matching.sort_by(|left, right| {
            right
                .requested_at_utc
                .cmp(&left.requested_at_utc)
                .then_with(|| left.job_id.cmp(&right.job_id))
        });
        Ok(matching.into_iter().next())
    }

    fn upsert_artifact(
        &self,
        artifact: ExportArtifactRecord,
    ) -> Result<ExportArtifactRecord, ReportExportWorkflowError> {
        validate_export_artifact(&artifact).map_err(map_contract_error)?;
        let mut artifacts = self
            .artifacts
            .lock()
            .expect("in-memory report export artifacts lock should not be poisoned");
        artifacts.insert(artifact.artifact_id.clone(), artifact.clone());
        Ok(artifact)
    }

    fn load_artifacts_for_job(
        &self,
        job_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ExportArtifactRecord>, ReportExportWorkflowError> {
        let normalized_job_id = normalize_reporting_export_identifier(job_id);
        let normalized_limit = normalize_limit(limit)?;
        let artifacts = self
            .artifacts
            .lock()
            .expect("in-memory report export artifacts lock should not be poisoned");
        let mut matching = artifacts
            .values()
            .filter(|artifact| artifact.job_id == normalized_job_id)
            .cloned()
            .collect::<Vec<_>>();
        matching.sort_by(|left, right| {
            left.artifact_type
                .as_str()
                .cmp(right.artifact_type.as_str())
                .then_with(|| left.artifact_id.cmp(&right.artifact_id))
        });
        matching.truncate(normalized_limit as usize);
        Ok(matching)
    }

    fn load_artifact_by_id(
        &self,
        job_id: &str,
        artifact_id: &str,
    ) -> Result<Option<ExportArtifactRecord>, ReportExportWorkflowError> {
        let normalized_job_id = normalize_reporting_export_identifier(job_id);
        let normalized_artifact_id = normalize_reporting_export_identifier(artifact_id);
        let artifacts = self
            .artifacts
            .lock()
            .expect("in-memory report export artifacts lock should not be poisoned");
        Ok(artifacts
            .get(&normalized_artifact_id)
            .filter(|artifact| artifact.job_id == normalized_job_id)
            .cloned())
    }
}

fn to_job_evidence(
    job: ExportJobRecord,
    artifacts: Vec<ExportArtifactRecord>,
) -> ReportExportJobEvidence {
    let mut missing_artifact_types = artifacts
        .iter()
        .filter(|artifact| !artifact.is_available)
        .map(|artifact| artifact.artifact_type.as_str().to_string())
        .collect::<Vec<_>>();
    missing_artifact_types.sort();
    ReportExportJobEvidence {
        job_id: job.job_id,
        trigger_source: job.trigger_source.as_str().to_string(),
        status: job.status.as_str().to_string(),
        reason_code: job.reason_code,
        artifact_count: artifacts.len(),
        missing_artifact_types,
        package_reference: job.package_reference,
        package_checksum: job.package_checksum,
        correlation_id: job.correlation_id,
        requested_at_utc: job.requested_at_utc,
        started_at_utc: job.started_at_utc,
        finished_at_utc: job.finished_at_utc,
        updated_at_utc: job.updated_at_utc,
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ReportExportWorkflowError> {
    if !value.trim().is_empty() {
        return Ok(());
    }
    Err(ReportExportWorkflowError::invalid_payload(
        format!("{field} must not be empty"),
        vec![ReportingExportValidationIssue {
            field,
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: format!("{field} must not be empty"),
        }],
    ))
}

fn normalize_limit(limit: Option<i64>) -> Result<i64, ReportExportWorkflowError> {
    let value = limit.unwrap_or(DEFAULT_EXPORT_LIST_LIMIT);
    if !(1..=MAX_EXPORT_LIST_LIMIT).contains(&value) {
        return Err(ReportExportWorkflowError::invalid_payload(
            format!("limit must be between 1 and {MAX_EXPORT_LIST_LIMIT}"),
            vec![ReportingExportValidationIssue {
                field: "limit",
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("limit must be between 1 and {MAX_EXPORT_LIST_LIMIT}"),
            }],
        ));
    }
    Ok(value)
}

fn parse_optional_reason_code(
    value: Option<&str>,
    fallback: ReportingExportReasonCode,
) -> Result<ReportingExportReasonCode, ReportExportWorkflowError> {
    value
        .map(ReportingExportReasonCode::parse)
        .transpose()
        .map_err(map_contract_error)
        .map(|code| code.unwrap_or(fallback))
}

fn normalize_unavailable_artifact_types(
    candidates: &[String],
) -> Result<BTreeSet<ReportingExportArtifactType>, ReportExportWorkflowError> {
    let mut normalized = BTreeSet::new();
    for candidate in candidates {
        let artifact = ReportingExportArtifactType::parse(candidate).map_err(map_contract_error)?;
        normalized.insert(artifact);
    }
    Ok(normalized)
}

fn compose_export_job_id(
    trigger_source: ReportingExportTriggerSource,
    schedule_id: Option<&str>,
    schedule_window_key: Option<&str>,
    report_run_id: Option<&str>,
    correlation_id: &str,
    requested_at_utc: &str,
) -> String {
    let fingerprint = stable_fingerprint(&[
        trigger_source.as_str(),
        schedule_id.unwrap_or("none"),
        schedule_window_key.unwrap_or("none"),
        report_run_id.unwrap_or("none"),
        correlation_id,
        requested_at_utc,
    ]);
    format!("report-export::{fingerprint}")
}

fn compose_export_artifact_id(job_id: &str, artifact_type: ReportingExportArtifactType) -> String {
    let fingerprint = stable_fingerprint(&[job_id, artifact_type.as_str()]);
    format!("report-export-artifact::{fingerprint}")
}

fn stable_fingerprint(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    let digest = hasher.finalize();
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn emit_incident_export_failure_alert(
    job: &ExportJobRecord,
    impacted_system: &str,
    runbook_url: &str,
    emitted_at_utc: &str,
) -> Result<(), ReportExportWorkflowError> {
    let failed_at = parse_utc_timestamp(
        "finished_at_utc",
        job.finished_at_utc
            .as_deref()
            .unwrap_or(job.requested_at_utc.as_str()),
    )
    .map_err(map_contract_error)?;
    let emitted_at =
        parse_utc_timestamp("alert_emitted_at_utc", emitted_at_utc).map_err(map_contract_error)?;
    if emitted_at > failed_at + Duration::seconds(30) {
        return Err(ReportExportWorkflowError::stale_evidence(
            "incident export alert evidence exceeded 30 second SLA",
        ));
    }
    let event = IncidentExportFailureAlertEvent {
        event_name: "report_export_incident_failure_alert_v1",
        signal_name: "report_export_incident_failure_v1",
        alert_compatible: true,
        alert_target_seconds: 30,
        impacted_system,
        runbook_url,
        trigger_source: job.trigger_source.as_str(),
        reason_code: &job.reason_code,
        correlation_id: &job.correlation_id,
        timestamp_utc: emitted_at_utc,
    };
    println!(
        "{}",
        serde_json::to_string(&event)
            .expect("incident export failure alert event should always serialize")
    );
    Ok(())
}

fn emit_export_telemetry(event: ExportJobTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event).expect("report export telemetry should always serialize")
    );
}

fn map_contract_error(error: ReportingExportContractError) -> ReportExportWorkflowError {
    ReportExportWorkflowError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn map_persistence_error(error: ReportExportPersistenceError) -> ReportExportWorkflowError {
    match error.code {
        "report_export_query_failed" | "report_export_row_decode_failed" => {
            ReportExportWorkflowError::persistence_unavailable(error.message)
        }
        _ => ReportExportWorkflowError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

fn is_weekly_idempotency_collision(error: &ReportExportWorkflowError) -> bool {
    error.code == "report_export_constraint_violation"
        && error.message.contains("uq_export_jobs_weekly_idempotency")
}

#[derive(Debug, Clone, Serialize)]
struct ExportJobTelemetryEvent<'a> {
    event_name: &'a str,
    trigger_source: &'a str,
    status: &'a str,
    reason_code: &'a str,
    job_id: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct IncidentExportFailureAlertEvent<'a> {
    event_name: &'a str,
    signal_name: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u32,
    impacted_system: &'a str,
    runbook_url: &'a str,
    trigger_source: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_demand_input() -> TriggerOnDemandExportInput {
        TriggerOnDemandExportInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            correlation_id: "corr-export-001".to_string(),
            requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
            as_of_utc: "2026-04-07T00:00:00Z".to_string(),
            reason_code: None,
            unavailable_artifact_types: Vec::new(),
        }
    }

    #[test]
    fn on_demand_export_succeeds_with_required_artifact_coverage() {
        let service = ReportExportWorkflowService::in_memory();
        let evidence = service
            .trigger_on_demand_export(on_demand_input())
            .expect("on-demand export should succeed");
        assert_eq!(evidence.status, ReportingExportJobState::Succeeded.as_str());
        assert_eq!(
            evidence.reason_code,
            ReportingExportReasonCode::JobSucceeded.code()
        );
        assert_eq!(
            evidence.artifact_count,
            required_fr36_artifact_types().len()
        );
        assert!(evidence.missing_artifact_types.is_empty());

        let artifacts = service
            .list_export_artifacts(ListExportArtifactsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                job_id: evidence.job_id.clone(),
                correlation_id: "corr-export-read-001".to_string(),
                queried_at_utc: "2026-04-07T00:00:03Z".to_string(),
                limit: Some(50),
            })
            .expect("artifact listing should succeed");
        assert_eq!(artifacts.len(), required_fr36_artifact_types().len());
        assert!(artifacts.iter().all(|artifact| artifact.is_available));
    }

    #[test]
    fn weekly_dispatch_is_idempotent_per_schedule_window_and_report_run() {
        let service = ReportExportWorkflowService::in_memory();
        let input = DispatchWeeklyExportInput {
            actor_id: "scheduler".to_string(),
            actor_role: "operational_control".to_string(),
            schedule_id: "report-schedule-weekly".to_string(),
            schedule_window_key: "report-window::weekly::1712448000".to_string(),
            report_run_id: "report-run::weekly::abc123".to_string(),
            correlation_id: "corr-weekly-export-001".to_string(),
            requested_at_utc: "2026-04-13T00:00:05Z".to_string(),
            as_of_utc: "2026-04-13T00:00:05Z".to_string(),
            reason_code: None,
            unavailable_artifact_types: Vec::new(),
        };
        let first = service
            .dispatch_weekly_export(input.clone())
            .expect("first weekly dispatch should succeed");
        let second = service
            .dispatch_weekly_export(input)
            .expect("second weekly dispatch should reuse existing job");
        assert_eq!(first.job_id, second.job_id);
        assert_eq!(second.status, ReportingExportJobState::Succeeded.as_str());
    }

    #[test]
    fn unavailable_artifacts_fail_closed_and_mark_job_failed() {
        let service = ReportExportWorkflowService::in_memory();
        let mut input = on_demand_input();
        input.unavailable_artifact_types = vec!["access_audits".to_string()];
        let evidence = service
            .trigger_on_demand_export(input)
            .expect("failed export should still return deterministic evidence");
        assert_eq!(evidence.status, ReportingExportJobState::Failed.as_str());
        assert_eq!(
            evidence.reason_code,
            ReportingExportReasonCode::DependencyUnavailable.code()
        );
        assert_eq!(
            evidence.missing_artifact_types,
            vec!["access_audits".to_string()]
        );
    }

    #[test]
    fn incident_trigger_requires_context_and_emits_fail_closed_alert_evidence() {
        let service = ReportExportWorkflowService::in_memory();
        let missing_context = service
            .trigger_incident_export(TriggerIncidentExportInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                incident_id: "".to_string(),
                incident_severity: "".to_string(),
                correlation_id: "corr-incident-export-001".to_string(),
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                as_of_utc: "2026-04-07T00:00:00Z".to_string(),
                reason_code: None,
                unavailable_artifact_types: Vec::new(),
                impacted_system: Some("reporting export workflow".to_string()),
                runbook_url: Some(DEFAULT_INCIDENT_EXPORT_RUNBOOK.to_string()),
            })
            .expect_err("incident context is mandatory");
        assert_eq!(
            missing_context.code,
            ReportingExportReasonCode::MissingIncidentContext.code()
        );

        let failed = service
            .trigger_incident_export(TriggerIncidentExportInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                incident_id: "incident-42".to_string(),
                incident_severity: "severity_1".to_string(),
                correlation_id: "corr-incident-export-002".to_string(),
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                as_of_utc: "2026-04-07T00:00:00Z".to_string(),
                reason_code: None,
                unavailable_artifact_types: vec!["incident_postmortems".to_string()],
                impacted_system: Some("incident workflow".to_string()),
                runbook_url: Some(DEFAULT_INCIDENT_EXPORT_RUNBOOK.to_string()),
            })
            .expect("incident export should fail with deterministic evidence");
        assert_eq!(failed.status, ReportingExportJobState::Failed.as_str());
        assert_eq!(
            failed.reason_code,
            ReportingExportReasonCode::DependencyUnavailable.code()
        );
    }

    #[test]
    fn retrieval_is_fail_closed_for_unavailable_artifacts() {
        let service = ReportExportWorkflowService::in_memory();
        let mut input = on_demand_input();
        input.unavailable_artifact_types = vec!["promotion_decisions".to_string()];
        let evidence = service
            .trigger_on_demand_export(input)
            .expect("workflow should return failed job evidence");

        let artifacts = service
            .list_export_artifacts(ListExportArtifactsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                job_id: evidence.job_id.clone(),
                correlation_id: "corr-export-read-002".to_string(),
                queried_at_utc: "2026-04-07T00:00:03Z".to_string(),
                limit: Some(50),
            })
            .expect("artifact listing should work");
        let unavailable = artifacts
            .iter()
            .find(|artifact| !artifact.is_available)
            .expect("at least one unavailable artifact expected");

        let error = service
            .get_export_artifact(GetExportArtifactInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                job_id: evidence.job_id,
                artifact_id: unavailable.artifact_id.clone(),
                correlation_id: "corr-export-read-003".to_string(),
                queried_at_utc: "2026-04-07T00:00:05Z".to_string(),
            })
            .expect_err("unavailable artifact must fail closed");
        assert_eq!(
            error.code,
            ReportingExportReasonCode::ArtifactUnavailable.code()
        );
    }

    #[test]
    fn artifact_listing_returns_not_found_when_job_does_not_exist() {
        let service = ReportExportWorkflowService::in_memory();
        let error = service
            .list_export_artifacts(ListExportArtifactsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                job_id: "missing-job".to_string(),
                correlation_id: "corr-export-read-missing".to_string(),
                queried_at_utc: "2026-04-07T00:00:05Z".to_string(),
                limit: Some(25),
            })
            .expect_err("missing job must return not_found");
        assert_eq!(error.code, ReportingExportReasonCode::NotFound.code());
    }

    #[test]
    fn authorization_boundaries_enforce_trigger_vs_read_roles() {
        let service = ReportExportWorkflowService::in_memory();
        let trigger_error = service
            .trigger_on_demand_export(TriggerOnDemandExportInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                correlation_id: "corr-export-unauthorized-001".to_string(),
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                as_of_utc: "2026-04-07T00:00:00Z".to_string(),
                reason_code: None,
                unavailable_artifact_types: Vec::new(),
            })
            .expect_err("read-only roles must not trigger exports");
        assert_eq!(
            trigger_error.code,
            ReportingExportReasonCode::Unauthorized.code()
        );
    }

    #[test]
    fn repository_warmup_status_is_exposed() {
        let service = ReportExportWorkflowService::in_memory();
        assert_eq!(service.warmup_status(), "report-export-adapter-in-memory");
    }

    #[test]
    fn weekly_idempotency_collision_detection_is_precise() {
        let weekly_collision = ReportExportWorkflowError {
            code: "report_export_constraint_violation",
            message: "upsert_export_job rejected by constraint: uq_export_jobs_weekly_idempotency"
                .to_string(),
            field_errors: Vec::new(),
        };
        assert!(is_weekly_idempotency_collision(&weekly_collision));

        let unrelated_constraint = ReportExportWorkflowError {
            code: "report_export_constraint_violation",
            message: "upsert_export_job rejected by constraint: export_jobs_trigger_source_check"
                .to_string(),
            field_errors: Vec::new(),
        };
        assert!(!is_weekly_idempotency_collision(&unrelated_constraint));
    }

    #[test]
    fn format_utc_timestamp_roundtrip_is_stable() {
        let parsed =
            parse_utc_timestamp("now", "2026-04-07T00:00:00Z").expect("timestamp should parse");
        assert_eq!(
            domain::reporting_export::format_utc_timestamp(parsed),
            "2026-04-07T00:00:00Z"
        );
    }
}
