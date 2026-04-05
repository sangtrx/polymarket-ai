use domain::risk::{
    OrderEventOffsetCursor, USER_STREAM_AUTH_STATE_PARTITION_KEY, UserStreamAuthState,
    UserStreamAuthTransition, UserStreamContractError, UserStreamEvent, UserStreamOrderingDecision,
    UserStreamReasonCode, UserStreamValidationIssue, evaluate_user_stream_ordering,
    normalize_user_stream_idempotency_key, validate_order_event_offset_cursor,
    validate_user_stream_auth_transition, validate_user_stream_event,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_ACCEPTED_USER_STREAM_EVENT_SQL: &str = r#"
    INSERT INTO user_stream_events (
        event_id,
        event_kind,
        event_status,
        market_id,
        asset_id,
        order_id,
        trade_id,
        partition_key,
        idempotency_key,
        event_offset,
        reason_code,
        correlation_id,
        event_timestamp_utc,
        observed_at_utc,
        ingested_at_utc,
        ingestion_latency_seconds,
        raw_payload
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13::timestamptz, $14::timestamptz, $15::timestamptz, $16, $17
    )
"#;

const LOAD_ORDER_EVENT_CURSOR_FOR_UPDATE_SQL: &str = r#"
    SELECT
        partition_key,
        last_event_id,
        last_event_key,
        last_event_offset,
        auth_state,
        block_new_intents,
        reason_code,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM order_event_offsets
    WHERE partition_key = $1
    FOR UPDATE
"#;

const UPSERT_ORDER_EVENT_CURSOR_SQL: &str = r#"
    INSERT INTO order_event_offsets (
        partition_key,
        last_event_id,
        last_event_key,
        last_event_offset,
        auth_state,
        block_new_intents,
        reason_code,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz
    )
    ON CONFLICT (partition_key) DO UPDATE SET
        last_event_id = EXCLUDED.last_event_id,
        last_event_key = EXCLUDED.last_event_key,
        last_event_offset = EXCLUDED.last_event_offset,
        auth_state = EXCLUDED.auth_state,
        block_new_intents = EXCLUDED.block_new_intents,
        reason_code = EXCLUDED.reason_code,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_USER_STREAM_AUTH_CURSOR_SQL: &str = r#"
    SELECT
        partition_key,
        last_event_id,
        last_event_key,
        last_event_offset,
        auth_state,
        block_new_intents,
        reason_code,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM order_event_offsets
    WHERE partition_key = $1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStreamPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<UserStreamValidationIssue>,
}

impl UserStreamPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<UserStreamValidationIssue>,
    ) -> Self {
        Self {
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn persistence_unavailable(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: UserStreamReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: UserStreamReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: UserStreamReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for UserStreamPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for UserStreamPersistenceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStreamPersistDisposition {
    Accepted,
    Duplicate,
    OutOfOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStreamPersistOutcome {
    pub disposition: UserStreamPersistDisposition,
    pub reason_code: String,
}

pub async fn persist_user_stream_event(
    pool: &PgPool,
    event: &UserStreamEvent,
    raw_payload: &serde_json::Value,
    auth_state: UserStreamAuthState,
    block_new_intents: bool,
) -> Result<UserStreamPersistOutcome, UserStreamPersistenceError> {
    validate_user_stream_event(event).map_err(map_contract_error)?;
    if raw_payload.is_null() {
        return Err(UserStreamPersistenceError::invalid_payload(
            "raw_payload cannot be null",
            Vec::new(),
        ));
    }

    let partition_key = normalize_user_stream_idempotency_key(&event.partition_key);
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_user_stream_event_tx", error))?;

    let row = sqlx::query(LOAD_ORDER_EVENT_CURSOR_FOR_UPDATE_SQL)
        .bind(&partition_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| classify_query_error("load_order_event_cursor_for_update", error))?;

    let cursor = row.map(decode_order_event_offset_cursor_row).transpose()?;
    let ordering =
        evaluate_user_stream_ordering(cursor.as_ref(), event).map_err(map_contract_error)?;

    match ordering {
        UserStreamOrderingDecision::Duplicate { reason_code } => Ok(UserStreamPersistOutcome {
            disposition: UserStreamPersistDisposition::Duplicate,
            reason_code: reason_code.to_string(),
        }),
        UserStreamOrderingDecision::OutOfOrder { reason_code } => Ok(UserStreamPersistOutcome {
            disposition: UserStreamPersistDisposition::OutOfOrder,
            reason_code: reason_code.to_string(),
        }),
        UserStreamOrderingDecision::Accept { reason_code } => {
            let normalized_key = normalize_user_stream_idempotency_key(&event.idempotency_key);
            sqlx::query(INSERT_ACCEPTED_USER_STREAM_EVENT_SQL)
                .bind(&event.event_id)
                .bind(event.event_kind.as_str())
                .bind(event.event_status.as_str())
                .bind(&event.market_id)
                .bind(&event.asset_id)
                .bind(&event.order_id)
                .bind(&event.trade_id)
                .bind(&partition_key)
                .bind(&normalized_key)
                .bind(event.event_offset)
                .bind(reason_code)
                .bind(&event.correlation_id)
                .bind(&event.event_timestamp_utc)
                .bind(&event.observed_at_utc)
                .bind(&event.ingested_at_utc)
                .bind(event.ingestion_latency_seconds)
                .bind(raw_payload)
                .execute(&mut *tx)
                .await
                .map_err(|error| {
                    classify_query_error("insert_accepted_user_stream_event", error)
                })?;

            sqlx::query(UPSERT_ORDER_EVENT_CURSOR_SQL)
                .bind(&partition_key)
                .bind(&event.event_id)
                .bind(&normalized_key)
                .bind(event.event_offset)
                .bind(auth_state.as_str())
                .bind(block_new_intents)
                .bind(reason_code)
                .bind(&event.correlation_id)
                .bind(&event.ingested_at_utc)
                .execute(&mut *tx)
                .await
                .map_err(|error| classify_query_error("upsert_order_event_cursor", error))?;

            tx.commit()
                .await
                .map_err(|error| classify_query_error("commit_user_stream_event_tx", error))?;

            Ok(UserStreamPersistOutcome {
                disposition: UserStreamPersistDisposition::Accepted,
                reason_code: reason_code.to_string(),
            })
        }
    }
}

pub async fn persist_user_stream_auth_transition(
    pool: &PgPool,
    transition: &UserStreamAuthTransition,
) -> Result<(), UserStreamPersistenceError> {
    validate_user_stream_auth_transition(transition).map_err(map_contract_error)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_user_stream_auth_transition_tx", error))?;

    sqlx::query(UPSERT_ORDER_EVENT_CURSOR_SQL)
        .bind(USER_STREAM_AUTH_STATE_PARTITION_KEY)
        .bind(&transition.transition_id)
        .bind("auth_state_transition")
        .bind(transition.transition_offset)
        .bind(transition.auth_state.as_str())
        .bind(transition.block_new_intents)
        .bind(&transition.reason_code)
        .bind(&transition.correlation_id)
        .bind(&transition.observed_at_utc)
        .execute(&mut *tx)
        .await
        .map_err(|error| classify_query_error("upsert_user_stream_auth_transition", error))?;

    tx.commit()
        .await
        .map_err(|error| classify_query_error("commit_user_stream_auth_transition_tx", error))?;
    Ok(())
}

pub async fn load_user_stream_auth_cursor(
    pool: &PgPool,
) -> Result<Option<OrderEventOffsetCursor>, UserStreamPersistenceError> {
    let row = sqlx::query(LOAD_USER_STREAM_AUTH_CURSOR_SQL)
        .bind(USER_STREAM_AUTH_STATE_PARTITION_KEY)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_user_stream_auth_cursor", error))?;
    row.map(decode_order_event_offset_cursor_row).transpose()
}

fn decode_order_event_offset_cursor_row(
    row: sqlx::postgres::PgRow,
) -> Result<OrderEventOffsetCursor, UserStreamPersistenceError> {
    let auth_state: String = row
        .try_get("auth_state")
        .map_err(|error| UserStreamPersistenceError::row_decode_failure("auth_state", error))?;
    let auth_state = UserStreamAuthState::parse(&auth_state).map_err(map_contract_error)?;

    let cursor = OrderEventOffsetCursor {
        partition_key: row.try_get("partition_key").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("partition_key", error)
        })?,
        last_event_id: row.try_get("last_event_id").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("last_event_id", error)
        })?,
        last_event_key: row.try_get("last_event_key").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("last_event_key", error)
        })?,
        last_event_offset: row.try_get("last_event_offset").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("last_event_offset", error)
        })?,
        auth_state,
        block_new_intents: row.try_get("block_new_intents").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("block_new_intents", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            UserStreamPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };
    validate_order_event_offset_cursor(&cursor).map_err(map_contract_error)?;
    Ok(cursor)
}

fn map_contract_error(error: UserStreamContractError) -> UserStreamPersistenceError {
    UserStreamPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> UserStreamPersistenceError {
    if is_constraint_error(&error) {
        return UserStreamPersistenceError::constraint_violation(operation, error);
    }
    UserStreamPersistenceError::persistence_unavailable(operation, error)
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
    use domain::risk::{UserStreamEventKind, UserStreamEventStatus};

    const USER_STREAM_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406030000_user_stream_ingestion.sql");

    fn sample_event() -> UserStreamEvent {
        UserStreamEvent {
            event_id: "order::order-1::100".to_string(),
            event_kind: UserStreamEventKind::Order,
            event_status: UserStreamEventStatus::Placement,
            market_id: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            asset_id:
                "106585164761922456203746651621390029417453862034640469075081961934906147433548"
                    .to_string(),
            order_id: "order-1".to_string(),
            trade_id: None,
            partition_key: "order-1".to_string(),
            idempotency_key: "order-1::100::placement".to_string(),
            event_offset: 100,
            event_timestamp_utc: "2026-04-06T00:00:00Z".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            ingested_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
            correlation_id: "corr-user-stream-001".to_string(),
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
        }
    }

    fn sample_cursor() -> OrderEventOffsetCursor {
        OrderEventOffsetCursor {
            partition_key: "order-1".to_string(),
            last_event_id: "order::order-1::100".to_string(),
            last_event_key: "order-1::100::placement".to_string(),
            last_event_offset: 100,
            auth_state: UserStreamAuthState::Authenticated,
            block_new_intents: false,
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
            correlation_id: "corr-user-stream-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:01Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_user_stream_schema_scope() {
        assert!(
            USER_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS user_stream_events")
        );
        assert!(
            USER_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS order_event_offsets")
        );
        assert!(!USER_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS market_ticks"));
        assert!(
            !USER_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS market_stream_health")
        );
        assert!(!USER_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS orders"));
        assert!(
            !USER_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS freshness_gate_events")
        );
    }

    #[test]
    fn migration_enforces_constraints_indexes_and_monotonicity_trigger() {
        assert!(USER_STREAM_MIGRATION_SQL.contains("event_kind IN ('order', 'trade')"));
        assert!(USER_STREAM_MIGRATION_SQL.contains("event_offset >= 0"));
        assert!(
            USER_STREAM_MIGRATION_SQL.contains("auth_state IN ('authenticated', 'auth_expired')")
        );
        assert!(USER_STREAM_MIGRATION_SQL.contains("idx_user_stream_events_order_time"));
        assert!(USER_STREAM_MIGRATION_SQL.contains("idx_order_event_offsets_blocking_state"));
        assert!(USER_STREAM_MIGRATION_SQL.contains("enforce_order_event_offset_monotonicity"));
        assert!(USER_STREAM_MIGRATION_SQL.contains("trg_order_event_offsets_monotonicity"));
    }

    #[test]
    fn ordering_decision_duplicate_is_deterministic_for_same_offset_key() {
        let event = sample_event();
        let cursor = sample_cursor();
        let decision = evaluate_user_stream_ordering(Some(&cursor), &event)
            .expect("equal offset + equal key should be duplicate");
        assert_eq!(
            decision,
            UserStreamOrderingDecision::Duplicate {
                reason_code: UserStreamReasonCode::DuplicateEvent.code(),
            }
        );
    }

    #[test]
    fn auth_transition_validation_requires_fail_closed_auth_expired_state() {
        let transition = UserStreamAuthTransition {
            transition_id: "auth-transition-1".to_string(),
            transition_offset: 1,
            auth_state: UserStreamAuthState::AuthExpired,
            block_new_intents: false,
            reason_code: UserStreamReasonCode::AuthExpired.code().to_string(),
            correlation_id: "corr-auth-001".to_string(),
            observed_at_utc: "2026-04-06T00:00:01Z".to_string(),
        };
        let error = validate_user_stream_auth_transition(&transition)
            .expect_err("auth-expired transition must be fail-closed");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "block_new_intents")
        );
    }
}
