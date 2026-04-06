use crate::read_models::queries::{
    ReportingReadModelError, ReportingReadModelOrchestrator, ReportingReadRequest,
};
use common::time::timestamp_utc;
use domain::alerts::{
    AlertDeliveryAttempt, AlertDeliveryChannel, AlertDeliveryOutcome, AlertDispatchStatus,
    AlertReasonCode, AlertSeverity, IncidentAlert, compose_alert_identifier,
};
use domain::reporting::{DEFAULT_REPORTING_LIMIT, ReportingReasonCode as ReadModelReasonCode};
use domain::reporting_schedule::{
    DEFAULT_REPORT_RUN_HISTORY_LIMIT, MAX_REPORT_RUN_HISTORY_LIMIT, ReportRunRecord,
    ReportSchedule, ReportingCadence, ReportingRunState, ReportingScheduleContractError,
    ReportingScheduleReasonCode, ReportingScheduleState, ReportingScheduleValidationIssue,
    advance_to_next_run_at_utc, authorize_schedule_mutation, authorize_schedule_read,
    build_reporting_window_for_boundary, next_run_at_utc, normalize_reporting_schedule_identifier,
    validate_report_run, validate_report_schedule,
};
use persistence::postgres::incident_alerts::{
    AlertPersistenceError, append_alert_delivery_attempt, create_incident_alert,
};
use persistence::postgres::report_schedules::{
    ReportSchedulePersistenceError, load_due_report_schedules, load_report_run_by_window_key,
    load_report_run_history, load_report_schedule_by_id, upsert_report_run, upsert_report_schedule,
};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use time::Duration;

const DEFAULT_RUNBOOK_URL: &str = "https://docs.example.com/operations/recurring-report-scheduling";
const SCHEDULER_SOURCE_CONTEXT: &str = "reporting-service.scheduler";
const SCHEDULER_ACTOR_ID: &str = "scheduler";
const SCHEDULER_ACTOR_ROLE: &str = "operational_control";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportSchedulingServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ReportingScheduleValidationIssue>,
}

impl ReportSchedulingServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ReportingScheduleValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn alert_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::AlertUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ReportSchedulingServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReportSchedulingServiceError {}

#[derive(Debug, Clone)]
pub struct UpsertReportScheduleInput {
    pub actor_id: String,
    pub actor_role: String,
    pub schedule_id: String,
    pub cadence: String,
    pub reason_code: Option<String>,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub runbook_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PauseReportScheduleInput {
    pub actor_id: String,
    pub actor_role: String,
    pub schedule_id: String,
    pub reason_code: Option<String>,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[derive(Debug, Clone)]
pub struct ResumeReportScheduleInput {
    pub actor_id: String,
    pub actor_role: String,
    pub schedule_id: String,
    pub reason_code: Option<String>,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[derive(Debug, Clone)]
pub struct QueryReportRunHistoryInput {
    pub actor_id: String,
    pub actor_role: String,
    pub schedule_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportScheduleMutationEvidence {
    pub schedule_id: String,
    pub cadence: String,
    pub status: String,
    pub next_run_at_utc: String,
    pub actor_id: String,
    pub actor_role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub runbook_url: String,
    pub timestamp_utc: String,
}

pub trait ReportScheduleOrchestrator: Send + Sync {
    fn warmup_status(&self) -> &'static str;
    fn upsert_schedule(
        &self,
        input: UpsertReportScheduleInput,
    ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError>;
    fn pause_schedule(
        &self,
        input: PauseReportScheduleInput,
    ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError>;
    fn resume_schedule(
        &self,
        input: ResumeReportScheduleInput,
    ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError>;
    fn query_run_history(
        &self,
        input: QueryReportRunHistoryInput,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError>;
    fn process_due_schedules(
        &self,
        as_of_utc: &str,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError>;
}

trait ReportScheduleRepositoryPort: Send + Sync {
    fn warmup_status(&self) -> &'static str;
    fn upsert_schedule(
        &self,
        schedule: ReportSchedule,
    ) -> Result<ReportSchedule, ReportSchedulingServiceError>;
    fn load_schedule_by_id(
        &self,
        schedule_id: &str,
    ) -> Result<Option<ReportSchedule>, ReportSchedulingServiceError>;
    fn load_due_schedules(
        &self,
        as_of_utc: &str,
        limit: i64,
    ) -> Result<Vec<ReportSchedule>, ReportSchedulingServiceError>;
    fn upsert_run(
        &self,
        run: ReportRunRecord,
    ) -> Result<ReportRunRecord, ReportSchedulingServiceError>;
    fn load_run_by_window_key(
        &self,
        schedule_id: &str,
        window_key: &str,
    ) -> Result<Option<ReportRunRecord>, ReportSchedulingServiceError>;
    fn load_run_history(
        &self,
        schedule_id: &str,
        limit: i64,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError>;
}

trait ReportingSummaryPort: Send + Sync {
    fn hydrate_summary_window(
        &self,
        window_start_at_utc: &str,
        window_end_at_utc: &str,
        correlation_id: &str,
    ) -> Result<(), ReportSchedulingServiceError>;
}

trait ReportScheduleAlertPort: Send + Sync {
    fn emit_critical_failure_alert(
        &self,
        run: &ReportRunRecord,
        failure_code: &'static str,
        failure_message: &str,
    ) -> Result<(), ReportSchedulingServiceError>;
}

#[derive(Clone)]
pub struct ReportSchedulingService {
    repository: Arc<dyn ReportScheduleRepositoryPort>,
    summary_port: Arc<dyn ReportingSummaryPort>,
    alert_port: Arc<dyn ReportScheduleAlertPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl ReportSchedulingService {
    fn new(
        repository: Arc<dyn ReportScheduleRepositoryPort>,
        summary_port: Arc<dyn ReportingSummaryPort>,
        alert_port: Arc<dyn ReportScheduleAlertPort>,
    ) -> Self {
        Self {
            repository,
            summary_port,
            alert_port,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryReportScheduleRepository::default()),
            Arc::new(StubSummaryPort::default()),
            Arc::new(InMemoryAlertPort::default()),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        let read_model = ReportingReadModelOrchestrator::new(pool.clone());
        Self::new(
            Arc::new(PostgresReportScheduleRepository::new(pool.clone())),
            Arc::new(ReadModelSummaryPort::new(read_model)),
            Arc::new(PostgresAlertPort::new(pool)),
        )
    }

    pub fn bootstrap_from_env() -> Result<Option<Self>, ReportSchedulingServiceError> {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(error) => {
                return Err(ReportSchedulingServiceError::dependency_unavailable(
                    format!("failed to read DATABASE_URL: {error}"),
                ));
            }
        };

        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_lazy(&database_url)
            .map_err(|error| {
                ReportSchedulingServiceError::dependency_unavailable(format!(
                    "failed to initialize reporting schedule pool: {error}"
                ))
            })?;
        Ok(Some(Self::postgres(pool)))
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, ReportSchedulingServiceError> {
        self.operation_lock.lock().map_err(|_| {
            ReportSchedulingServiceError::persistence_unavailable(
                "report scheduling operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for ReportSchedulingService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ReportScheduleOrchestrator for ReportSchedulingService {
    fn warmup_status(&self) -> &'static str {
        self.repository.warmup_status()
    }

    fn upsert_schedule(
        &self,
        input: UpsertReportScheduleInput,
    ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError> {
        authorize_schedule_mutation(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("timestamp_utc", &input.timestamp_utc)?;

        let schedule_id = normalize_reporting_schedule_identifier(&input.schedule_id);
        validate_non_empty("schedule_id", &schedule_id)?;
        let cadence = ReportingCadence::parse(&input.cadence).map_err(map_contract_error)?;
        let reason_code = input
            .reason_code
            .as_deref()
            .map(ReportingScheduleReasonCode::parse)
            .transpose()
            .map_err(map_contract_error)?
            .unwrap_or(ReportingScheduleReasonCode::Ready)
            .code()
            .to_string();
        let runbook_url = input
            .runbook_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_RUNBOOK_URL)
            .to_string();

        let _lock = self.lock_operations()?;
        let existing = self.repository.load_schedule_by_id(&schedule_id)?;
        let created_at_utc = existing
            .as_ref()
            .map(|schedule| schedule.created_at_utc.clone())
            .unwrap_or_else(|| input.timestamp_utc.clone());
        let next_run_at_utc =
            next_run_at_utc(cadence, &input.timestamp_utc).map_err(map_contract_error)?;
        let schedule = ReportSchedule {
            schedule_id,
            cadence,
            status: ReportingScheduleState::Active,
            next_run_at_utc,
            actor_id: normalize_reporting_schedule_identifier(&input.actor_id),
            actor_role: normalize_reporting_schedule_identifier(&input.actor_role),
            reason_code,
            correlation_id: normalize_reporting_schedule_identifier(&input.correlation_id),
            runbook_url,
            created_at_utc,
            updated_at_utc: input.timestamp_utc.clone(),
            paused_at_utc: None,
        };
        let saved = self.repository.upsert_schedule(schedule)?;
        emit_schedule_telemetry(ScheduleTelemetryEvent {
            event_name: "report_schedule_upsert_v1",
            actor_id: &saved.actor_id,
            actor_role: &saved.actor_role,
            schedule_id: &saved.schedule_id,
            cadence: saved.cadence.as_str(),
            status: saved.status.as_str(),
            reason_code: &saved.reason_code,
            correlation_id: &saved.correlation_id,
            timestamp_utc: &saved.updated_at_utc,
        });
        Ok(to_schedule_mutation(saved))
    }

    fn pause_schedule(
        &self,
        input: PauseReportScheduleInput,
    ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError> {
        authorize_schedule_mutation(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("timestamp_utc", &input.timestamp_utc)?;

        let schedule_id = normalize_reporting_schedule_identifier(&input.schedule_id);
        validate_non_empty("schedule_id", &schedule_id)?;
        let reason_code = input
            .reason_code
            .as_deref()
            .map(ReportingScheduleReasonCode::parse)
            .transpose()
            .map_err(map_contract_error)?
            .unwrap_or(ReportingScheduleReasonCode::SchedulePaused)
            .code()
            .to_string();

        let _lock = self.lock_operations()?;
        let Some(mut schedule) = self.repository.load_schedule_by_id(&schedule_id)? else {
            return Err(ReportSchedulingServiceError::not_found(format!(
                "report schedule `{schedule_id}` not found"
            )));
        };
        schedule.status = ReportingScheduleState::Paused;
        schedule.reason_code = reason_code;
        schedule.actor_id = normalize_reporting_schedule_identifier(&input.actor_id);
        schedule.actor_role = normalize_reporting_schedule_identifier(&input.actor_role);
        schedule.correlation_id = normalize_reporting_schedule_identifier(&input.correlation_id);
        schedule.updated_at_utc = input.timestamp_utc.clone();
        schedule.paused_at_utc = Some(input.timestamp_utc);
        let saved = self.repository.upsert_schedule(schedule)?;
        emit_schedule_telemetry(ScheduleTelemetryEvent {
            event_name: "report_schedule_pause_v1",
            actor_id: &saved.actor_id,
            actor_role: &saved.actor_role,
            schedule_id: &saved.schedule_id,
            cadence: saved.cadence.as_str(),
            status: saved.status.as_str(),
            reason_code: &saved.reason_code,
            correlation_id: &saved.correlation_id,
            timestamp_utc: &saved.updated_at_utc,
        });
        Ok(to_schedule_mutation(saved))
    }

    fn resume_schedule(
        &self,
        input: ResumeReportScheduleInput,
    ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError> {
        authorize_schedule_mutation(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("timestamp_utc", &input.timestamp_utc)?;

        let schedule_id = normalize_reporting_schedule_identifier(&input.schedule_id);
        validate_non_empty("schedule_id", &schedule_id)?;
        let reason_code = input
            .reason_code
            .as_deref()
            .map(ReportingScheduleReasonCode::parse)
            .transpose()
            .map_err(map_contract_error)?
            .unwrap_or(ReportingScheduleReasonCode::ScheduleResumed)
            .code()
            .to_string();

        let _lock = self.lock_operations()?;
        let Some(mut schedule) = self.repository.load_schedule_by_id(&schedule_id)? else {
            return Err(ReportSchedulingServiceError::not_found(format!(
                "report schedule `{schedule_id}` not found"
            )));
        };
        if schedule.status != ReportingScheduleState::Paused {
            return Err(ReportSchedulingServiceError::invalid_payload(
                "resume is only allowed for paused schedules",
                vec![ReportingScheduleValidationIssue {
                    field: "status",
                    code: ReportingScheduleReasonCode::InvalidPayload.code(),
                    message: "resume is only allowed for paused schedules".to_string(),
                }],
            ));
        }
        schedule.status = ReportingScheduleState::Active;
        schedule.reason_code = reason_code;
        schedule.actor_id = normalize_reporting_schedule_identifier(&input.actor_id);
        schedule.actor_role = normalize_reporting_schedule_identifier(&input.actor_role);
        schedule.correlation_id = normalize_reporting_schedule_identifier(&input.correlation_id);
        schedule.updated_at_utc = input.timestamp_utc.clone();
        schedule.paused_at_utc = None;
        schedule.next_run_at_utc =
            next_run_at_utc(schedule.cadence, &input.timestamp_utc).map_err(map_contract_error)?;
        let saved = self.repository.upsert_schedule(schedule)?;
        emit_schedule_telemetry(ScheduleTelemetryEvent {
            event_name: "report_schedule_resume_v1",
            actor_id: &saved.actor_id,
            actor_role: &saved.actor_role,
            schedule_id: &saved.schedule_id,
            cadence: saved.cadence.as_str(),
            status: saved.status.as_str(),
            reason_code: &saved.reason_code,
            correlation_id: &saved.correlation_id,
            timestamp_utc: &saved.updated_at_utc,
        });
        Ok(to_schedule_mutation(saved))
    }

    fn query_run_history(
        &self,
        input: QueryReportRunHistoryInput,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError> {
        authorize_schedule_read(&input.actor_role).map_err(map_contract_error)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        let schedule_id = normalize_reporting_schedule_identifier(&input.schedule_id);
        validate_non_empty("schedule_id", &schedule_id)?;
        let limit = normalize_history_limit(input.limit)?;

        let runs = self.repository.load_run_history(&schedule_id, limit)?;
        emit_schedule_telemetry(ScheduleTelemetryEvent {
            event_name: "report_schedule_run_history_query_v1",
            actor_id: &normalize_reporting_schedule_identifier(&input.actor_id),
            actor_role: &normalize_reporting_schedule_identifier(&input.actor_role),
            schedule_id: &schedule_id,
            cadence: "mixed",
            status: "query",
            reason_code: ReportingScheduleReasonCode::Ready.code(),
            correlation_id: &normalize_reporting_schedule_identifier(&input.correlation_id),
            timestamp_utc: &input.queried_at_utc,
        });
        Ok(runs)
    }

    fn process_due_schedules(
        &self,
        as_of_utc: &str,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError> {
        let _lock = self.lock_operations()?;
        let schedules = self.repository.load_due_schedules(as_of_utc, 64)?;
        let mut processed_runs = Vec::with_capacity(schedules.len());

        for mut schedule in schedules {
            let window = build_reporting_window_for_boundary(
                &schedule.schedule_id,
                schedule.cadence,
                &schedule.next_run_at_utc,
            )
            .map_err(map_contract_error)?;
            let now = as_of_utc.trim().to_string();
            let mut run = if let Some(mut existing) = self
                .repository
                .load_run_by_window_key(&schedule.schedule_id, &window.window_key)?
            {
                if run_requires_alert_retry(&existing) {
                    match self.alert_port.emit_critical_failure_alert(
                        &existing,
                        ReportingScheduleReasonCode::AlertUnavailable.code(),
                        "retrying missing critical alert evidence for failed report run",
                    ) {
                        Ok(()) => {
                            if let Err(sla_error) =
                                record_alert_emission_within_sla(&mut existing, now.clone())
                            {
                                existing.updated_at_utc = now.clone();
                                let _ = self.repository.upsert_run(existing);
                                return Err(sla_error);
                            }
                            existing.updated_at_utc = now.clone();
                            existing = self.repository.upsert_run(existing)?;
                        }
                        Err(alert_error) => {
                            existing.updated_at_utc = now.clone();
                            let _ = self.repository.upsert_run(existing);
                            return Err(alert_error);
                        }
                    }
                }

                if matches!(
                    existing.status,
                    ReportingRunState::Succeeded
                        | ReportingRunState::Failed
                        | ReportingRunState::Missed
                ) {
                    schedule.next_run_at_utc =
                        advance_to_next_run_at_utc(schedule.cadence, &schedule.next_run_at_utc)
                            .map_err(map_contract_error)?;
                    schedule.reason_code = ReportingScheduleReasonCode::RunDuplicateSuppressed
                        .code()
                        .to_string();
                    schedule.actor_id = SCHEDULER_ACTOR_ID.to_string();
                    schedule.actor_role = SCHEDULER_ACTOR_ROLE.to_string();
                    schedule.updated_at_utc = now.clone();
                    schedule.paused_at_utc = None;
                    self.repository.upsert_schedule(schedule.clone())?;

                    emit_schedule_telemetry(ScheduleTelemetryEvent {
                        event_name: "report_schedule_run_duplicate_v1",
                        actor_id: &schedule.actor_id,
                        actor_role: &schedule.actor_role,
                        schedule_id: &schedule.schedule_id,
                        cadence: schedule.cadence.as_str(),
                        status: existing.status.as_str(),
                        reason_code: ReportingScheduleReasonCode::RunDuplicateSuppressed.code(),
                        correlation_id: &existing.correlation_id,
                        timestamp_utc: &now,
                    });
                    processed_runs.push(existing);
                    continue;
                }

                emit_schedule_telemetry(ScheduleTelemetryEvent {
                    event_name: "report_schedule_run_replay_v1",
                    actor_id: &schedule.actor_id,
                    actor_role: &schedule.actor_role,
                    schedule_id: &schedule.schedule_id,
                    cadence: schedule.cadence.as_str(),
                    status: existing.status.as_str(),
                    reason_code: &existing.reason_code,
                    correlation_id: &existing.correlation_id,
                    timestamp_utc: &now,
                });
                existing
            } else {
                let run_id = compose_report_run_id(
                    &schedule.schedule_id,
                    schedule.cadence,
                    &window.window_key,
                )?;
                let run = ReportRunRecord {
                    run_id,
                    schedule_id: schedule.schedule_id.clone(),
                    cadence: schedule.cadence,
                    window_key: window.window_key.clone(),
                    window_started_at_utc: window.window_start_at_utc.clone(),
                    window_ended_at_utc: window.window_end_at_utc.clone(),
                    status: ReportingRunState::Pending,
                    reason_code: ReportingScheduleReasonCode::Ready.code().to_string(),
                    correlation_id: schedule.correlation_id.clone(),
                    source_context: SCHEDULER_SOURCE_CONTEXT.to_string(),
                    actor_id: Some(SCHEDULER_ACTOR_ID.to_string()),
                    run_started_at_utc: now.clone(),
                    run_finished_at_utc: None,
                    alert_emitted_at_utc: None,
                    runbook_url: Some(schedule.runbook_url.clone()),
                    impacted_system: Some("reporting-service scheduler".to_string()),
                    created_at_utc: now.clone(),
                    updated_at_utc: now.clone(),
                };
                self.repository.upsert_run(run)?
            };

            let run_boundary = domain::reporting_schedule::parse_utc_timestamp(
                "window_ended_at_utc",
                &run.window_ended_at_utc,
            )
            .map_err(map_contract_error)?;
            let now_timestamp = domain::reporting_schedule::parse_utc_timestamp("as_of_utc", &now)
                .map_err(map_contract_error)?;

            if now_timestamp > run_boundary + Duration::seconds(30) {
                run.status = ReportingRunState::Missed;
                run.reason_code = ReportingScheduleReasonCode::RunMissed.code().to_string();
                run.run_finished_at_utc = Some(now.clone());
                run.updated_at_utc = now.clone();
                run = self.repository.upsert_run(run)?;
            } else {
                run.status = ReportingRunState::Running;
                run.updated_at_utc = now.clone();
                run = self.repository.upsert_run(run)?;

                match self.summary_port.hydrate_summary_window(
                    &run.window_started_at_utc,
                    &run.window_ended_at_utc,
                    &run.correlation_id,
                ) {
                    Ok(()) => {
                        run.status = ReportingRunState::Succeeded;
                        run.reason_code =
                            ReportingScheduleReasonCode::RunSucceeded.code().to_string();
                    }
                    Err(error) => {
                        run.status = ReportingRunState::Failed;
                        run.reason_code = map_failure_reason_code(error.code).to_string();
                        run.run_finished_at_utc = Some(now.clone());
                        run.updated_at_utc = now.clone();
                        run = self.repository.upsert_run(run)?;

                        if matches!(
                            error.code,
                            code if code == ReportingScheduleReasonCode::DependencyUnavailable.code()
                                || code == ReportingScheduleReasonCode::StaleEvidence.code()
                                || code == ReportingScheduleReasonCode::PersistenceUnavailable.code()
                        ) {
                            match self.alert_port.emit_critical_failure_alert(
                                &run,
                                error.code,
                                &error.message,
                            ) {
                                Ok(()) => {
                                    if let Err(sla_error) =
                                        record_alert_emission_within_sla(&mut run, now.clone())
                                    {
                                        run.reason_code =
                                            ReportingScheduleReasonCode::AlertUnavailable
                                                .code()
                                                .to_string();
                                        run.updated_at_utc = now.clone();
                                        let _ = self.repository.upsert_run(run.clone());
                                        return Err(sla_error);
                                    }
                                }
                                Err(alert_error) => {
                                    run.reason_code = ReportingScheduleReasonCode::AlertUnavailable
                                        .code()
                                        .to_string();
                                    run.updated_at_utc = now.clone();
                                    let _ = self.repository.upsert_run(run.clone());
                                    return Err(alert_error);
                                }
                            }
                        }
                    }
                }
            }

            run.run_finished_at_utc = run.run_finished_at_utc.or_else(|| Some(now.clone()));
            run.updated_at_utc = now.clone();
            run = self.repository.upsert_run(run)?;

            schedule.next_run_at_utc =
                advance_to_next_run_at_utc(schedule.cadence, &schedule.next_run_at_utc)
                    .map_err(map_contract_error)?;
            schedule.status = ReportingScheduleState::Active;
            schedule.reason_code = run.reason_code.clone();
            schedule.actor_id = SCHEDULER_ACTOR_ID.to_string();
            schedule.actor_role = SCHEDULER_ACTOR_ROLE.to_string();
            schedule.updated_at_utc = now.clone();
            schedule.paused_at_utc = None;
            self.repository.upsert_schedule(schedule.clone())?;

            emit_schedule_telemetry(ScheduleTelemetryEvent {
                event_name: "report_schedule_run_transition_v1",
                actor_id: SCHEDULER_ACTOR_ID,
                actor_role: SCHEDULER_ACTOR_ROLE,
                schedule_id: &schedule.schedule_id,
                cadence: schedule.cadence.as_str(),
                status: run.status.as_str(),
                reason_code: &run.reason_code,
                correlation_id: &run.correlation_id,
                timestamp_utc: &now,
            });

            processed_runs.push(run);
        }

        Ok(processed_runs)
    }
}

#[derive(Debug, Clone, Serialize)]
struct ScheduleTelemetryEvent<'a> {
    event_name: &'a str,
    actor_id: &'a str,
    actor_role: &'a str,
    schedule_id: &'a str,
    cadence: &'a str,
    status: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
}

fn emit_schedule_telemetry(event: ScheduleTelemetryEvent<'_>) {
    match serde_json::to_string(&event) {
        Ok(serialized) => eprintln!("{serialized}"),
        Err(error) => eprintln!("failed to serialize report scheduling telemetry: {error}"),
    }
}

fn to_schedule_mutation(schedule: ReportSchedule) -> ReportScheduleMutationEvidence {
    ReportScheduleMutationEvidence {
        schedule_id: schedule.schedule_id,
        cadence: schedule.cadence.as_str().to_string(),
        status: schedule.status.as_str().to_string(),
        next_run_at_utc: schedule.next_run_at_utc,
        actor_id: schedule.actor_id,
        actor_role: schedule.actor_role,
        reason_code: schedule.reason_code,
        correlation_id: schedule.correlation_id,
        runbook_url: schedule.runbook_url,
        timestamp_utc: schedule.updated_at_utc,
    }
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ReportSchedulingServiceError> {
    if !value.trim().is_empty() {
        return Ok(());
    }
    Err(ReportSchedulingServiceError::invalid_payload(
        format!("{field} must not be empty"),
        vec![ReportingScheduleValidationIssue {
            field,
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: format!("{field} must not be empty"),
        }],
    ))
}

fn normalize_history_limit(limit: Option<i64>) -> Result<i64, ReportSchedulingServiceError> {
    match limit {
        None => Ok(DEFAULT_REPORT_RUN_HISTORY_LIMIT),
        Some(value) if (1..=MAX_REPORT_RUN_HISTORY_LIMIT).contains(&value) => Ok(value),
        Some(_) => Err(ReportSchedulingServiceError::invalid_payload(
            format!("limit must be between 1 and {MAX_REPORT_RUN_HISTORY_LIMIT}"),
            vec![ReportingScheduleValidationIssue {
                field: "limit",
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: format!("limit must be between 1 and {MAX_REPORT_RUN_HISTORY_LIMIT}"),
            }],
        )),
    }
}

fn compose_report_run_id(
    schedule_id: &str,
    cadence: ReportingCadence,
    window_key: &str,
) -> Result<String, ReportSchedulingServiceError> {
    validate_non_empty("schedule_id", schedule_id)?;
    let normalized_window = normalize_reporting_schedule_identifier(window_key);
    validate_non_empty("window_key", &normalized_window)?;
    let run_key_fingerprint = deterministic_run_key_fingerprint(&normalized_window);
    let run_id = format!("report-run::{}::{run_key_fingerprint}", cadence.as_str());
    validate_non_empty("run_id", &run_id)?;
    Ok(run_id)
}

fn deterministic_run_key_fingerprint(normalized_window_key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalized_window_key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn run_requires_alert_retry(run: &ReportRunRecord) -> bool {
    run.status == ReportingRunState::Failed
        && run.reason_code == ReportingScheduleReasonCode::AlertUnavailable.code()
        && run.alert_emitted_at_utc.is_none()
}

fn record_alert_emission_within_sla(
    run: &mut ReportRunRecord,
    emitted_at_utc: String,
) -> Result<(), ReportSchedulingServiceError> {
    let failure_time = run
        .run_finished_at_utc
        .as_deref()
        .unwrap_or(run.run_started_at_utc.as_str());
    let failed_at =
        domain::reporting_schedule::parse_utc_timestamp("run_finished_at_utc", failure_time)
            .map_err(map_contract_error)?;
    let emitted_at =
        domain::reporting_schedule::parse_utc_timestamp("alert_emitted_at_utc", &emitted_at_utc)
            .map_err(map_contract_error)?;
    if emitted_at > failed_at + Duration::seconds(30) {
        return Err(ReportSchedulingServiceError::alert_unavailable(
            "critical schedule alert evidence exceeded 30 second SLA",
        ));
    }
    run.alert_emitted_at_utc = Some(emitted_at_utc);
    Ok(())
}

fn map_failure_reason_code(error_code: &str) -> &'static str {
    match error_code {
        code if code == ReportingScheduleReasonCode::DependencyUnavailable.code() => {
            ReportingScheduleReasonCode::DependencyUnavailable.code()
        }
        code if code == ReportingScheduleReasonCode::StaleEvidence.code() => {
            ReportingScheduleReasonCode::StaleEvidence.code()
        }
        code if code == ReportingScheduleReasonCode::PersistenceUnavailable.code() => {
            ReportingScheduleReasonCode::PersistenceUnavailable.code()
        }
        code if code == ReportingScheduleReasonCode::AlertUnavailable.code() => {
            ReportingScheduleReasonCode::AlertUnavailable.code()
        }
        _ => ReportingScheduleReasonCode::RunFailed.code(),
    }
}

fn map_contract_error(error: ReportingScheduleContractError) -> ReportSchedulingServiceError {
    ReportSchedulingServiceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn map_persistence_error(error: ReportSchedulePersistenceError) -> ReportSchedulingServiceError {
    match error.code {
        "report_schedule_query_failed" | "report_schedule_row_decode_failed" => {
            ReportSchedulingServiceError::persistence_unavailable(error.message)
        }
        _ => ReportSchedulingServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

fn map_alert_persistence_error(error: AlertPersistenceError) -> ReportSchedulingServiceError {
    match error.code {
        "alert_query_failed" | "alert_row_decode_failed" => {
            ReportSchedulingServiceError::alert_unavailable(error.message)
        }
        "alert_constraint_violation" => ReportSchedulingServiceError::alert_unavailable(format!(
            "alert evidence could not be persisted deterministically: {}",
            error.message
        )),
        _ => ReportSchedulingServiceError {
            code: error.code,
            message: error.message,
            field_errors: error
                .field_errors
                .into_iter()
                .map(|issue| ReportingScheduleValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        },
    }
}

fn map_read_model_error(error: ReportingReadModelError) -> ReportSchedulingServiceError {
    match error.code {
        code if code == ReadModelReasonCode::DependencyUnavailable.code() => {
            ReportSchedulingServiceError::dependency_unavailable(error.message)
        }
        code if code == ReadModelReasonCode::StaleDependency.code() => {
            ReportSchedulingServiceError::stale_evidence(error.message)
        }
        code if code == ReadModelReasonCode::PersistenceUnavailable.code() => {
            ReportSchedulingServiceError::persistence_unavailable(error.message)
        }
        code if code == ReadModelReasonCode::InvalidPayload.code() => {
            ReportSchedulingServiceError::invalid_payload(
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| ReportingScheduleValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
            )
        }
        code if code == ReadModelReasonCode::Unauthorized.code() => {
            ReportSchedulingServiceError::unauthorized(error.message)
        }
        code if code == ReadModelReasonCode::EvidenceUnavailable.code() => {
            ReportSchedulingServiceError::stale_evidence(error.message)
        }
        _ => ReportSchedulingServiceError::dependency_unavailable(error.message),
    }
}

#[derive(Debug, Clone)]
struct PostgresReportScheduleRepository {
    pool: PgPool,
}

impl PostgresReportScheduleRepository {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, ReportSchedulingServiceError>
    where
        F: Future<Output = Result<T, ReportSchedulePersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ReportSchedulingServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl ReportScheduleRepositoryPort for PostgresReportScheduleRepository {
    fn warmup_status(&self) -> &'static str {
        "report-schedule-adapter-initialized"
    }

    fn upsert_schedule(
        &self,
        schedule: ReportSchedule,
    ) -> Result<ReportSchedule, ReportSchedulingServiceError> {
        self.run_with_runtime(upsert_report_schedule(&self.pool, &schedule))
    }

    fn load_schedule_by_id(
        &self,
        schedule_id: &str,
    ) -> Result<Option<ReportSchedule>, ReportSchedulingServiceError> {
        self.run_with_runtime(load_report_schedule_by_id(&self.pool, schedule_id))
    }

    fn load_due_schedules(
        &self,
        as_of_utc: &str,
        limit: i64,
    ) -> Result<Vec<ReportSchedule>, ReportSchedulingServiceError> {
        self.run_with_runtime(load_due_report_schedules(&self.pool, as_of_utc, limit))
    }

    fn upsert_run(
        &self,
        run: ReportRunRecord,
    ) -> Result<ReportRunRecord, ReportSchedulingServiceError> {
        self.run_with_runtime(upsert_report_run(&self.pool, &run))
    }

    fn load_run_by_window_key(
        &self,
        schedule_id: &str,
        window_key: &str,
    ) -> Result<Option<ReportRunRecord>, ReportSchedulingServiceError> {
        self.run_with_runtime(load_report_run_by_window_key(
            &self.pool,
            schedule_id,
            window_key,
        ))
    }

    fn load_run_history(
        &self,
        schedule_id: &str,
        limit: i64,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError> {
        self.run_with_runtime(load_report_run_history(&self.pool, schedule_id, limit))
    }
}

#[derive(Debug, Default)]
struct InMemoryReportScheduleRepository {
    schedules: Mutex<BTreeMap<String, ReportSchedule>>,
    runs: Mutex<BTreeMap<String, ReportRunRecord>>,
}

impl ReportScheduleRepositoryPort for InMemoryReportScheduleRepository {
    fn warmup_status(&self) -> &'static str {
        "report-schedule-adapter-in-memory"
    }

    fn upsert_schedule(
        &self,
        schedule: ReportSchedule,
    ) -> Result<ReportSchedule, ReportSchedulingServiceError> {
        validate_report_schedule(&schedule).map_err(map_contract_error)?;
        let mut schedules = self
            .schedules
            .lock()
            .expect("in-memory report schedule lock should not be poisoned");
        schedules.insert(schedule.schedule_id.clone(), schedule.clone());
        Ok(schedule)
    }

    fn load_schedule_by_id(
        &self,
        schedule_id: &str,
    ) -> Result<Option<ReportSchedule>, ReportSchedulingServiceError> {
        let normalized = normalize_reporting_schedule_identifier(schedule_id);
        let schedules = self
            .schedules
            .lock()
            .expect("in-memory report schedule lock should not be poisoned");
        Ok(schedules.get(&normalized).cloned())
    }

    fn load_due_schedules(
        &self,
        as_of_utc: &str,
        limit: i64,
    ) -> Result<Vec<ReportSchedule>, ReportSchedulingServiceError> {
        let now = domain::reporting_schedule::parse_utc_timestamp("as_of_utc", as_of_utc)
            .map_err(map_contract_error)?;
        let schedules = self
            .schedules
            .lock()
            .expect("in-memory report schedule lock should not be poisoned");
        let mut due = schedules
            .values()
            .filter_map(|schedule| {
                if schedule.status != ReportingScheduleState::Active {
                    return None;
                }
                let next = domain::reporting_schedule::parse_utc_timestamp(
                    "next_run_at_utc",
                    &schedule.next_run_at_utc,
                )
                .ok()?;
                if next <= now {
                    Some(schedule.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        due.sort_by(|left, right| {
            left.next_run_at_utc
                .cmp(&right.next_run_at_utc)
                .then_with(|| left.schedule_id.cmp(&right.schedule_id))
        });
        due.truncate(limit.clamp(1, 200) as usize);
        Ok(due)
    }

    fn upsert_run(
        &self,
        run: ReportRunRecord,
    ) -> Result<ReportRunRecord, ReportSchedulingServiceError> {
        validate_report_run(&run).map_err(map_contract_error)?;
        let mut runs = self
            .runs
            .lock()
            .expect("in-memory report run lock should not be poisoned");
        if let Some(existing) = runs.values().find(|existing| {
            existing.schedule_id == run.schedule_id
                && existing.window_key == run.window_key
                && existing.run_id != run.run_id
        }) {
            return Err(ReportSchedulingServiceError {
                code: "report_schedule_constraint_violation",
                message: format!(
                    "window key already exists for schedule `{}` (run `{}`)",
                    existing.schedule_id, existing.run_id
                ),
                field_errors: Vec::new(),
            });
        }
        runs.insert(run.run_id.clone(), run.clone());
        Ok(run)
    }

    fn load_run_by_window_key(
        &self,
        schedule_id: &str,
        window_key: &str,
    ) -> Result<Option<ReportRunRecord>, ReportSchedulingServiceError> {
        let normalized_schedule = normalize_reporting_schedule_identifier(schedule_id);
        let normalized_window = normalize_reporting_schedule_identifier(window_key);
        let runs = self
            .runs
            .lock()
            .expect("in-memory report run lock should not be poisoned");
        Ok(runs
            .values()
            .find(|run| {
                run.schedule_id == normalized_schedule && run.window_key == normalized_window
            })
            .cloned())
    }

    fn load_run_history(
        &self,
        schedule_id: &str,
        limit: i64,
    ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError> {
        let normalized_schedule = normalize_reporting_schedule_identifier(schedule_id);
        let mut history = self
            .runs
            .lock()
            .expect("in-memory report run lock should not be poisoned")
            .values()
            .filter(|run| run.schedule_id == normalized_schedule)
            .cloned()
            .collect::<Vec<_>>();
        history.sort_by(|left, right| {
            right
                .window_started_at_utc
                .cmp(&left.window_started_at_utc)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        history.truncate(limit.clamp(1, MAX_REPORT_RUN_HISTORY_LIMIT) as usize);
        Ok(history)
    }
}

#[derive(Debug, Clone)]
struct ReadModelSummaryPort {
    orchestrator: ReportingReadModelOrchestrator,
}

impl ReadModelSummaryPort {
    fn new(orchestrator: ReportingReadModelOrchestrator) -> Self {
        Self { orchestrator }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, ReportSchedulingServiceError>
    where
        F: Future<Output = Result<T, ReportingReadModelError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_read_model_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ReportSchedulingServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_read_model_error),
        }
    }
}

impl ReportingSummaryPort for ReadModelSummaryPort {
    fn hydrate_summary_window(
        &self,
        window_start_at_utc: &str,
        window_end_at_utc: &str,
        correlation_id: &str,
    ) -> Result<(), ReportSchedulingServiceError> {
        let request = ReportingReadRequest {
            actor_role: "read_only_analytics".to_string(),
            start_inclusive_utc: window_start_at_utc.to_string(),
            end_exclusive_utc: window_end_at_utc.to_string(),
            market_id: None,
            alpha_id: None,
            correlation_id: Some(correlation_id.to_string()),
            limit: DEFAULT_REPORTING_LIMIT,
        };
        self.run_with_runtime(async {
            self.orchestrator.query_trade_rows(&request).await?;
            self.orchestrator.query_position_rows(&request).await?;
            self.orchestrator.query_risk_event_rows(&request).await?;
            self.orchestrator.query_performance_rows(&request).await?;
            Ok(())
        })?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct StubSummaryPort {
    fail_with: Option<(&'static str, &'static str)>,
}

impl ReportingSummaryPort for StubSummaryPort {
    fn hydrate_summary_window(
        &self,
        _window_start_at_utc: &str,
        _window_end_at_utc: &str,
        _correlation_id: &str,
    ) -> Result<(), ReportSchedulingServiceError> {
        if let Some((code, message)) = self.fail_with {
            return Err(ReportSchedulingServiceError {
                code,
                message: message.to_string(),
                field_errors: Vec::new(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PostgresAlertPort {
    pool: PgPool,
}

impl PostgresAlertPort {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, ReportSchedulingServiceError>
    where
        F: Future<Output = Result<T, AlertPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_alert_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ReportSchedulingServiceError::alert_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_alert_persistence_error),
        }
    }
}

impl ReportScheduleAlertPort for PostgresAlertPort {
    fn emit_critical_failure_alert(
        &self,
        run: &ReportRunRecord,
        failure_code: &'static str,
        failure_message: &str,
    ) -> Result<(), ReportSchedulingServiceError> {
        let issued_at = run
            .run_finished_at_utc
            .as_deref()
            .unwrap_or(run.run_started_at_utc.as_str())
            .to_string();
        let reason = match failure_code {
            code if code == ReportingScheduleReasonCode::DependencyUnavailable.code() => {
                AlertReasonCode::DependencyUnavailable
            }
            code if code == ReportingScheduleReasonCode::StaleEvidence.code() => {
                AlertReasonCode::StaleEvidence
            }
            code if code == ReportingScheduleReasonCode::PersistenceUnavailable.code() => {
                AlertReasonCode::DependencyUnavailable
            }
            _ => AlertReasonCode::DependencyUnavailable,
        };
        let alert_id = compose_alert_identifier(&run.correlation_id, reason, &issued_at)
            .map_err(|error| ReportSchedulingServiceError::alert_unavailable(error.message))?;
        let alert = IncidentAlert {
            alert_id: alert_id.clone(),
            severity: AlertSeverity::Critical,
            impacted_subsystem: "reporting-service scheduler".to_string(),
            cause: failure_message.trim().to_string(),
            recommended_next_action:
                "Follow recurring report scheduling runbook and restore scheduler dependencies."
                    .to_string(),
            evidence_link: run
                .runbook_url
                .clone()
                .unwrap_or_else(|| DEFAULT_RUNBOOK_URL.to_string()),
            issued_at: issued_at.clone(),
            correlation_id: run.correlation_id.clone(),
            reason_code: reason.code().to_string(),
            status: AlertDispatchStatus::Delivered,
            delivered_at: Some(issued_at.clone()),
            failed_at: None,
        };

        if let Err(error) = self.run_with_runtime(create_incident_alert(&self.pool, &alert)) {
            if error.code != ReportingScheduleReasonCode::AlertUnavailable.code() {
                return Err(error);
            }
            if !error.message.contains("constraint") {
                return Err(error);
            }
        }

        let attempt = AlertDeliveryAttempt {
            alert_id,
            attempt_number: 1,
            channel: AlertDeliveryChannel::PagerDuty,
            outcome: AlertDeliveryOutcome::Delivered,
            reason_code: AlertReasonCode::Ready.code().to_string(),
            correlation_id: run.correlation_id.clone(),
            attempted_at: issued_at.clone(),
            delivered_at: Some(issued_at),
            failed_at: None,
        };
        if let Err(error) =
            self.run_with_runtime(append_alert_delivery_attempt(&self.pool, &attempt))
        {
            if !error.message.contains("constraint") {
                return Err(error);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct InMemoryAlertPort {
    alerts: Mutex<Vec<(String, String, String)>>,
}

impl ReportScheduleAlertPort for InMemoryAlertPort {
    fn emit_critical_failure_alert(
        &self,
        run: &ReportRunRecord,
        failure_code: &'static str,
        _failure_message: &str,
    ) -> Result<(), ReportSchedulingServiceError> {
        self.alerts
            .lock()
            .expect("in-memory alert lock should not be poisoned")
            .push((
                run.run_id.clone(),
                failure_code.to_string(),
                timestamp_utc(),
            ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::reporting_schedule::{build_reporting_window_for_boundary, parse_utc_timestamp};

    fn service_with_stub_summary(
        fail_with: Option<(&'static str, &'static str)>,
    ) -> ReportSchedulingService {
        ReportSchedulingService::new(
            Arc::new(InMemoryReportScheduleRepository::default()),
            Arc::new(StubSummaryPort { fail_with }),
            Arc::new(InMemoryAlertPort::default()),
        )
    }

    fn sample_upsert(
        schedule_id: &str,
        cadence: &str,
        timestamp_utc: &str,
    ) -> UpsertReportScheduleInput {
        UpsertReportScheduleInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            schedule_id: schedule_id.to_string(),
            cadence: cadence.to_string(),
            reason_code: None,
            correlation_id: "corr-report-schedule-001".to_string(),
            timestamp_utc: timestamp_utc.to_string(),
            runbook_url: Some(DEFAULT_RUNBOOK_URL.to_string()),
        }
    }

    #[test]
    fn schedule_mutations_compute_utc_boundaries_and_status_transitions() {
        let service = service_with_stub_summary(None);
        let created = service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-06T23:59:59Z",
            ))
            .expect("upsert should succeed");
        assert_eq!(created.status, "active");
        assert_eq!(created.next_run_at_utc, "2026-04-07T00:00:00Z");

        let paused = service
            .pause_schedule(PauseReportScheduleInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                schedule_id: "report-schedule-daily".to_string(),
                reason_code: None,
                correlation_id: "corr-pause-001".to_string(),
                timestamp_utc: "2026-04-07T00:01:00Z".to_string(),
            })
            .expect("pause should succeed");
        assert_eq!(paused.status, "paused");
        assert_eq!(
            paused.reason_code,
            ReportingScheduleReasonCode::SchedulePaused.code()
        );

        let resumed = service
            .resume_schedule(ResumeReportScheduleInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                schedule_id: "report-schedule-daily".to_string(),
                reason_code: None,
                correlation_id: "corr-resume-001".to_string(),
                timestamp_utc: "2026-04-07T00:10:00Z".to_string(),
            })
            .expect("resume should succeed");
        assert_eq!(resumed.status, "active");
        assert_eq!(resumed.next_run_at_utc, "2026-04-08T00:00:00Z");
    }

    #[test]
    fn scheduler_processes_due_runs_and_advances_next_boundary() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-weekly",
                "weekly",
                "2026-04-13T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let runs = service
            .process_due_schedules("2026-04-13T00:00:05Z")
            .expect("due processing should succeed");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ReportingRunState::Succeeded);
        assert_eq!(
            runs[0].reason_code,
            ReportingScheduleReasonCode::RunSucceeded.code()
        );

        let schedule = service
            .repository
            .load_schedule_by_id("report-schedule-weekly")
            .expect("schedule lookup should succeed")
            .expect("schedule must exist");
        assert_eq!(schedule.next_run_at_utc, "2026-04-20T00:00:00Z");
    }

    #[test]
    fn scheduler_marks_missed_runs_when_tick_arrives_after_sla_window() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-monthly",
                "monthly",
                "2026-05-01T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let runs = service
            .process_due_schedules("2026-05-01T00:00:31Z")
            .expect("due processing should succeed");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ReportingRunState::Missed);
        assert_eq!(
            runs[0].reason_code,
            ReportingScheduleReasonCode::RunMissed.code()
        );
    }

    #[test]
    fn scheduler_fails_closed_on_summary_dependency_errors() {
        let service = service_with_stub_summary(Some((
            ReportingScheduleReasonCode::DependencyUnavailable.code(),
            "read model dependency unavailable",
        )));
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-07T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let runs = service
            .process_due_schedules("2026-04-07T00:00:05Z")
            .expect("due processing should complete with failed run evidence");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ReportingRunState::Failed);
        assert_eq!(
            runs[0].reason_code,
            ReportingScheduleReasonCode::DependencyUnavailable.code()
        );
        assert!(runs[0].alert_emitted_at_utc.is_some());
    }

    #[test]
    fn run_history_is_bounded_and_deterministic() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-07T00:00:00Z",
            ))
            .expect("schedule should upsert");
        service
            .process_due_schedules("2026-04-07T00:00:05Z")
            .expect("first run should process");
        service
            .process_due_schedules("2026-04-08T00:00:05Z")
            .expect("second run should process");

        let runs = service
            .query_run_history(QueryReportRunHistoryInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                schedule_id: "report-schedule-daily".to_string(),
                correlation_id: "corr-history-001".to_string(),
                queried_at_utc: "2026-04-08T00:01:00Z".to_string(),
                limit: Some(1),
            })
            .expect("history query should succeed");
        assert_eq!(runs.len(), 1);
        let latest_window =
            parse_utc_timestamp("window_started_at_utc", &runs[0].window_started_at_utc)
                .expect("window timestamp should parse");
        assert_eq!(
            latest_window.unix_timestamp(),
            parse_utc_timestamp("window_started_at_utc", "2026-04-07T00:00:00Z")
                .expect("expected timestamp")
                .unix_timestamp()
        );
    }

    #[test]
    fn scheduler_replays_non_terminal_runs_after_restart() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-07T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let window = build_reporting_window_for_boundary(
            "report-schedule-daily",
            ReportingCadence::Daily,
            "2026-04-07T00:00:00Z",
        )
        .expect("window should build");
        let run_id = compose_report_run_id(
            "report-schedule-daily",
            ReportingCadence::Daily,
            &window.window_key,
        )
        .expect("run id should compose");
        service
            .repository
            .upsert_run(ReportRunRecord {
                run_id,
                schedule_id: "report-schedule-daily".to_string(),
                cadence: ReportingCadence::Daily,
                window_key: window.window_key,
                window_started_at_utc: window.window_start_at_utc,
                window_ended_at_utc: window.window_end_at_utc,
                status: ReportingRunState::Pending,
                reason_code: ReportingScheduleReasonCode::Ready.code().to_string(),
                correlation_id: "corr-report-schedule-001".to_string(),
                source_context: SCHEDULER_SOURCE_CONTEXT.to_string(),
                actor_id: Some(SCHEDULER_ACTOR_ID.to_string()),
                run_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
                run_finished_at_utc: None,
                alert_emitted_at_utc: None,
                runbook_url: Some(DEFAULT_RUNBOOK_URL.to_string()),
                impacted_system: Some("reporting-service scheduler".to_string()),
                created_at_utc: "2026-04-07T00:00:00Z".to_string(),
                updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
            })
            .expect("pending run should seed successfully");

        let runs = service
            .process_due_schedules("2026-04-07T00:00:05Z")
            .expect("existing pending run should be replayed");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ReportingRunState::Succeeded);

        let schedule = service
            .repository
            .load_schedule_by_id("report-schedule-daily")
            .expect("schedule lookup should succeed")
            .expect("schedule should exist");
        assert_eq!(schedule.next_run_at_utc, "2026-04-08T00:00:00Z");
    }

    #[test]
    fn run_history_query_rejects_out_of_range_limits() {
        let service = service_with_stub_summary(None);
        let error = service
            .query_run_history(QueryReportRunHistoryInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                schedule_id: "report-schedule-daily".to_string(),
                correlation_id: "corr-history-001".to_string(),
                queried_at_utc: "2026-04-08T00:01:00Z".to_string(),
                limit: Some(0),
            })
            .expect_err("limit below lower bound should fail");
        assert_eq!(
            error.code,
            ReportingScheduleReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );
    }

    #[test]
    fn resume_rejects_non_paused_schedule_states() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-07T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let error = service
            .resume_schedule(ResumeReportScheduleInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                schedule_id: "report-schedule-daily".to_string(),
                reason_code: None,
                correlation_id: "corr-resume-001".to_string(),
                timestamp_utc: "2026-04-07T00:05:00Z".to_string(),
            })
            .expect_err("active schedules should reject resume mutation");
        assert_eq!(
            error.code,
            ReportingScheduleReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "status")
        );
    }

    #[test]
    fn scheduler_retries_missing_alert_evidence_before_advancing_schedule() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-07T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let window = build_reporting_window_for_boundary(
            "report-schedule-daily",
            ReportingCadence::Daily,
            "2026-04-07T00:00:00Z",
        )
        .expect("window should build");
        let run_id = compose_report_run_id(
            "report-schedule-daily",
            ReportingCadence::Daily,
            &window.window_key,
        )
        .expect("run id should compose");
        service
            .repository
            .upsert_run(ReportRunRecord {
                run_id,
                schedule_id: "report-schedule-daily".to_string(),
                cadence: ReportingCadence::Daily,
                window_key: window.window_key,
                window_started_at_utc: window.window_start_at_utc,
                window_ended_at_utc: window.window_end_at_utc,
                status: ReportingRunState::Failed,
                reason_code: ReportingScheduleReasonCode::AlertUnavailable
                    .code()
                    .to_string(),
                correlation_id: "corr-report-schedule-001".to_string(),
                source_context: SCHEDULER_SOURCE_CONTEXT.to_string(),
                actor_id: Some(SCHEDULER_ACTOR_ID.to_string()),
                run_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
                run_finished_at_utc: Some("2026-04-07T00:00:05Z".to_string()),
                alert_emitted_at_utc: None,
                runbook_url: Some(DEFAULT_RUNBOOK_URL.to_string()),
                impacted_system: Some("reporting-service scheduler".to_string()),
                created_at_utc: "2026-04-07T00:00:00Z".to_string(),
                updated_at_utc: "2026-04-07T00:00:05Z".to_string(),
            })
            .expect("failed run should seed successfully");

        let runs = service
            .process_due_schedules("2026-04-07T00:00:10Z")
            .expect("scheduler should retry alert emission and continue");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ReportingRunState::Failed);
        assert!(runs[0].alert_emitted_at_utc.is_some());

        let schedule = service
            .repository
            .load_schedule_by_id("report-schedule-daily")
            .expect("schedule lookup should succeed")
            .expect("schedule should exist");
        assert_eq!(schedule.next_run_at_utc, "2026-04-08T00:00:00Z");
    }

    #[test]
    fn scheduler_keeps_fail_closed_when_alert_retry_exceeds_sla() {
        let service = service_with_stub_summary(None);
        service
            .upsert_schedule(sample_upsert(
                "report-schedule-daily",
                "daily",
                "2026-04-07T00:00:00Z",
            ))
            .expect("schedule should upsert");

        let window = build_reporting_window_for_boundary(
            "report-schedule-daily",
            ReportingCadence::Daily,
            "2026-04-07T00:00:00Z",
        )
        .expect("window should build");
        let run_id = compose_report_run_id(
            "report-schedule-daily",
            ReportingCadence::Daily,
            &window.window_key,
        )
        .expect("run id should compose");
        service
            .repository
            .upsert_run(ReportRunRecord {
                run_id,
                schedule_id: "report-schedule-daily".to_string(),
                cadence: ReportingCadence::Daily,
                window_key: window.window_key,
                window_started_at_utc: window.window_start_at_utc,
                window_ended_at_utc: window.window_end_at_utc,
                status: ReportingRunState::Failed,
                reason_code: ReportingScheduleReasonCode::AlertUnavailable
                    .code()
                    .to_string(),
                correlation_id: "corr-report-schedule-001".to_string(),
                source_context: SCHEDULER_SOURCE_CONTEXT.to_string(),
                actor_id: Some(SCHEDULER_ACTOR_ID.to_string()),
                run_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
                run_finished_at_utc: Some("2026-04-07T00:00:00Z".to_string()),
                alert_emitted_at_utc: None,
                runbook_url: Some(DEFAULT_RUNBOOK_URL.to_string()),
                impacted_system: Some("reporting-service scheduler".to_string()),
                created_at_utc: "2026-04-07T00:00:00Z".to_string(),
                updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
            })
            .expect("failed run should seed successfully");

        let error = service
            .process_due_schedules("2026-04-07T00:00:40Z")
            .expect_err("alert SLA violation should fail closed");
        assert_eq!(
            error.code,
            ReportingScheduleReasonCode::AlertUnavailable.code()
        );

        let schedule = service
            .repository
            .load_schedule_by_id("report-schedule-daily")
            .expect("schedule lookup should succeed")
            .expect("schedule should exist");
        assert_eq!(schedule.next_run_at_utc, "2026-04-07T00:00:00Z");
    }

    #[test]
    fn composed_run_id_is_bounded_for_maximum_schedule_identifiers() {
        let schedule_id = format!("sched-{}", "a".repeat(154));
        let first_window = build_reporting_window_for_boundary(
            &schedule_id,
            ReportingCadence::Daily,
            "2026-04-07T00:00:00Z",
        )
        .expect("first window should build");
        let second_window = build_reporting_window_for_boundary(
            &schedule_id,
            ReportingCadence::Daily,
            "2026-04-08T00:00:00Z",
        )
        .expect("second window should build");

        let first_run_id = compose_report_run_id(
            &schedule_id,
            ReportingCadence::Daily,
            &first_window.window_key,
        )
        .expect("first run id should compose");
        let second_run_id = compose_report_run_id(
            &schedule_id,
            ReportingCadence::Daily,
            &second_window.window_key,
        )
        .expect("second run id should compose");

        assert!(first_run_id.len() <= 160);
        assert!(second_run_id.len() <= 160);
        assert_ne!(first_run_id, second_run_id);
    }
}
