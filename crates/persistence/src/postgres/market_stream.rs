use domain::risk::{
    MarketStreamContractError, MarketStreamHealth, MarketStreamHealthStatus,
    MarketStreamReasonCode, MarketStreamTick, MarketStreamValidationIssue, QuarantinedMarketEvent,
    validate_market_stream_health, validate_market_stream_tick, validate_quarantined_market_event,
};
use serde_json::json;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_ACCEPTED_MARKET_TICK_SQL: &str = r#"
    INSERT INTO market_ticks (
        tick_id,
        market_id,
        asset_id,
        cluster_id,
        best_bid,
        best_ask,
        top_bid_depth,
        top_ask_depth,
        last_trade_price,
        tick_size,
        market_status,
        ingest_status,
        quarantine_reason_code,
        raw_payload,
        ingestion_latency_seconds,
        correlation_id,
        observed_at_utc,
        ingested_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'accepted', NULL, $12, $13, $14, $15::timestamptz, $16::timestamptz
    )
"#;

const INSERT_QUARANTINED_MARKET_EVENT_SQL: &str = r#"
    INSERT INTO market_ticks (
        tick_id,
        market_id,
        asset_id,
        cluster_id,
        best_bid,
        best_ask,
        top_bid_depth,
        top_ask_depth,
        last_trade_price,
        tick_size,
        market_status,
        ingest_status,
        quarantine_reason_code,
        raw_payload,
        ingestion_latency_seconds,
        correlation_id,
        observed_at_utc,
        ingested_at_utc
    ) VALUES (
        $1, NULL, NULL, NULL, NULL, NULL, '[]'::jsonb, '[]'::jsonb, NULL, NULL, NULL, 'quarantined', $2, $3, $4, $5, $6::timestamptz, $7::timestamptz
    )
"#;

const INSERT_MARKET_STREAM_HEALTH_SQL: &str = r#"
    INSERT INTO market_stream_health (
        health_event_id,
        stream_name,
        health_status,
        reason_code,
        backlog_seconds,
        sustained_backlog_seconds,
        heartbeat_gap_seconds,
        correlation_id,
        observed_at_utc,
        evidence
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz, $10
    )
"#;

const LOAD_LATEST_MARKET_STREAM_HEALTH_SQL: &str = r#"
    SELECT
        health_event_id,
        stream_name,
        health_status,
        reason_code,
        backlog_seconds,
        sustained_backlog_seconds,
        heartbeat_gap_seconds,
        correlation_id,
        to_char(observed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS observed_at_utc
    FROM market_stream_health
    WHERE stream_name = $1
    ORDER BY observed_at_utc DESC, health_event_id ASC
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketStreamPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<MarketStreamValidationIssue>,
}

impl MarketStreamPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<MarketStreamValidationIssue>,
    ) -> Self {
        Self {
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_stream_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_stream_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_stream_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for MarketStreamPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for MarketStreamPersistenceError {}

pub async fn insert_accepted_market_tick<'e, E>(
    executor: E,
    tick: &MarketStreamTick,
) -> Result<(), MarketStreamPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_market_stream_tick(tick).map_err(map_contract_error)?;

    let raw_payload = json!({
        "market_id": tick.market_id,
        "asset_id": tick.asset_id,
        "cluster_id": tick.cluster_id,
        "best_bid": tick.best_bid,
        "best_ask": tick.best_ask,
        "top_bid_depth": tick.top_bid_depth,
        "top_ask_depth": tick.top_ask_depth,
        "last_trade_price": tick.last_trade_price,
        "tick_size": tick.tick_size,
        "market_status": tick.market_status.as_str()
    });

    let result = sqlx::query(INSERT_ACCEPTED_MARKET_TICK_SQL)
        .bind(&tick.tick_id)
        .bind(&tick.market_id)
        .bind(&tick.asset_id)
        .bind(normalize_cluster_id(&tick.cluster_id))
        .bind(tick.best_bid)
        .bind(tick.best_ask)
        .bind(serde_json::to_value(&tick.top_bid_depth).map_err(|error| {
            MarketStreamPersistenceError::invalid_payload(
                format!("top_bid_depth serialization failed: {error}"),
                Vec::new(),
            )
        })?)
        .bind(serde_json::to_value(&tick.top_ask_depth).map_err(|error| {
            MarketStreamPersistenceError::invalid_payload(
                format!("top_ask_depth serialization failed: {error}"),
                Vec::new(),
            )
        })?)
        .bind(tick.last_trade_price)
        .bind(tick.tick_size)
        .bind(tick.market_status.as_str())
        .bind(raw_payload)
        .bind(tick.ingestion_latency_seconds)
        .bind(&tick.correlation_id)
        .bind(&tick.observed_at_utc)
        .bind(&tick.ingested_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_accepted_market_tick", error))?;

    if result.rows_affected() != 1 {
        return Err(MarketStreamPersistenceError::invalid_payload(
            format!(
                "insert_accepted_market_tick expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn insert_quarantined_market_event<'e, E>(
    executor: E,
    event: &QuarantinedMarketEvent,
) -> Result<(), MarketStreamPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_quarantined_market_event(event).map_err(map_contract_error)?;

    let result = sqlx::query(INSERT_QUARANTINED_MARKET_EVENT_SQL)
        .bind(&event.event_id)
        .bind(&event.reason_code)
        .bind(&event.raw_payload)
        .bind(event.ingestion_latency_seconds)
        .bind(&event.correlation_id)
        .bind(&event.observed_at_utc)
        .bind(&event.quarantined_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_quarantined_market_event", error))?;

    if result.rows_affected() != 1 {
        return Err(MarketStreamPersistenceError::invalid_payload(
            format!(
                "insert_quarantined_market_event expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn insert_market_stream_health<'e, E>(
    executor: E,
    health: &MarketStreamHealth,
) -> Result<(), MarketStreamPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_market_stream_health(health).map_err(map_contract_error)?;
    let evidence = json!({
        "status": health.status.as_str(),
        "reason_code": health.reason_code,
        "backlog_seconds": health.backlog_seconds,
        "sustained_backlog_seconds": health.sustained_backlog_seconds,
        "heartbeat_gap_seconds": health.heartbeat_gap_seconds,
    });

    let result = sqlx::query(INSERT_MARKET_STREAM_HEALTH_SQL)
        .bind(&health.health_event_id)
        .bind(&health.stream_name)
        .bind(health.status.as_str())
        .bind(&health.reason_code)
        .bind(health.backlog_seconds)
        .bind(health.sustained_backlog_seconds)
        .bind(health.heartbeat_gap_seconds)
        .bind(&health.correlation_id)
        .bind(&health.observed_at_utc)
        .bind(evidence)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_market_stream_health", error))?;

    if result.rows_affected() != 1 {
        return Err(MarketStreamPersistenceError::invalid_payload(
            format!(
                "insert_market_stream_health expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_latest_market_stream_health<'e, E>(
    executor: E,
    stream_name: &str,
) -> Result<Option<MarketStreamHealth>, MarketStreamPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("stream_name", stream_name)?;

    let row = sqlx::query(LOAD_LATEST_MARKET_STREAM_HEALTH_SQL)
        .bind(stream_name.trim())
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            MarketStreamPersistenceError::query_failure("load_latest_market_stream_health", error)
        })?;

    row.map(decode_market_stream_health_row).transpose()
}

fn decode_market_stream_health_row(
    row: sqlx::postgres::PgRow,
) -> Result<MarketStreamHealth, MarketStreamPersistenceError> {
    let status: String = row.try_get("health_status").map_err(|error| {
        MarketStreamPersistenceError::row_decode_failure("health_status", error)
    })?;
    let status = match status.as_str() {
        "healthy" => MarketStreamHealthStatus::Healthy,
        "degraded" => MarketStreamHealthStatus::Degraded,
        _ => {
            return Err(MarketStreamPersistenceError::invalid_payload(
                format!("unexpected health status `{status}`"),
                Vec::new(),
            ));
        }
    };

    let health = MarketStreamHealth {
        health_event_id: row.try_get("health_event_id").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("health_event_id", error)
        })?,
        stream_name: row.try_get("stream_name").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("stream_name", error)
        })?,
        status,
        reason_code: row.try_get("reason_code").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("reason_code", error)
        })?,
        backlog_seconds: row.try_get("backlog_seconds").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("backlog_seconds", error)
        })?,
        sustained_backlog_seconds: row.try_get("sustained_backlog_seconds").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("sustained_backlog_seconds", error)
        })?,
        heartbeat_gap_seconds: row.try_get("heartbeat_gap_seconds").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("heartbeat_gap_seconds", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        observed_at_utc: row.try_get("observed_at_utc").map_err(|error| {
            MarketStreamPersistenceError::row_decode_failure("observed_at_utc", error)
        })?,
    };

    validate_market_stream_health(&health).map_err(map_contract_error)?;
    Ok(health)
}

fn map_contract_error(error: MarketStreamContractError) -> MarketStreamPersistenceError {
    MarketStreamPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> MarketStreamPersistenceError {
    if is_constraint_error(&error) {
        return MarketStreamPersistenceError::constraint_violation(operation, error);
    }
    MarketStreamPersistenceError::query_failure(operation, error)
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

fn normalize_cluster_id(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), MarketStreamPersistenceError> {
    if value.trim().is_empty() {
        return Err(MarketStreamPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::risk::{MarketDepthLevel, MarketStatus};

    const MARKET_STREAM_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406020000_market_stream_ingestion.sql");

    fn sample_tick() -> MarketStreamTick {
        MarketStreamTick {
            tick_id: "tick-1".to_string(),
            market_id: "market_yes_no_1".to_string(),
            asset_id: "asset-1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            best_bid: 0.51,
            best_ask: 0.53,
            top_bid_depth: vec![MarketDepthLevel {
                price: 0.51,
                size: 100.0,
            }],
            top_ask_depth: vec![MarketDepthLevel {
                price: 0.53,
                size: 120.0,
            }],
            last_trade_price: 0.52,
            tick_size: 0.01,
            market_status: MarketStatus::Trading,
            correlation_id: "corr-1".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            ingested_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
        }
    }

    fn sample_quarantined_event() -> QuarantinedMarketEvent {
        QuarantinedMarketEvent {
            event_id: "quarantine-1".to_string(),
            reason_code: MarketStreamReasonCode::InvalidPayload.code().to_string(),
            correlation_id: "corr-1".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            quarantined_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
            raw_payload: json!({
                "event_type": "book"
            }),
        }
    }

    fn sample_health() -> MarketStreamHealth {
        MarketStreamHealth {
            health_event_id: "health-1".to_string(),
            stream_name: "polymarket_market_stream".to_string(),
            status: MarketStreamHealthStatus::Healthy,
            reason_code: MarketStreamReasonCode::Healthy.code().to_string(),
            backlog_seconds: 1.0,
            sustained_backlog_seconds: 0.0,
            heartbeat_gap_seconds: 1.0,
            correlation_id: "corr-1".to_string(),
            observed_at_utc: "2026-04-06T00:00:01Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_market_stream_schema_scope() {
        assert!(MARKET_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS market_ticks"));
        assert!(
            MARKET_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS market_stream_health")
        );
        assert!(
            !MARKET_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS user_stream_events")
        );
        assert!(
            !MARKET_STREAM_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS order_event_offsets")
        );
        assert!(
            !MARKET_STREAM_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS freshness_gate_events")
        );
    }

    #[test]
    fn migration_enforces_constraints_and_indexes() {
        assert!(
            MARKET_STREAM_MIGRATION_SQL.contains("ingest_status IN ('accepted', 'quarantined')")
        );
        assert!(MARKET_STREAM_MIGRATION_SQL.contains("health_status IN ('healthy', 'degraded')"));
        assert!(MARKET_STREAM_MIGRATION_SQL.contains("ingestion_latency_seconds >= 0"));
        assert!(MARKET_STREAM_MIGRATION_SQL.contains("idx_market_ticks_market_time"));
        assert!(MARKET_STREAM_MIGRATION_SQL.contains("idx_market_stream_health_stream_time"));
        assert!(MARKET_STREAM_MIGRATION_SQL.contains("idx_market_stream_health_status_time"));
    }

    #[test]
    fn accepted_tick_validation_rejects_negative_latency() {
        let mut tick = sample_tick();
        tick.ingestion_latency_seconds = -0.1;
        let error = validate_market_stream_tick(&tick)
            .expect_err("negative ingestion latency should fail validation");
        assert_eq!(error.code, MarketStreamReasonCode::InvalidPayload.code());
    }

    #[test]
    fn quarantined_event_validation_rejects_unknown_reason_code() {
        let mut event = sample_quarantined_event();
        event.reason_code = "market_stream_unknown_reason".to_string();
        let error = validate_quarantined_market_event(&event)
            .expect_err("unknown reason code should fail validation");
        assert_eq!(error.code, MarketStreamReasonCode::InvalidPayload.code());
    }

    #[test]
    fn market_stream_health_validation_accepts_expected_payload() {
        assert!(validate_market_stream_health(&sample_health()).is_ok());
    }

    #[test]
    fn latest_health_lookup_query_orders_deterministically() {
        assert!(
            LOAD_LATEST_MARKET_STREAM_HEALTH_SQL
                .contains("ORDER BY observed_at_utc DESC, health_event_id ASC")
        );
    }
}
