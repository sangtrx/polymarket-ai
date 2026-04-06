use domain::reporting_schedule::{
    MAX_REPORT_RUN_HISTORY_LIMIT, ReportRunRecord, ReportSchedule, ReportingCadence,
    ReportingRunState, ReportingScheduleContractError, ReportingScheduleReasonCode,
    ReportingScheduleState, ReportingScheduleValidationIssue, validate_report_run,
    validate_report_schedule, validate_run_state_transition,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_REPORT_SCHEDULE_SQL: &str = r#"
    INSERT INTO report_schedules (
        schedule_id,
        cadence,
        status,
        next_run_at_utc,
        actor_id,
        actor_role,
        reason_code,
        correlation_id,
        runbook_url,
        created_at_utc,
        updated_at_utc,
        paused_at_utc
    ) VALUES (
        $1, $2, $3, $4::timestamptz, $5, $6, $7, $8, $9, $10::timestamptz, $11::timestamptz, $12::timestamptz
    )
    ON CONFLICT (schedule_id)
    DO UPDATE SET
        cadence = EXCLUDED.cadence,
        status = EXCLUDED.status,
        next_run_at_utc = EXCLUDED.next_run_at_utc,
        actor_id = EXCLUDED.actor_id,
        actor_role = EXCLUDED.actor_role,
        reason_code = EXCLUDED.reason_code,
        correlation_id = EXCLUDED.correlation_id,
        runbook_url = EXCLUDED.runbook_url,
        updated_at_utc = EXCLUDED.updated_at_utc,
        paused_at_utc = EXCLUDED.paused_at_utc
    RETURNING
        schedule_id,
        cadence,
        status,
        to_char(next_run_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS next_run_at_utc,
        actor_id,
        actor_role,
        reason_code,
        correlation_id,
        runbook_url,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc,
        CASE
            WHEN paused_at_utc IS NULL THEN NULL
            ELSE to_char(paused_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS paused_at_utc
"#;

const LOAD_REPORT_SCHEDULE_BY_ID_SQL: &str = r#"
    SELECT
        schedule_id,
        cadence,
        status,
        to_char(next_run_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS next_run_at_utc,
        actor_id,
        actor_role,
        reason_code,
        correlation_id,
        runbook_url,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc,
        CASE
            WHEN paused_at_utc IS NULL THEN NULL
            ELSE to_char(paused_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS paused_at_utc
    FROM report_schedules
    WHERE schedule_id = $1
    LIMIT 1
"#;

const LOAD_DUE_REPORT_SCHEDULES_SQL: &str = r#"
    SELECT
        schedule_id,
        cadence,
        status,
        to_char(next_run_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS next_run_at_utc,
        actor_id,
        actor_role,
        reason_code,
        correlation_id,
        runbook_url,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc,
        CASE
            WHEN paused_at_utc IS NULL THEN NULL
            ELSE to_char(paused_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS paused_at_utc
    FROM report_schedules
    WHERE status = 'active'
      AND next_run_at_utc <= $1::timestamptz
    ORDER BY next_run_at_utc ASC, schedule_id ASC
    LIMIT $2
"#;

const LOAD_REPORT_RUN_STATUS_SQL: &str = r#"
    SELECT status
    FROM report_runs
    WHERE run_id = $1
    LIMIT 1
"#;

const UPSERT_REPORT_RUN_SQL: &str = r#"
    INSERT INTO report_runs (
        run_id,
        schedule_id,
        cadence,
        window_key,
        window_started_at_utc,
        window_ended_at_utc,
        status,
        reason_code,
        correlation_id,
        source_context,
        actor_id,
        run_started_at_utc,
        run_finished_at_utc,
        alert_emitted_at_utc,
        runbook_url,
        impacted_system,
        created_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5::timestamptz, $6::timestamptz, $7, $8, $9, $10, $11,
        $12::timestamptz, $13::timestamptz, $14::timestamptz, $15, $16, $17::timestamptz, $18::timestamptz
    )
    ON CONFLICT (run_id)
    DO UPDATE SET
        schedule_id = EXCLUDED.schedule_id,
        cadence = EXCLUDED.cadence,
        window_key = EXCLUDED.window_key,
        window_started_at_utc = EXCLUDED.window_started_at_utc,
        window_ended_at_utc = EXCLUDED.window_ended_at_utc,
        status = EXCLUDED.status,
        reason_code = EXCLUDED.reason_code,
        correlation_id = EXCLUDED.correlation_id,
        source_context = EXCLUDED.source_context,
        actor_id = EXCLUDED.actor_id,
        run_started_at_utc = EXCLUDED.run_started_at_utc,
        run_finished_at_utc = EXCLUDED.run_finished_at_utc,
        alert_emitted_at_utc = EXCLUDED.alert_emitted_at_utc,
        runbook_url = EXCLUDED.runbook_url,
        impacted_system = EXCLUDED.impacted_system,
        created_at_utc = EXCLUDED.created_at_utc,
        updated_at_utc = EXCLUDED.updated_at_utc
    RETURNING
        run_id,
        schedule_id,
        cadence,
        window_key,
        to_char(window_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_started_at_utc,
        to_char(window_ended_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_ended_at_utc,
        status,
        reason_code,
        correlation_id,
        source_context,
        actor_id,
        to_char(run_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS run_started_at_utc,
        CASE
            WHEN run_finished_at_utc IS NULL THEN NULL
            ELSE to_char(run_finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS run_finished_at_utc,
        CASE
            WHEN alert_emitted_at_utc IS NULL THEN NULL
            ELSE to_char(alert_emitted_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS alert_emitted_at_utc,
        runbook_url,
        impacted_system,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
"#;

const LOAD_REPORT_RUN_BY_WINDOW_SQL: &str = r#"
    SELECT
        run_id,
        schedule_id,
        cadence,
        window_key,
        to_char(window_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_started_at_utc,
        to_char(window_ended_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_ended_at_utc,
        status,
        reason_code,
        correlation_id,
        source_context,
        actor_id,
        to_char(run_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS run_started_at_utc,
        CASE
            WHEN run_finished_at_utc IS NULL THEN NULL
            ELSE to_char(run_finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS run_finished_at_utc,
        CASE
            WHEN alert_emitted_at_utc IS NULL THEN NULL
            ELSE to_char(alert_emitted_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS alert_emitted_at_utc,
        runbook_url,
        impacted_system,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM report_runs
    WHERE schedule_id = $1
      AND window_key = $2
    ORDER BY window_started_at_utc DESC, run_id ASC
    LIMIT 1
"#;

const LOAD_REPORT_RUN_HISTORY_SQL: &str = r#"
    SELECT
        run_id,
        schedule_id,
        cadence,
        window_key,
        to_char(window_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_started_at_utc,
        to_char(window_ended_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_ended_at_utc,
        status,
        reason_code,
        correlation_id,
        source_context,
        actor_id,
        to_char(run_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS run_started_at_utc,
        CASE
            WHEN run_finished_at_utc IS NULL THEN NULL
            ELSE to_char(run_finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS run_finished_at_utc,
        CASE
            WHEN alert_emitted_at_utc IS NULL THEN NULL
            ELSE to_char(alert_emitted_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS alert_emitted_at_utc,
        runbook_url,
        impacted_system,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM report_runs
    WHERE schedule_id = $1
    ORDER BY window_started_at_utc DESC, run_id ASC
    LIMIT $2
"#;

const REPORT_SCHEDULE_QUERY_FAILED: &str = "report_schedule_query_failed";
const REPORT_SCHEDULE_CONSTRAINT_VIOLATION: &str = "report_schedule_constraint_violation";
const REPORT_SCHEDULE_ROW_DECODE_FAILED: &str = "report_schedule_row_decode_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSchedulePersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingScheduleValidationIssue>,
}

impl ReportSchedulePersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ReportingScheduleValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingScheduleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REPORT_SCHEDULE_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REPORT_SCHEDULE_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: impl Display) -> Self {
        Self {
            code: REPORT_SCHEDULE_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ReportSchedulePersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReportSchedulePersistenceError {}

pub async fn upsert_report_schedule(
    pool: &PgPool,
    schedule: &ReportSchedule,
) -> Result<ReportSchedule, ReportSchedulePersistenceError> {
    let canonical = canonicalize_schedule(schedule)?;
    let row = sqlx::query(UPSERT_REPORT_SCHEDULE_SQL)
        .bind(&canonical.schedule_id)
        .bind(canonical.cadence.as_str())
        .bind(canonical.status.as_str())
        .bind(&canonical.next_run_at_utc)
        .bind(&canonical.actor_id)
        .bind(&canonical.actor_role)
        .bind(&canonical.reason_code)
        .bind(&canonical.correlation_id)
        .bind(&canonical.runbook_url)
        .bind(&canonical.created_at_utc)
        .bind(&canonical.updated_at_utc)
        .bind(canonical.paused_at_utc.as_deref())
        .fetch_one(pool)
        .await
        .map_err(|error| classify_query_error("upsert_report_schedule", error))?;
    decode_schedule_row(row)
}

pub async fn load_report_schedule_by_id(
    pool: &PgPool,
    schedule_id: &str,
) -> Result<Option<ReportSchedule>, ReportSchedulePersistenceError> {
    let normalized_schedule_id = normalize_lookup_identifier("schedule_id", schedule_id)?;
    let row = sqlx::query(LOAD_REPORT_SCHEDULE_BY_ID_SQL)
        .bind(&normalized_schedule_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_report_schedule_by_id", error))?;
    row.map(decode_schedule_row).transpose()
}

pub async fn load_due_report_schedules(
    pool: &PgPool,
    as_of_utc: &str,
    limit: i64,
) -> Result<Vec<ReportSchedule>, ReportSchedulePersistenceError> {
    let normalized_limit = normalize_limit(limit, 1, 200)?;
    let rows = sqlx::query(LOAD_DUE_REPORT_SCHEDULES_SQL)
        .bind(as_of_utc.trim())
        .bind(normalized_limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_due_report_schedules", error))?;
    rows.into_iter().map(decode_schedule_row).collect()
}

pub async fn upsert_report_run(
    pool: &PgPool,
    run: &ReportRunRecord,
) -> Result<ReportRunRecord, ReportSchedulePersistenceError> {
    let canonical = canonicalize_run(run)?;
    let previous_state = load_run_status(pool, &canonical.run_id).await?;
    validate_run_state_transition(previous_state, canonical.status).map_err(map_contract_error)?;

    let row = sqlx::query(UPSERT_REPORT_RUN_SQL)
        .bind(&canonical.run_id)
        .bind(&canonical.schedule_id)
        .bind(canonical.cadence.as_str())
        .bind(&canonical.window_key)
        .bind(&canonical.window_started_at_utc)
        .bind(&canonical.window_ended_at_utc)
        .bind(canonical.status.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.correlation_id)
        .bind(&canonical.source_context)
        .bind(canonical.actor_id.as_deref())
        .bind(&canonical.run_started_at_utc)
        .bind(canonical.run_finished_at_utc.as_deref())
        .bind(canonical.alert_emitted_at_utc.as_deref())
        .bind(canonical.runbook_url.as_deref())
        .bind(canonical.impacted_system.as_deref())
        .bind(&canonical.created_at_utc)
        .bind(&canonical.updated_at_utc)
        .fetch_one(pool)
        .await
        .map_err(|error| classify_query_error("upsert_report_run", error))?;
    decode_run_row(row)
}

pub async fn load_report_run_by_window_key(
    pool: &PgPool,
    schedule_id: &str,
    window_key: &str,
) -> Result<Option<ReportRunRecord>, ReportSchedulePersistenceError> {
    let normalized_schedule_id = normalize_lookup_identifier("schedule_id", schedule_id)?;
    let normalized_window_key = normalize_lookup_identifier("window_key", window_key)?;
    let row = sqlx::query(LOAD_REPORT_RUN_BY_WINDOW_SQL)
        .bind(&normalized_schedule_id)
        .bind(&normalized_window_key)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_report_run_by_window_key", error))?;
    row.map(decode_run_row).transpose()
}

pub async fn load_report_run_history(
    pool: &PgPool,
    schedule_id: &str,
    limit: i64,
) -> Result<Vec<ReportRunRecord>, ReportSchedulePersistenceError> {
    let normalized_schedule_id = normalize_lookup_identifier("schedule_id", schedule_id)?;
    let normalized_limit = normalize_limit(limit, 1, MAX_REPORT_RUN_HISTORY_LIMIT)?;
    let rows = sqlx::query(LOAD_REPORT_RUN_HISTORY_SQL)
        .bind(&normalized_schedule_id)
        .bind(normalized_limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_report_run_history", error))?;
    rows.into_iter().map(decode_run_row).collect()
}

async fn load_run_status(
    pool: &PgPool,
    run_id: &str,
) -> Result<Option<ReportingRunState>, ReportSchedulePersistenceError> {
    let row = sqlx::query(LOAD_REPORT_RUN_STATUS_SQL)
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_run_status", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status: String = row
        .try_get("status")
        .map_err(|error| ReportSchedulePersistenceError::row_decode_failure("status", error))?;
    let state = ReportingRunState::parse(&status).map_err(map_contract_error)?;
    Ok(Some(state))
}

fn decode_schedule_row(
    row: sqlx::postgres::PgRow,
) -> Result<ReportSchedule, ReportSchedulePersistenceError> {
    let cadence: String = row
        .try_get("cadence")
        .map_err(|error| ReportSchedulePersistenceError::row_decode_failure("cadence", error))?;
    let status: String = row
        .try_get("status")
        .map_err(|error| ReportSchedulePersistenceError::row_decode_failure("status", error))?;
    let schedule = ReportSchedule {
        schedule_id: row.try_get("schedule_id").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("schedule_id", error)
        })?,
        cadence: ReportingCadence::parse(&cadence).map_err(map_contract_error)?,
        status: ReportingScheduleState::parse(&status).map_err(map_contract_error)?,
        next_run_at_utc: row.try_get("next_run_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("next_run_at_utc", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("actor_id", error)
        })?,
        actor_role: row.try_get("actor_role").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("actor_role", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("correlation_id", error)
        })?,
        runbook_url: row.try_get("runbook_url").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("runbook_url", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
        paused_at_utc: row.try_get("paused_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("paused_at_utc", error)
        })?,
    };
    validate_report_schedule(&schedule).map_err(map_contract_error)?;
    Ok(schedule)
}

fn decode_run_row(
    row: sqlx::postgres::PgRow,
) -> Result<ReportRunRecord, ReportSchedulePersistenceError> {
    let cadence: String = row
        .try_get("cadence")
        .map_err(|error| ReportSchedulePersistenceError::row_decode_failure("cadence", error))?;
    let status: String = row
        .try_get("status")
        .map_err(|error| ReportSchedulePersistenceError::row_decode_failure("status", error))?;
    let run = ReportRunRecord {
        run_id: row
            .try_get("run_id")
            .map_err(|error| ReportSchedulePersistenceError::row_decode_failure("run_id", error))?,
        schedule_id: row.try_get("schedule_id").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("schedule_id", error)
        })?,
        cadence: ReportingCadence::parse(&cadence).map_err(map_contract_error)?,
        window_key: row.try_get("window_key").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("window_key", error)
        })?,
        window_started_at_utc: row.try_get("window_started_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("window_started_at_utc", error)
        })?,
        window_ended_at_utc: row.try_get("window_ended_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("window_ended_at_utc", error)
        })?,
        status: ReportingRunState::parse(&status).map_err(map_contract_error)?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("correlation_id", error)
        })?,
        source_context: row.try_get("source_context").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("source_context", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("actor_id", error)
        })?,
        run_started_at_utc: row.try_get("run_started_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("run_started_at_utc", error)
        })?,
        run_finished_at_utc: row.try_get("run_finished_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("run_finished_at_utc", error)
        })?,
        alert_emitted_at_utc: row.try_get("alert_emitted_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("alert_emitted_at_utc", error)
        })?,
        runbook_url: row.try_get("runbook_url").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("runbook_url", error)
        })?,
        impacted_system: row.try_get("impacted_system").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("impacted_system", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            ReportSchedulePersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };
    validate_report_run(&run).map_err(map_contract_error)?;
    Ok(run)
}

fn canonicalize_schedule(
    schedule: &ReportSchedule,
) -> Result<ReportSchedule, ReportSchedulePersistenceError> {
    let mut canonical = schedule.clone();
    canonical.schedule_id = normalize_lookup_identifier("schedule_id", &canonical.schedule_id)?;
    canonical.actor_id = normalize_lookup_identifier("actor_id", &canonical.actor_id)?;
    canonical.actor_role = normalize_lookup_identifier("actor_role", &canonical.actor_role)?;
    canonical.correlation_id =
        normalize_lookup_identifier("correlation_id", &canonical.correlation_id)?;
    canonical.reason_code = ReportingScheduleReasonCode::parse(&canonical.reason_code)
        .map_err(map_contract_error)?
        .code()
        .to_string();
    canonical.runbook_url = canonical.runbook_url.trim().to_string();
    validate_report_schedule(&canonical).map_err(map_contract_error)?;
    Ok(canonical)
}

fn canonicalize_run(
    run: &ReportRunRecord,
) -> Result<ReportRunRecord, ReportSchedulePersistenceError> {
    let mut canonical = run.clone();
    canonical.run_id = normalize_lookup_identifier("run_id", &canonical.run_id)?;
    canonical.schedule_id = normalize_lookup_identifier("schedule_id", &canonical.schedule_id)?;
    canonical.window_key = normalize_lookup_identifier("window_key", &canonical.window_key)?;
    canonical.correlation_id =
        normalize_lookup_identifier("correlation_id", &canonical.correlation_id)?;
    canonical.reason_code = ReportingScheduleReasonCode::parse(&canonical.reason_code)
        .map_err(map_contract_error)?
        .code()
        .to_string();
    canonical.source_context = canonical.source_context.trim().to_string();
    canonical.actor_id = canonical
        .actor_id
        .as_deref()
        .map(|value| normalize_lookup_identifier("actor_id", value))
        .transpose()?;
    canonical.runbook_url = normalize_optional_string(canonical.runbook_url.as_deref());
    canonical.impacted_system = normalize_optional_string(canonical.impacted_system.as_deref());
    validate_report_run(&canonical).map_err(map_contract_error)?;
    Ok(canonical)
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(str::to_string)
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, ReportSchedulePersistenceError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() < 3 || normalized.len() > 200 {
        return Err(ReportSchedulePersistenceError::invalid_payload(
            format!("{field} must contain 3-200 canonical characters"),
            vec![ReportingScheduleValidationIssue {
                field,
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-200 canonical characters"),
            }],
        ));
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        return Err(ReportSchedulePersistenceError::invalid_payload(
            format!("{field} contains unsupported characters"),
            vec![ReportingScheduleValidationIssue {
                field,
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: format!("{field} contains unsupported characters"),
            }],
        ));
    }
    Ok(normalized)
}

fn normalize_limit(limit: i64, min: i64, max: i64) -> Result<i64, ReportSchedulePersistenceError> {
    if limit < min || limit > max {
        return Err(ReportSchedulePersistenceError::invalid_payload(
            format!("limit must be between {min} and {max}"),
            vec![ReportingScheduleValidationIssue {
                field: "limit",
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: format!("limit must be between {min} and {max}"),
            }],
        ));
    }
    Ok(limit)
}

fn map_contract_error(error: ReportingScheduleContractError) -> ReportSchedulePersistenceError {
    ReportSchedulePersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ReportSchedulePersistenceError {
    if is_constraint_error(&error) {
        return ReportSchedulePersistenceError::constraint_violation(operation, error);
    }
    ReportSchedulePersistenceError::query_failure(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .map(|code| code.starts_with("23") || code == "55000")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::reporting_schedule::{
        DEFAULT_REPORT_RUN_HISTORY_LIMIT, build_reporting_window_for_boundary,
    };

    const REPORT_SCHEDULES_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407050000_report_schedules_report_runs.sql");

    fn sample_schedule(status: ReportingScheduleState) -> ReportSchedule {
        ReportSchedule {
            schedule_id: "report-schedule-daily".to_string(),
            cadence: ReportingCadence::Daily,
            status,
            next_run_at_utc: "2026-04-07T00:00:00Z".to_string(),
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            reason_code: ReportingScheduleReasonCode::Ready.code().to_string(),
            correlation_id: "corr-report-schedule-001".to_string(),
            runbook_url: "https://docs.example.com/operations/recurring-report-scheduling"
                .to_string(),
            created_at_utc: "2026-04-06T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
            paused_at_utc: if status == ReportingScheduleState::Paused {
                Some("2026-04-06T01:00:00Z".to_string())
            } else {
                None
            },
        }
    }

    fn sample_run(status: ReportingRunState) -> ReportRunRecord {
        let window = build_reporting_window_for_boundary(
            "report-schedule-daily",
            ReportingCadence::Daily,
            "2026-04-07T00:00:00Z",
        )
        .expect("window should build");
        ReportRunRecord {
            run_id: "report-run-daily-001".to_string(),
            schedule_id: "report-schedule-daily".to_string(),
            cadence: ReportingCadence::Daily,
            window_key: window.window_key,
            window_started_at_utc: window.window_start_at_utc,
            window_ended_at_utc: window.window_end_at_utc,
            status,
            reason_code: ReportingScheduleReasonCode::RunSucceeded.code().to_string(),
            correlation_id: "corr-report-run-001".to_string(),
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
    fn migration_scope_is_limited_to_report_schedules_and_runs() {
        assert!(
            REPORT_SCHEDULES_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS report_schedules")
        );
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS report_runs"));
        assert!(!REPORT_SCHEDULES_MIGRATION_SQL.contains("api_contract_versions"));
        assert!(!REPORT_SCHEDULES_MIGRATION_SQL.contains("export_jobs"));
    }

    #[test]
    fn migration_enforces_required_checks_and_indexes() {
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("report_schedules_cadence_check"));
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("report_schedules_status_check"));
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("report_runs_cadence_check"));
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("report_runs_status_check"));
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("report_runs_unique_schedule_window_key"));
        assert!(
            REPORT_SCHEDULES_MIGRATION_SQL.contains("idx_report_schedules_cadence_status_next_run")
        );
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("idx_report_runs_schedule_window"));
        assert!(REPORT_SCHEDULES_MIGRATION_SQL.contains("idx_report_runs_correlation_time"));
    }

    #[test]
    fn adapter_queries_remain_deterministic_for_due_and_history_lookups() {
        assert!(LOAD_DUE_REPORT_SCHEDULES_SQL.contains("status = 'active'"));
        assert!(
            LOAD_DUE_REPORT_SCHEDULES_SQL.contains("ORDER BY next_run_at_utc ASC, schedule_id ASC")
        );
        assert!(
            LOAD_REPORT_RUN_HISTORY_SQL.contains("ORDER BY window_started_at_utc DESC, run_id ASC")
        );
        assert!(
            LOAD_REPORT_RUN_BY_WINDOW_SQL
                .contains("ORDER BY window_started_at_utc DESC, run_id ASC")
        );
    }

    #[test]
    fn canonicalization_normalizes_identifiers_and_reason_codes() {
        let mut schedule = sample_schedule(ReportingScheduleState::Active);
        schedule.schedule_id = " REPORT-SCHEDULE-DAILY ".to_string();
        schedule.reason_code = "reporting_schedule_ready".to_string();
        let canonical_schedule =
            canonicalize_schedule(&schedule).expect("schedule canonicalization should succeed");
        assert_eq!(canonical_schedule.schedule_id, "report-schedule-daily");
        assert_eq!(
            canonical_schedule.reason_code,
            ReportingScheduleReasonCode::Ready.code()
        );

        let mut run = sample_run(ReportingRunState::Succeeded);
        run.run_id = " REPORT-RUN-DAILY-001 ".to_string();
        run.reason_code = "reporting_schedule_run_succeeded".to_string();
        let canonical_run = canonicalize_run(&run).expect("run canonicalization should succeed");
        assert_eq!(canonical_run.run_id, "report-run-daily-001");
        assert_eq!(
            canonical_run.reason_code,
            ReportingScheduleReasonCode::RunSucceeded.code()
        );
    }

    #[test]
    fn validation_rejects_invalid_payload_shapes() {
        let mut schedule = sample_schedule(ReportingScheduleState::Active);
        schedule.next_run_at_utc = "2026-04-07T00:00:00+01:00".to_string();
        let schedule_error = canonicalize_schedule(&schedule)
            .expect_err("non-UTC schedule timestamps should fail validation");
        assert_eq!(
            schedule_error.code,
            ReportingScheduleReasonCode::InvalidPayload.code()
        );

        let mut run = sample_run(ReportingRunState::Succeeded);
        run.run_finished_at_utc = None;
        let run_error =
            canonicalize_run(&run).expect_err("terminal run without finish timestamp should fail");
        assert_eq!(
            run_error.code,
            ReportingScheduleReasonCode::InvalidPayload.code()
        );
    }

    #[test]
    fn run_transition_rules_fail_closed_for_invalid_transitions() {
        let transition_error = validate_run_state_transition(
            Some(ReportingRunState::Succeeded),
            ReportingRunState::Failed,
        )
        .expect_err("succeeded -> failed transition should be rejected");
        assert_eq!(
            transition_error.code,
            ReportingScheduleReasonCode::InvalidPayload.code()
        );
    }

    #[test]
    fn history_limit_validation_enforces_bounds() {
        assert_eq!(
            normalize_limit(
                DEFAULT_REPORT_RUN_HISTORY_LIMIT,
                1,
                MAX_REPORT_RUN_HISTORY_LIMIT
            )
            .expect("default limit should pass"),
            DEFAULT_REPORT_RUN_HISTORY_LIMIT
        );
        assert!(normalize_limit(0, 1, 10).is_err());
        assert!(normalize_limit(11, 1, 10).is_err());
    }
}
