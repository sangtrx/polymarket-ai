use crate::governance::{GovernancePermission, GovernanceRole, RolePermissionMatrix};
use serde::{Deserialize, Serialize};
use time::{
    Date, Duration, Month, OffsetDateTime, Time, UtcOffset, Weekday,
    format_description::well_known::Rfc3339,
};

pub const DEFAULT_REPORT_RUN_HISTORY_LIMIT: i64 = 200;
pub const MAX_REPORT_RUN_HISTORY_LIMIT: i64 = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingScheduleReasonCode {
    Ready,
    InvalidPayload,
    Unauthorized,
    DependencyUnavailable,
    StaleEvidence,
    PersistenceUnavailable,
    AlertUnavailable,
    NotFound,
    RunDuplicateSuppressed,
    RunSucceeded,
    RunFailed,
    RunMissed,
    SchedulePaused,
    ScheduleResumed,
}

impl ReportingScheduleReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "reporting_schedule_ready",
            Self::InvalidPayload => "reporting_schedule_invalid_payload",
            Self::Unauthorized => "reporting_schedule_unauthorized",
            Self::DependencyUnavailable => "reporting_schedule_dependency_unavailable",
            Self::StaleEvidence => "reporting_schedule_stale_evidence",
            Self::PersistenceUnavailable => "reporting_schedule_persistence_unavailable",
            Self::AlertUnavailable => "reporting_schedule_alert_unavailable",
            Self::NotFound => "reporting_schedule_not_found",
            Self::RunDuplicateSuppressed => "reporting_schedule_run_duplicate_suppressed",
            Self::RunSucceeded => "reporting_schedule_run_succeeded",
            Self::RunFailed => "reporting_schedule_run_failed",
            Self::RunMissed => "reporting_schedule_run_missed",
            Self::SchedulePaused => "reporting_schedule_paused",
            Self::ScheduleResumed => "reporting_schedule_resumed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingScheduleContractError> {
        match normalize_reporting_schedule_identifier(value).as_str() {
            "reporting_schedule_ready" => Ok(Self::Ready),
            "reporting_schedule_invalid_payload" => Ok(Self::InvalidPayload),
            "reporting_schedule_unauthorized" => Ok(Self::Unauthorized),
            "reporting_schedule_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "reporting_schedule_stale_evidence" => Ok(Self::StaleEvidence),
            "reporting_schedule_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "reporting_schedule_alert_unavailable" => Ok(Self::AlertUnavailable),
            "reporting_schedule_not_found" => Ok(Self::NotFound),
            "reporting_schedule_run_duplicate_suppressed" => Ok(Self::RunDuplicateSuppressed),
            "reporting_schedule_run_succeeded" => Ok(Self::RunSucceeded),
            "reporting_schedule_run_failed" => Ok(Self::RunFailed),
            "reporting_schedule_run_missed" => Ok(Self::RunMissed),
            "reporting_schedule_paused" => Ok(Self::SchedulePaused),
            "reporting_schedule_resumed" => Ok(Self::ScheduleResumed),
            other => Err(ReportingScheduleContractError::invalid_payload(format!(
                "unknown reporting schedule reason code `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReportingCadence {
    Daily,
    Weekly,
    Monthly,
}

impl ReportingCadence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingScheduleContractError> {
        match normalize_reporting_schedule_identifier(value).as_str() {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            _ => Err(ReportingScheduleContractError::invalid_payload_with_issues(
                "cadence must be one of: daily, weekly, monthly",
                vec![ReportingScheduleValidationIssue {
                    field: "cadence",
                    code: ReportingScheduleReasonCode::InvalidPayload.code(),
                    message: "cadence must be one of: daily, weekly, monthly".to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingScheduleState {
    Active,
    Paused,
    Disabled,
}

impl ReportingScheduleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingScheduleContractError> {
        match normalize_reporting_schedule_identifier(value).as_str() {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "disabled" => Ok(Self::Disabled),
            _ => Err(ReportingScheduleContractError::invalid_payload_with_issues(
                "status must be one of: active, paused, disabled",
                vec![ReportingScheduleValidationIssue {
                    field: "status",
                    code: ReportingScheduleReasonCode::InvalidPayload.code(),
                    message: "status must be one of: active, paused, disabled".to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingRunState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Missed,
}

impl ReportingRunState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Missed => "missed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingScheduleContractError> {
        match normalize_reporting_schedule_identifier(value).as_str() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "missed" => Ok(Self::Missed),
            _ => Err(ReportingScheduleContractError::invalid_payload_with_issues(
                "run status must be one of: pending, running, succeeded, failed, missed",
                vec![ReportingScheduleValidationIssue {
                    field: "status",
                    code: ReportingScheduleReasonCode::InvalidPayload.code(),
                    message:
                        "run status must be one of: pending, running, succeeded, failed, missed"
                            .to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingScheduleValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingScheduleContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingScheduleValidationIssue>,
}

impl ReportingScheduleContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ReportingScheduleValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn alert_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::AlertUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ReportingScheduleReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportSchedule {
    pub schedule_id: String,
    pub cadence: ReportingCadence,
    pub status: ReportingScheduleState,
    pub next_run_at_utc: String,
    pub actor_id: String,
    pub actor_role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub runbook_url: String,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    pub paused_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportRunRecord {
    pub run_id: String,
    pub schedule_id: String,
    pub cadence: ReportingCadence,
    pub window_key: String,
    pub window_started_at_utc: String,
    pub window_ended_at_utc: String,
    pub status: ReportingRunState,
    pub reason_code: String,
    pub correlation_id: String,
    pub source_context: String,
    pub actor_id: Option<String>,
    pub run_started_at_utc: String,
    pub run_finished_at_utc: Option<String>,
    pub alert_emitted_at_utc: Option<String>,
    pub runbook_url: Option<String>,
    pub impacted_system: Option<String>,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingWindowBounds {
    pub window_start_at_utc: String,
    pub window_end_at_utc: String,
    pub window_key: String,
}

pub fn normalize_reporting_schedule_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ReportingScheduleContractError> {
    let timestamp = OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| {
        ReportingScheduleContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![ReportingScheduleValidationIssue {
                field,
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    if timestamp.offset() != UtcOffset::UTC {
        return Err(ReportingScheduleContractError::invalid_payload_with_issues(
            format!("{field} must be UTC (offset Z)"),
            vec![ReportingScheduleValidationIssue {
                field,
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
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

pub fn authorize_schedule_mutation(actor_role: &str) -> Result<(), ReportingScheduleContractError> {
    let role = GovernanceRole::parse(actor_role).map_err(|_| {
        ReportingScheduleContractError::unauthorized(
            "actor role is not authorized for report schedule mutation",
        )
    })?;
    let matrix = RolePermissionMatrix::canonical();
    let allowed = matrix
        .has_permission(role, GovernancePermission::ExecuteControlAction)
        .map_err(|_| {
            ReportingScheduleContractError::unauthorized(
                "authorization matrix rejected actor role for control action",
            )
        })?;
    if !allowed {
        return Err(ReportingScheduleContractError::unauthorized(
            "actor role is not authorized for report schedule mutation",
        ));
    }
    Ok(())
}

pub fn authorize_schedule_read(actor_role: &str) -> Result<(), ReportingScheduleContractError> {
    let role = GovernanceRole::parse(actor_role).map_err(|_| {
        ReportingScheduleContractError::unauthorized(
            "actor role is not authorized for report scheduling reads",
        )
    })?;
    let matrix = RolePermissionMatrix::canonical();
    let allowed = matrix
        .has_permission(role, GovernancePermission::ReadAnalytics)
        .map_err(|_| {
            ReportingScheduleContractError::unauthorized(
                "authorization matrix rejected actor role for analytics read",
            )
        })?;
    if !allowed {
        return Err(ReportingScheduleContractError::unauthorized(
            "actor role is not authorized for report scheduling reads",
        ));
    }
    Ok(())
}

pub fn cadence_window_bounds(
    cadence: ReportingCadence,
    timestamp_utc: &str,
) -> Result<(String, String), ReportingScheduleContractError> {
    let timestamp = parse_utc_timestamp("timestamp_utc", timestamp_utc)?;
    let (window_start, window_end) = cadence_window_bounds_from_time(cadence, timestamp)?;
    Ok((
        format_utc_timestamp(window_start),
        format_utc_timestamp(window_end),
    ))
}

pub fn next_run_at_utc(
    cadence: ReportingCadence,
    reference_utc: &str,
) -> Result<String, ReportingScheduleContractError> {
    let reference = parse_utc_timestamp("reference_utc", reference_utc)?;
    let (window_start, window_end) = cadence_window_bounds_from_time(cadence, reference)?;
    if reference == window_start {
        return Ok(format_utc_timestamp(window_start));
    }
    Ok(format_utc_timestamp(window_end))
}

pub fn advance_to_next_run_at_utc(
    cadence: ReportingCadence,
    current_boundary_utc: &str,
) -> Result<String, ReportingScheduleContractError> {
    let current = parse_utc_timestamp("current_boundary_utc", current_boundary_utc)?;
    let advanced = current + Duration::seconds(1);
    next_run_at_utc(cadence, &format_utc_timestamp(advanced))
}

pub fn build_reporting_window_for_boundary(
    schedule_id: &str,
    cadence: ReportingCadence,
    boundary_utc: &str,
) -> Result<ReportingWindowBounds, ReportingScheduleContractError> {
    let schedule_id = normalize_reporting_schedule_identifier(schedule_id);
    validate_canonical_identifier("schedule_id", &schedule_id)?;

    let boundary = parse_utc_timestamp("boundary_utc", boundary_utc)?;
    let previous_tick = boundary - Duration::seconds(1);
    let (window_start, window_end) = cadence_window_bounds_from_time(cadence, previous_tick)?;
    let window_start_at_utc = format_utc_timestamp(window_start);
    let window_end_at_utc = format_utc_timestamp(window_end);
    let window_key = compose_report_window_key(
        &schedule_id,
        cadence,
        &window_start_at_utc,
        &window_end_at_utc,
    )?;
    Ok(ReportingWindowBounds {
        window_start_at_utc,
        window_end_at_utc,
        window_key,
    })
}

pub fn compose_report_window_key(
    schedule_id: &str,
    cadence: ReportingCadence,
    window_start_at_utc: &str,
    window_end_at_utc: &str,
) -> Result<String, ReportingScheduleContractError> {
    let normalized_schedule = normalize_reporting_schedule_identifier(schedule_id);
    validate_canonical_identifier("schedule_id", &normalized_schedule)?;
    let start = parse_utc_timestamp("window_start_at_utc", window_start_at_utc)?;
    let end = parse_utc_timestamp("window_end_at_utc", window_end_at_utc)?;
    if end <= start {
        return Err(ReportingScheduleContractError::invalid_payload_with_issues(
            "window_end_at_utc must be strictly greater than window_start_at_utc",
            vec![ReportingScheduleValidationIssue {
                field: "window_end_at_utc",
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: "window_end_at_utc must be strictly greater than window_start_at_utc"
                    .to_string(),
            }],
        ));
    }
    let window_key = format!(
        "report-window::{normalized_schedule}::{}::{}",
        cadence.as_str(),
        start.unix_timestamp()
    );
    validate_canonical_identifier("window_key", &window_key)?;
    Ok(window_key)
}

pub fn validate_report_schedule(
    schedule: &ReportSchedule,
) -> Result<(), ReportingScheduleContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier_issue(&mut field_errors, "schedule_id", &schedule.schedule_id);
    validate_canonical_identifier_issue(&mut field_errors, "actor_id", &schedule.actor_id);
    validate_canonical_identifier_issue(&mut field_errors, "actor_role", &schedule.actor_role);
    validate_canonical_identifier_issue(&mut field_errors, "reason_code", &schedule.reason_code);
    validate_canonical_identifier_issue(
        &mut field_errors,
        "correlation_id",
        &schedule.correlation_id,
    );
    validate_non_empty_issue(&mut field_errors, "runbook_url", &schedule.runbook_url);

    if !schedule.runbook_url.starts_with("http://") && !schedule.runbook_url.starts_with("https://")
    {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "runbook_url",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "runbook_url must begin with http:// or https://".to_string(),
        });
    }

    if ReportingScheduleReasonCode::parse(&schedule.reason_code).is_err() {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "reason_code",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported reporting schedule reason code".to_string(),
        });
    }

    parse_timestamp_issue(
        &mut field_errors,
        "next_run_at_utc",
        &schedule.next_run_at_utc,
    );
    parse_timestamp_issue(
        &mut field_errors,
        "created_at_utc",
        &schedule.created_at_utc,
    );
    parse_timestamp_issue(
        &mut field_errors,
        "updated_at_utc",
        &schedule.updated_at_utc,
    );
    if let Some(paused_at_utc) = schedule.paused_at_utc.as_deref() {
        parse_timestamp_issue(&mut field_errors, "paused_at_utc", paused_at_utc);
    }
    if schedule.status == ReportingScheduleState::Paused && schedule.paused_at_utc.is_none() {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "paused_at_utc",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "paused schedules must include paused_at_utc".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(ReportingScheduleContractError::invalid_payload_with_issues(
            "report schedule failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_report_run(run: &ReportRunRecord) -> Result<(), ReportingScheduleContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier_issue(&mut field_errors, "run_id", &run.run_id);
    validate_canonical_identifier_issue(&mut field_errors, "schedule_id", &run.schedule_id);
    validate_canonical_identifier_issue(&mut field_errors, "window_key", &run.window_key);
    validate_canonical_identifier_issue(&mut field_errors, "reason_code", &run.reason_code);
    validate_canonical_identifier_issue(&mut field_errors, "correlation_id", &run.correlation_id);
    validate_non_empty_issue(&mut field_errors, "source_context", &run.source_context);
    if let Some(actor_id) = run.actor_id.as_deref() {
        validate_canonical_identifier_issue(&mut field_errors, "actor_id", actor_id);
    }
    if let Some(runbook_url) = run.runbook_url.as_deref()
        && !runbook_url.is_empty()
        && !runbook_url.starts_with("http://")
        && !runbook_url.starts_with("https://")
    {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "runbook_url",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "runbook_url must begin with http:// or https://".to_string(),
        });
    }

    if ReportingScheduleReasonCode::parse(&run.reason_code).is_err() {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "reason_code",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported reporting schedule reason code".to_string(),
        });
    }

    let window_start = parse_timestamp_issue(
        &mut field_errors,
        "window_started_at_utc",
        &run.window_started_at_utc,
    );
    let window_end = parse_timestamp_issue(
        &mut field_errors,
        "window_ended_at_utc",
        &run.window_ended_at_utc,
    );
    let run_started = parse_timestamp_issue(
        &mut field_errors,
        "run_started_at_utc",
        &run.run_started_at_utc,
    );
    let run_finished = if let Some(value) = run.run_finished_at_utc.as_deref() {
        parse_timestamp_issue(&mut field_errors, "run_finished_at_utc", value)
    } else {
        None
    };
    parse_timestamp_issue(&mut field_errors, "created_at_utc", &run.created_at_utc);
    parse_timestamp_issue(&mut field_errors, "updated_at_utc", &run.updated_at_utc);
    if let Some(value) = run.alert_emitted_at_utc.as_deref() {
        parse_timestamp_issue(&mut field_errors, "alert_emitted_at_utc", value);
    }

    if let (Some(window_start), Some(window_end)) = (window_start, window_end)
        && window_end <= window_start
    {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "window_ended_at_utc",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "window_ended_at_utc must be strictly greater than window_started_at_utc"
                .to_string(),
        });
    }

    if matches!(
        run.status,
        ReportingRunState::Pending | ReportingRunState::Running
    ) && run.run_finished_at_utc.is_some()
    {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "run_finished_at_utc",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "pending/running runs must not include run_finished_at_utc".to_string(),
        });
    }
    if matches!(
        run.status,
        ReportingRunState::Succeeded | ReportingRunState::Failed | ReportingRunState::Missed
    ) && run.run_finished_at_utc.is_none()
    {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "run_finished_at_utc",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "terminal run states require run_finished_at_utc".to_string(),
        });
    }
    if let (Some(run_started), Some(run_finished)) = (run_started, run_finished)
        && run_finished < run_started
    {
        field_errors.push(ReportingScheduleValidationIssue {
            field: "run_finished_at_utc",
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: "run_finished_at_utc must be >= run_started_at_utc".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(ReportingScheduleContractError::invalid_payload_with_issues(
            "report run record failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_run_state_transition(
    previous: Option<ReportingRunState>,
    next: ReportingRunState,
) -> Result<(), ReportingScheduleContractError> {
    let allowed = match previous {
        None => matches!(next, ReportingRunState::Pending | ReportingRunState::Missed),
        Some(ReportingRunState::Pending) => {
            matches!(
                next,
                ReportingRunState::Pending
                    | ReportingRunState::Running
                    | ReportingRunState::Failed
                    | ReportingRunState::Missed
            )
        }
        Some(ReportingRunState::Running) => {
            matches!(
                next,
                ReportingRunState::Running
                    | ReportingRunState::Succeeded
                    | ReportingRunState::Failed
                    | ReportingRunState::Missed
            )
        }
        Some(ReportingRunState::Succeeded)
        | Some(ReportingRunState::Failed)
        | Some(ReportingRunState::Missed) => previous == Some(next),
    };
    if !allowed {
        return Err(ReportingScheduleContractError::invalid_payload_with_issues(
            "invalid report run status transition",
            vec![ReportingScheduleValidationIssue {
                field: "status",
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: "status transition is not allowed".to_string(),
            }],
        ));
    }
    Ok(())
}

fn cadence_window_bounds_from_time(
    cadence: ReportingCadence,
    timestamp: OffsetDateTime,
) -> Result<(OffsetDateTime, OffsetDateTime), ReportingScheduleContractError> {
    let utc = timestamp.to_offset(UtcOffset::UTC);
    let start = match cadence {
        ReportingCadence::Daily => start_of_day(utc)?,
        ReportingCadence::Weekly => start_of_week(utc)?,
        ReportingCadence::Monthly => start_of_month(utc)?,
    };
    let end = match cadence {
        ReportingCadence::Daily => start + Duration::days(1),
        ReportingCadence::Weekly => start + Duration::days(7),
        ReportingCadence::Monthly => start_of_next_month(start)?,
    };
    Ok((start, end))
}

fn start_of_day(
    timestamp: OffsetDateTime,
) -> Result<OffsetDateTime, ReportingScheduleContractError> {
    let date = Date::from_calendar_date(timestamp.year(), timestamp.month(), timestamp.day())
        .map_err(|_| ReportingScheduleContractError::invalid_payload("invalid day boundary"))?;
    Ok(date.with_time(Time::MIDNIGHT).assume_utc())
}

fn start_of_week(
    timestamp: OffsetDateTime,
) -> Result<OffsetDateTime, ReportingScheduleContractError> {
    let midnight = start_of_day(timestamp)?;
    let weekday = midnight.weekday();
    let offset_days = match weekday {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    };
    Ok(midnight - Duration::days(offset_days))
}

fn start_of_month(
    timestamp: OffsetDateTime,
) -> Result<OffsetDateTime, ReportingScheduleContractError> {
    let date = Date::from_calendar_date(timestamp.year(), timestamp.month(), 1).map_err(|_| {
        ReportingScheduleContractError::invalid_payload("invalid month boundary date")
    })?;
    Ok(date.with_time(Time::MIDNIGHT).assume_utc())
}

fn start_of_next_month(
    month_start: OffsetDateTime,
) -> Result<OffsetDateTime, ReportingScheduleContractError> {
    let (year, month) = if month_start.month() == Month::December {
        (month_start.year() + 1, Month::January)
    } else {
        let next = match month_start.month() {
            Month::January => Month::February,
            Month::February => Month::March,
            Month::March => Month::April,
            Month::April => Month::May,
            Month::May => Month::June,
            Month::June => Month::July,
            Month::July => Month::August,
            Month::August => Month::September,
            Month::September => Month::October,
            Month::October => Month::November,
            Month::November => Month::December,
            Month::December => Month::January,
        };
        (month_start.year(), next)
    };
    let next_month_date = Date::from_calendar_date(year, month, 1)
        .map_err(|_| ReportingScheduleContractError::invalid_payload("invalid next month date"))?;
    Ok(next_month_date.with_time(Time::MIDNIGHT).assume_utc())
}

fn validate_non_empty_issue(
    issues: &mut Vec<ReportingScheduleValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        issues.push(ReportingScheduleValidationIssue {
            field,
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: format!("{field} must not be empty"),
        });
    }
}

fn validate_canonical_identifier_issue(
    issues: &mut Vec<ReportingScheduleValidationIssue>,
    field: &'static str,
    value: &str,
) {
    let normalized = normalize_reporting_schedule_identifier(value);
    let (min_len, max_len) = identifier_length_bounds(field);
    if !(min_len..=max_len).contains(&normalized.len()) {
        issues.push(ReportingScheduleValidationIssue {
            field,
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain {min_len}-{max_len} canonical characters"),
        });
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        issues.push(ReportingScheduleValidationIssue {
            field,
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: format!("{field} contains unsupported characters"),
        });
    }
}

fn validate_canonical_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), ReportingScheduleContractError> {
    let mut issues = Vec::new();
    validate_canonical_identifier_issue(&mut issues, field, value);
    if issues.is_empty() {
        return Ok(());
    }
    Err(ReportingScheduleContractError::invalid_payload_with_issues(
        format!("{field} failed canonical identifier validation"),
        issues,
    ))
}

fn identifier_length_bounds(field: &'static str) -> (usize, usize) {
    match field {
        "window_key" => (3, 200),
        _ => (3, 160),
    }
}

fn parse_timestamp_issue(
    issues: &mut Vec<ReportingScheduleValidationIssue>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schedule(status: ReportingScheduleState) -> ReportSchedule {
        ReportSchedule {
            schedule_id: "report-schedule-daily".to_string(),
            cadence: ReportingCadence::Daily,
            status,
            next_run_at_utc: "2026-04-07T00:00:00Z".to_string(),
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            reason_code: ReportingScheduleReasonCode::Ready.code().to_string(),
            correlation_id: "corr-schedule-001".to_string(),
            runbook_url: "https://docs.example.com/operations/recurring-report-scheduling"
                .to_string(),
            created_at_utc: "2026-04-06T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
            paused_at_utc: if status == ReportingScheduleState::Paused {
                Some("2026-04-06T00:10:00Z".to_string())
            } else {
                None
            },
        }
    }

    fn sample_run(status: ReportingRunState) -> ReportRunRecord {
        ReportRunRecord {
            run_id: "report-run-daily-001".to_string(),
            schedule_id: "report-schedule-daily".to_string(),
            cadence: ReportingCadence::Daily,
            window_key: "report-window::report-schedule-daily::daily::1775433600".to_string(),
            window_started_at_utc: "2026-04-06T00:00:00Z".to_string(),
            window_ended_at_utc: "2026-04-07T00:00:00Z".to_string(),
            status,
            reason_code: ReportingScheduleReasonCode::RunSucceeded.code().to_string(),
            correlation_id: "corr-schedule-run-001".to_string(),
            source_context: "reporting-service.scheduler".to_string(),
            actor_id: Some("scheduler".to_string()),
            run_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            run_finished_at_utc: if matches!(
                status,
                ReportingRunState::Succeeded
                    | ReportingRunState::Failed
                    | ReportingRunState::Missed
            ) {
                Some("2026-04-07T00:00:10Z".to_string())
            } else {
                None
            },
            alert_emitted_at_utc: None,
            runbook_url: Some(
                "https://docs.example.com/operations/recurring-report-scheduling".to_string(),
            ),
            impacted_system: Some("reporting-service scheduler".to_string()),
            created_at_utc: "2026-04-07T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-07T00:00:10Z".to_string(),
        }
    }

    #[test]
    fn reason_code_roundtrip_is_canonical() {
        for reason in [
            ReportingScheduleReasonCode::Ready,
            ReportingScheduleReasonCode::InvalidPayload,
            ReportingScheduleReasonCode::Unauthorized,
            ReportingScheduleReasonCode::DependencyUnavailable,
            ReportingScheduleReasonCode::StaleEvidence,
            ReportingScheduleReasonCode::PersistenceUnavailable,
            ReportingScheduleReasonCode::AlertUnavailable,
            ReportingScheduleReasonCode::NotFound,
            ReportingScheduleReasonCode::RunDuplicateSuppressed,
            ReportingScheduleReasonCode::RunSucceeded,
            ReportingScheduleReasonCode::RunFailed,
            ReportingScheduleReasonCode::RunMissed,
            ReportingScheduleReasonCode::SchedulePaused,
            ReportingScheduleReasonCode::ScheduleResumed,
        ] {
            assert_eq!(
                ReportingScheduleReasonCode::parse(reason.code()).expect("reason code must parse"),
                reason
            );
        }
    }

    #[test]
    fn utc_boundaries_are_deterministic_for_daily_weekly_monthly() {
        assert_eq!(
            next_run_at_utc(ReportingCadence::Daily, "2026-04-06T23:59:59Z")
                .expect("daily boundary should calculate"),
            "2026-04-07T00:00:00Z"
        );
        assert_eq!(
            next_run_at_utc(ReportingCadence::Daily, "2026-04-07T00:00:00Z")
                .expect("daily exact threshold should hold"),
            "2026-04-07T00:00:00Z"
        );
        assert_eq!(
            next_run_at_utc(ReportingCadence::Weekly, "2026-04-12T23:59:59Z")
                .expect("weekly rollover should calculate"),
            "2026-04-13T00:00:00Z"
        );
        assert_eq!(
            next_run_at_utc(ReportingCadence::Monthly, "2024-02-29T23:59:59Z")
                .expect("leap-year month rollover should calculate"),
            "2024-03-01T00:00:00Z"
        );
    }

    #[test]
    fn window_bounds_align_to_expected_story_edges() {
        let (daily_start, daily_end) =
            cadence_window_bounds(ReportingCadence::Daily, "2026-04-06T23:59:59Z")
                .expect("daily window should resolve");
        assert_eq!(daily_start, "2026-04-06T00:00:00Z");
        assert_eq!(daily_end, "2026-04-07T00:00:00Z");

        let (weekly_start, weekly_end) =
            cadence_window_bounds(ReportingCadence::Weekly, "2026-04-12T23:59:59Z")
                .expect("weekly window should resolve");
        assert_eq!(weekly_start, "2026-04-06T00:00:00Z");
        assert_eq!(weekly_end, "2026-04-13T00:00:00Z");

        let (monthly_start, monthly_end) =
            cadence_window_bounds(ReportingCadence::Monthly, "2024-02-29T23:59:59Z")
                .expect("monthly window should resolve");
        assert_eq!(monthly_start, "2024-02-01T00:00:00Z");
        assert_eq!(monthly_end, "2024-03-01T00:00:00Z");
    }

    #[test]
    fn compose_window_key_is_deterministic_and_canonical() {
        let window = build_reporting_window_for_boundary(
            " report-schedule-daily ",
            ReportingCadence::Daily,
            "2026-04-07T00:00:00Z",
        )
        .expect("window should build");
        assert_eq!(window.window_start_at_utc, "2026-04-06T00:00:00Z");
        assert_eq!(window.window_end_at_utc, "2026-04-07T00:00:00Z");
        assert_eq!(
            window.window_key,
            "report-window::report-schedule-daily::daily::1775433600"
        );
    }

    #[test]
    fn authorization_reuses_governance_role_matrix() {
        assert!(authorize_schedule_mutation("operational_control").is_ok());
        assert!(authorize_schedule_mutation("administrative_actions").is_ok());
        let denied = authorize_schedule_mutation("read_only_analytics")
            .expect_err("read-only role should be denied");
        assert_eq!(
            denied.code,
            ReportingScheduleReasonCode::Unauthorized.code()
        );

        assert!(authorize_schedule_read("read_only_analytics").is_ok());
        assert!(authorize_schedule_read("guest").is_err());
    }

    #[test]
    fn report_schedule_validation_enforces_paused_timestamp_and_utc() {
        assert!(validate_report_schedule(&sample_schedule(ReportingScheduleState::Active)).is_ok());
        assert!(validate_report_schedule(&sample_schedule(ReportingScheduleState::Paused)).is_ok());

        let mut invalid = sample_schedule(ReportingScheduleState::Paused);
        invalid.paused_at_utc = None;
        let paused_error =
            validate_report_schedule(&invalid).expect_err("paused schedule missing timestamp");
        assert_eq!(
            paused_error.field_errors[0].field, "paused_at_utc",
            "paused schedule should require paused_at_utc"
        );

        let mut non_utc = sample_schedule(ReportingScheduleState::Active);
        non_utc.next_run_at_utc = "2026-04-07T00:00:00+01:00".to_string();
        let utc_error =
            validate_report_schedule(&non_utc).expect_err("non-UTC timestamps must be rejected");
        assert!(
            utc_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "next_run_at_utc")
        );
    }

    #[test]
    fn report_run_validation_enforces_terminal_timestamps() {
        assert!(validate_report_run(&sample_run(ReportingRunState::Pending)).is_ok());
        assert!(validate_report_run(&sample_run(ReportingRunState::Running)).is_ok());
        assert!(validate_report_run(&sample_run(ReportingRunState::Succeeded)).is_ok());
        assert!(validate_report_run(&sample_run(ReportingRunState::Failed)).is_ok());
        assert!(validate_report_run(&sample_run(ReportingRunState::Missed)).is_ok());

        let mut invalid = sample_run(ReportingRunState::Succeeded);
        invalid.run_finished_at_utc = None;
        let error = validate_report_run(&invalid)
            .expect_err("terminal state without run_finished_at_utc should fail");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "run_finished_at_utc")
        );
    }

    #[test]
    fn run_state_transition_validation_is_fail_closed() {
        assert!(validate_run_state_transition(None, ReportingRunState::Pending).is_ok());
        assert!(
            validate_run_state_transition(
                Some(ReportingRunState::Pending),
                ReportingRunState::Running
            )
            .is_ok()
        );
        assert!(
            validate_run_state_transition(
                Some(ReportingRunState::Running),
                ReportingRunState::Succeeded
            )
            .is_ok()
        );

        let invalid = validate_run_state_transition(
            Some(ReportingRunState::Succeeded),
            ReportingRunState::Failed,
        )
        .expect_err("terminal state transition should fail");
        assert_eq!(
            invalid.code,
            ReportingScheduleReasonCode::InvalidPayload.code()
        );
    }
}
