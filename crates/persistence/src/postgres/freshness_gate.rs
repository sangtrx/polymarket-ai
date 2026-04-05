use domain::risk::{
    FreshnessGateContractError, FreshnessGateEvent, FreshnessGateReasonCode,
    FreshnessGateTransition, FreshnessGateValidationIssue, validate_freshness_gate_event,
};
use serde_json::json;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_FRESHNESS_GATE_EVENT_SQL: &str = r#"
    INSERT INTO freshness_gate_events (
        event_id,
        transition,
        reason_code,
        pause_active,
        block_new_order_creation,
        market_data_age_seconds,
        user_data_age_seconds,
        stale_threshold_seconds,
        stability_window_seconds,
        max_breach_to_pause_seconds,
        stale_breach_detected_at_utc,
        pause_activated_at_utc,
        recovery_window_started_at_utc,
        recovery_confirmed_at_utc,
        breach_to_pause_latency_seconds,
        evaluated_at_utc,
        correlation_id,
        evidence
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12::timestamptz, $13::timestamptz, $14::timestamptz, $15, $16::timestamptz, $17, $18
    )
"#;

const LOAD_LATEST_FRESHNESS_GATE_EVENT_SQL: &str = r#"
    SELECT
        event_id,
        transition,
        reason_code,
        pause_active,
        block_new_order_creation,
        market_data_age_seconds,
        user_data_age_seconds,
        stale_threshold_seconds,
        stability_window_seconds,
        max_breach_to_pause_seconds,
        to_char(stale_breach_detected_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stale_breach_detected_at_utc,
        to_char(pause_activated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS pause_activated_at_utc,
        to_char(recovery_window_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS recovery_window_started_at_utc,
        to_char(recovery_confirmed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS recovery_confirmed_at_utc,
        breach_to_pause_latency_seconds,
        to_char(evaluated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS evaluated_at_utc,
        correlation_id
    FROM freshness_gate_events
    ORDER BY evaluated_at_utc DESC, event_id DESC
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshnessGatePersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<FreshnessGateValidationIssue>,
}

impl FreshnessGatePersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<FreshnessGateValidationIssue>,
    ) -> Self {
        Self {
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn persistence_unavailable(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: FreshnessGateReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: FreshnessGateReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: FreshnessGateReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for FreshnessGatePersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for FreshnessGatePersistenceError {}

pub async fn insert_freshness_gate_event<'e, E>(
    executor: E,
    event: &FreshnessGateEvent,
) -> Result<(), FreshnessGatePersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_freshness_gate_event(event).map_err(map_contract_error)?;
    let evidence = json!({
        "transition": event.transition.as_str(),
        "reason_code": event.reason_code,
        "pause_active": event.pause_active,
        "block_new_order_creation": event.block_new_order_creation,
        "market_data_age_seconds": event.market_data_age_seconds,
        "user_data_age_seconds": event.user_data_age_seconds,
        "stale_threshold_seconds": event.stale_threshold_seconds,
        "stability_window_seconds": event.stability_window_seconds,
        "max_breach_to_pause_seconds": event.max_breach_to_pause_seconds,
        "breach_to_pause_latency_seconds": event.breach_to_pause_latency_seconds,
    });

    let result = sqlx::query(INSERT_FRESHNESS_GATE_EVENT_SQL)
        .bind(&event.event_id)
        .bind(event.transition.as_str())
        .bind(&event.reason_code)
        .bind(event.pause_active)
        .bind(event.block_new_order_creation)
        .bind(event.market_data_age_seconds)
        .bind(event.user_data_age_seconds)
        .bind(event.stale_threshold_seconds)
        .bind(event.stability_window_seconds)
        .bind(event.max_breach_to_pause_seconds)
        .bind(&event.stale_breach_detected_at_utc)
        .bind(&event.pause_activated_at_utc)
        .bind(&event.recovery_window_started_at_utc)
        .bind(&event.recovery_confirmed_at_utc)
        .bind(event.breach_to_pause_latency_seconds)
        .bind(&event.evaluated_at_utc)
        .bind(&event.correlation_id)
        .bind(evidence)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_freshness_gate_event", error))?;

    if result.rows_affected() != 1 {
        return Err(FreshnessGatePersistenceError::invalid_payload(
            format!(
                "insert_freshness_gate_event expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_latest_freshness_gate_event<'e, E>(
    executor: E,
) -> Result<Option<FreshnessGateEvent>, FreshnessGatePersistenceError>
where
    E: PgExecutor<'e>,
{
    let row = sqlx::query(LOAD_LATEST_FRESHNESS_GATE_EVENT_SQL)
        .fetch_optional(executor)
        .await
        .map_err(|error| classify_query_error("load_latest_freshness_gate_event", error))?;
    row.map(decode_freshness_gate_event_row).transpose()
}

fn decode_freshness_gate_event_row(
    row: sqlx::postgres::PgRow,
) -> Result<FreshnessGateEvent, FreshnessGatePersistenceError> {
    let transition: String = row
        .try_get("transition")
        .map_err(|error| FreshnessGatePersistenceError::row_decode_failure("transition", error))?;
    let transition = FreshnessGateTransition::parse(&transition).map_err(map_contract_error)?;

    let event = FreshnessGateEvent {
        event_id: row.try_get("event_id").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("event_id", error)
        })?,
        transition,
        reason_code: row.try_get("reason_code").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("reason_code", error)
        })?,
        pause_active: row.try_get("pause_active").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("pause_active", error)
        })?,
        block_new_order_creation: row.try_get("block_new_order_creation").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("block_new_order_creation", error)
        })?,
        market_data_age_seconds: row.try_get("market_data_age_seconds").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("market_data_age_seconds", error)
        })?,
        user_data_age_seconds: row.try_get("user_data_age_seconds").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("user_data_age_seconds", error)
        })?,
        stale_threshold_seconds: row.try_get("stale_threshold_seconds").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("stale_threshold_seconds", error)
        })?,
        stability_window_seconds: row.try_get("stability_window_seconds").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("stability_window_seconds", error)
        })?,
        max_breach_to_pause_seconds: row.try_get("max_breach_to_pause_seconds").map_err(
            |error| {
                FreshnessGatePersistenceError::row_decode_failure(
                    "max_breach_to_pause_seconds",
                    error,
                )
            },
        )?,
        stale_breach_detected_at_utc: row.try_get("stale_breach_detected_at_utc").map_err(
            |error| {
                FreshnessGatePersistenceError::row_decode_failure(
                    "stale_breach_detected_at_utc",
                    error,
                )
            },
        )?,
        pause_activated_at_utc: row.try_get("pause_activated_at_utc").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("pause_activated_at_utc", error)
        })?,
        recovery_window_started_at_utc: row.try_get("recovery_window_started_at_utc").map_err(
            |error| {
                FreshnessGatePersistenceError::row_decode_failure(
                    "recovery_window_started_at_utc",
                    error,
                )
            },
        )?,
        recovery_confirmed_at_utc: row.try_get("recovery_confirmed_at_utc").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("recovery_confirmed_at_utc", error)
        })?,
        breach_to_pause_latency_seconds: row.try_get("breach_to_pause_latency_seconds").map_err(
            |error| {
                FreshnessGatePersistenceError::row_decode_failure(
                    "breach_to_pause_latency_seconds",
                    error,
                )
            },
        )?,
        evaluated_at_utc: row.try_get("evaluated_at_utc").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("evaluated_at_utc", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            FreshnessGatePersistenceError::row_decode_failure("correlation_id", error)
        })?,
    };

    validate_freshness_gate_event(&event).map_err(map_contract_error)?;
    Ok(event)
}

fn map_contract_error(error: FreshnessGateContractError) -> FreshnessGatePersistenceError {
    FreshnessGatePersistenceError::invalid_payload(error.message, error.field_errors)
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> FreshnessGatePersistenceError {
    if is_constraint_error(&error) {
        return FreshnessGatePersistenceError::constraint_violation(operation, error);
    }
    FreshnessGatePersistenceError::persistence_unavailable(operation, error)
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

    const FRESHNESS_GATE_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406040500_freshness_gate_events.sql");

    fn sample_event() -> FreshnessGateEvent {
        FreshnessGateEvent {
            event_id: "freshness::pause_activated::20260406000100".to_string(),
            transition: FreshnessGateTransition::PauseActivated,
            reason_code: FreshnessGateReasonCode::StaleBreach.code().to_string(),
            pause_active: true,
            block_new_order_creation: true,
            market_data_age_seconds: Some(31.0),
            user_data_age_seconds: Some(32.0),
            stale_threshold_seconds: 30.0,
            stability_window_seconds: 10.0,
            max_breach_to_pause_seconds: 5.0,
            stale_breach_detected_at_utc: Some("2026-04-06T00:01:00Z".to_string()),
            pause_activated_at_utc: Some("2026-04-06T00:01:00Z".to_string()),
            recovery_window_started_at_utc: None,
            recovery_confirmed_at_utc: None,
            breach_to_pause_latency_seconds: Some(0.0),
            evaluated_at_utc: "2026-04-06T00:01:00Z".to_string(),
            correlation_id: "corr-freshness-001".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_freshness_schema_scope() {
        assert!(
            FRESHNESS_GATE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS freshness_gate_events")
        );
        assert!(!FRESHNESS_GATE_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS market_ticks"));
        assert!(
            !FRESHNESS_GATE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS market_stream_health")
        );
        assert!(
            !FRESHNESS_GATE_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS user_stream_events")
        );
        assert!(
            !FRESHNESS_GATE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS order_event_offsets")
        );
    }

    #[test]
    fn migration_enforces_constraints_and_indexes_for_traceability() {
        assert!(FRESHNESS_GATE_MIGRATION_SQL.contains("market_data_age_seconds >= 0"));
        assert!(FRESHNESS_GATE_MIGRATION_SQL.contains("user_data_age_seconds >= 0"));
        assert!(
            FRESHNESS_GATE_MIGRATION_SQL
                .contains("breach_to_pause_latency_seconds <= max_breach_to_pause_seconds")
        );
        assert!(FRESHNESS_GATE_MIGRATION_SQL.contains("idx_freshness_gate_events_latest"));
        assert!(FRESHNESS_GATE_MIGRATION_SQL.contains("idx_freshness_gate_events_transition_time"));
        assert!(FRESHNESS_GATE_MIGRATION_SQL.contains("idx_freshness_gate_events_reason_time"));
        assert!(
            FRESHNESS_GATE_MIGRATION_SQL.contains("idx_freshness_gate_events_correlation_time")
        );
    }

    #[test]
    fn event_validation_accepts_pause_activated_payload() {
        assert!(validate_freshness_gate_event(&sample_event()).is_ok());
    }

    #[test]
    fn event_validation_rejects_recovery_confirmed_with_pause_still_active() {
        let mut event = sample_event();
        event.transition = FreshnessGateTransition::RecoveryConfirmed;
        event.reason_code = FreshnessGateReasonCode::RecoveryConfirmed
            .code()
            .to_string();
        event.pause_active = true;
        event.block_new_order_creation = true;
        event.recovery_confirmed_at_utc = Some("2026-04-06T00:01:30Z".to_string());

        let error = validate_freshness_gate_event(&event)
            .expect_err("recovery confirmed must clear pause and block flags");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "pause_active")
        );
    }

    #[test]
    fn latest_event_lookup_query_orders_deterministically() {
        assert!(
            LOAD_LATEST_FRESHNESS_GATE_EVENT_SQL
                .contains("ORDER BY evaluated_at_utc DESC, event_id DESC")
        );
    }
}
