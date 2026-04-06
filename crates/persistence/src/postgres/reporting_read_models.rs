use domain::reporting::{
    PerformanceReportingRow, PositionReportingRow, ReportingContractError,
    ReportingEvidenceMetadata, ReportingReadQuery, ReportingReasonCode, ReportingValidationIssue,
    RiskEventReportingRow, TradeReportingRow, sort_performance_reporting_rows,
    sort_position_reporting_rows, sort_risk_event_reporting_rows, sort_trade_reporting_rows,
    validate_performance_reporting_row, validate_position_reporting_row,
    validate_reporting_read_query, validate_risk_event_reporting_row, validate_trade_reporting_row,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const LOAD_TRADE_REPORTING_ROWS_SQL: &str = r#"
    SELECT
        report_row_id,
        order_id,
        trade_id,
        market_id,
        asset_id,
        lifecycle_state,
        event_status,
        to_char(occurred_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS occurred_at_utc,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        source,
        reason_code,
        correlation_id,
        run_id,
        snapshot_id
    FROM reporting_trade_read_models
    WHERE occurred_at_utc >= $1::timestamptz
      AND occurred_at_utc < $2::timestamptz
      AND ($3::text IS NULL OR market_id = $3)
      AND ($4::text IS NULL OR correlation_id = $4)
    ORDER BY occurred_at_utc DESC, market_id ASC, order_id ASC, trade_id ASC, report_row_id ASC
    LIMIT $5
"#;

const LOAD_POSITION_REPORTING_ROWS_SQL: &str = r#"
    SELECT
        report_row_id,
        market_id,
        net_exposure,
        gross_exposure,
        open_order_count,
        run_status,
        to_char(window_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_started_at_utc,
        to_char(window_ended_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_ended_at_utc,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        source,
        reason_code,
        correlation_id,
        run_id,
        snapshot_id
    FROM reporting_position_read_models
    WHERE as_of_utc >= $1::timestamptz
      AND as_of_utc < $2::timestamptz
      AND ($3::text IS NULL OR market_id = $3)
      AND ($4::text IS NULL OR correlation_id = $4)
    ORDER BY as_of_utc DESC, market_id ASC, run_id ASC, report_row_id ASC
    LIMIT $5
"#;

const LOAD_RISK_EVENT_REPORTING_ROWS_SQL: &str = r#"
    SELECT
        report_row_id,
        event_type,
        severity,
        outcome,
        market_id,
        to_char(event_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS event_at_utc,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        source,
        reason_code,
        correlation_id,
        run_id,
        snapshot_id
    FROM reporting_risk_event_read_models
    WHERE event_at_utc >= $1::timestamptz
      AND event_at_utc < $2::timestamptz
      AND ($3::text IS NULL OR market_id = $3)
      AND ($4::text IS NULL OR correlation_id = $4)
    ORDER BY event_at_utc DESC, event_type ASC, report_row_id ASC
    LIMIT $5
"#;

const LOAD_PERFORMANCE_REPORTING_ROWS_SQL: &str = r#"
    SELECT
        report_row_id,
        market_id,
        alpha_id,
        period_scope,
        to_char(period_start_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS period_start_utc,
        to_char(period_end_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS period_end_utc,
        realized_pnl_usd,
        unrealized_pnl_usd,
        gross_pnl_usd,
        net_pnl_usd,
        fees_usd,
        rebates_usd,
        incentives_usd,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        source,
        reason_code,
        correlation_id,
        run_id,
        snapshot_id
    FROM reporting_performance_read_models
    WHERE as_of_utc >= $1::timestamptz
      AND as_of_utc < $2::timestamptz
      AND ($3::text IS NULL OR market_id = $3)
      AND ($4::text IS NULL OR alpha_id = $4)
      AND ($5::text IS NULL OR correlation_id = $5)
    ORDER BY period_end_utc DESC, market_id ASC, alpha_id ASC, report_row_id ASC
    LIMIT $6
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingReadModelPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingValidationIssue>,
}

impl ReportingReadModelPersistenceError {
    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ReportingReadModelPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReportingReadModelPersistenceError {}

pub async fn load_trade_reporting_rows(
    pool: &PgPool,
    query: &ReportingReadQuery,
) -> Result<Vec<TradeReportingRow>, ReportingReadModelPersistenceError> {
    validate_reporting_query_or_error(query)?;

    let rows = sqlx::query(LOAD_TRADE_REPORTING_ROWS_SQL)
        .bind(&query.start_inclusive_utc)
        .bind(&query.end_exclusive_utc)
        .bind(query.market_id.as_deref())
        .bind(query.correlation_id.as_deref())
        .bind(query.limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_trade_reporting_rows", error))?;
    let mut decoded = rows
        .into_iter()
        .map(decode_trade_row)
        .collect::<Result<Vec<_>, _>>()?;
    sort_trade_reporting_rows(&mut decoded);
    Ok(decoded)
}

pub async fn load_position_reporting_rows(
    pool: &PgPool,
    query: &ReportingReadQuery,
) -> Result<Vec<PositionReportingRow>, ReportingReadModelPersistenceError> {
    validate_reporting_query_or_error(query)?;

    let rows = sqlx::query(LOAD_POSITION_REPORTING_ROWS_SQL)
        .bind(&query.start_inclusive_utc)
        .bind(&query.end_exclusive_utc)
        .bind(query.market_id.as_deref())
        .bind(query.correlation_id.as_deref())
        .bind(query.limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_position_reporting_rows", error))?;
    let mut decoded = rows
        .into_iter()
        .map(decode_position_row)
        .collect::<Result<Vec<_>, _>>()?;
    sort_position_reporting_rows(&mut decoded);
    Ok(decoded)
}

pub async fn load_risk_event_reporting_rows(
    pool: &PgPool,
    query: &ReportingReadQuery,
) -> Result<Vec<RiskEventReportingRow>, ReportingReadModelPersistenceError> {
    validate_reporting_query_or_error(query)?;

    let rows = sqlx::query(LOAD_RISK_EVENT_REPORTING_ROWS_SQL)
        .bind(&query.start_inclusive_utc)
        .bind(&query.end_exclusive_utc)
        .bind(query.market_id.as_deref())
        .bind(query.correlation_id.as_deref())
        .bind(query.limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_risk_event_reporting_rows", error))?;
    let mut decoded = rows
        .into_iter()
        .map(decode_risk_event_row)
        .collect::<Result<Vec<_>, _>>()?;
    sort_risk_event_reporting_rows(&mut decoded);
    Ok(decoded)
}

pub async fn load_performance_reporting_rows(
    pool: &PgPool,
    query: &ReportingReadQuery,
) -> Result<Vec<PerformanceReportingRow>, ReportingReadModelPersistenceError> {
    validate_reporting_query_or_error(query)?;

    let rows = sqlx::query(LOAD_PERFORMANCE_REPORTING_ROWS_SQL)
        .bind(&query.start_inclusive_utc)
        .bind(&query.end_exclusive_utc)
        .bind(query.market_id.as_deref())
        .bind(query.alpha_id.as_deref())
        .bind(query.correlation_id.as_deref())
        .bind(query.limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_performance_reporting_rows", error))?;
    let mut decoded = rows
        .into_iter()
        .map(decode_performance_row)
        .collect::<Result<Vec<_>, _>>()?;
    sort_performance_reporting_rows(&mut decoded);
    Ok(decoded)
}

fn decode_trade_row(
    row: sqlx::postgres::PgRow,
) -> Result<TradeReportingRow, ReportingReadModelPersistenceError> {
    let decoded = TradeReportingRow {
        report_row_id: row.try_get("report_row_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("report_row_id", error)
        })?,
        order_id: row.try_get("order_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("order_id", error)
        })?,
        trade_id: row.try_get("trade_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("trade_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("market_id", error)
        })?,
        asset_id: row.try_get("asset_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("asset_id", error)
        })?,
        lifecycle_state: row.try_get("lifecycle_state").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("lifecycle_state", error)
        })?,
        event_status: row.try_get("event_status").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("event_status", error)
        })?,
        occurred_at_utc: row.try_get("occurred_at_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("occurred_at_utc", error)
        })?,
        evidence: decode_evidence(&row)?,
    };
    validate_trade_reporting_row(&decoded).map_err(map_contract_error)?;
    Ok(decoded)
}

fn decode_position_row(
    row: sqlx::postgres::PgRow,
) -> Result<PositionReportingRow, ReportingReadModelPersistenceError> {
    let decoded = PositionReportingRow {
        report_row_id: row.try_get("report_row_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("report_row_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("market_id", error)
        })?,
        net_exposure: row.try_get("net_exposure").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("net_exposure", error)
        })?,
        gross_exposure: row.try_get("gross_exposure").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("gross_exposure", error)
        })?,
        open_order_count: row.try_get("open_order_count").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("open_order_count", error)
        })?,
        run_status: row.try_get("run_status").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("run_status", error)
        })?,
        window_started_at_utc: row.try_get("window_started_at_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("window_started_at_utc", error)
        })?,
        window_ended_at_utc: row.try_get("window_ended_at_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("window_ended_at_utc", error)
        })?,
        evidence: decode_evidence(&row)?,
    };
    validate_position_reporting_row(&decoded).map_err(map_contract_error)?;
    Ok(decoded)
}

fn decode_risk_event_row(
    row: sqlx::postgres::PgRow,
) -> Result<RiskEventReportingRow, ReportingReadModelPersistenceError> {
    let decoded = RiskEventReportingRow {
        report_row_id: row.try_get("report_row_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("report_row_id", error)
        })?,
        event_type: row.try_get("event_type").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("event_type", error)
        })?,
        severity: row.try_get("severity").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("severity", error)
        })?,
        outcome: row.try_get("outcome").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("outcome", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("market_id", error)
        })?,
        event_at_utc: row.try_get("event_at_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("event_at_utc", error)
        })?,
        evidence: decode_evidence(&row)?,
    };
    validate_risk_event_reporting_row(&decoded).map_err(map_contract_error)?;
    Ok(decoded)
}

fn decode_performance_row(
    row: sqlx::postgres::PgRow,
) -> Result<PerformanceReportingRow, ReportingReadModelPersistenceError> {
    let decoded = PerformanceReportingRow {
        report_row_id: row.try_get("report_row_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("report_row_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("market_id", error)
        })?,
        alpha_id: row.try_get("alpha_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("alpha_id", error)
        })?,
        period_scope: row.try_get("period_scope").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("period_scope", error)
        })?,
        period_start_utc: row.try_get("period_start_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("period_start_utc", error)
        })?,
        period_end_utc: row.try_get("period_end_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("period_end_utc", error)
        })?,
        realized_pnl_usd: row.try_get("realized_pnl_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("realized_pnl_usd", error)
        })?,
        unrealized_pnl_usd: row.try_get("unrealized_pnl_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("unrealized_pnl_usd", error)
        })?,
        gross_pnl_usd: row.try_get("gross_pnl_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("gross_pnl_usd", error)
        })?,
        net_pnl_usd: row.try_get("net_pnl_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("net_pnl_usd", error)
        })?,
        fees_usd: row.try_get("fees_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("fees_usd", error)
        })?,
        rebates_usd: row.try_get("rebates_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("rebates_usd", error)
        })?,
        incentives_usd: row.try_get("incentives_usd").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("incentives_usd", error)
        })?,
        evidence: decode_evidence(&row)?,
    };
    validate_performance_reporting_row(&decoded).map_err(map_contract_error)?;
    Ok(decoded)
}

fn decode_evidence(
    row: &sqlx::postgres::PgRow,
) -> Result<ReportingEvidenceMetadata, ReportingReadModelPersistenceError> {
    Ok(ReportingEvidenceMetadata {
        as_of_utc: row.try_get("as_of_utc").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("as_of_utc", error)
        })?,
        source: row.try_get("source").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("source", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        run_id: row.try_get("run_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("run_id", error)
        })?,
        snapshot_id: row.try_get("snapshot_id").map_err(|error| {
            ReportingReadModelPersistenceError::row_decode_failure("snapshot_id", error)
        })?,
    })
}

fn validate_reporting_query_or_error(
    query: &ReportingReadQuery,
) -> Result<(), ReportingReadModelPersistenceError> {
    validate_reporting_read_query(query).map_err(map_contract_error)
}

fn map_contract_error(error: ReportingContractError) -> ReportingReadModelPersistenceError {
    ReportingReadModelPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ReportingReadModelPersistenceError {
    if is_constraint_error(&error) {
        return ReportingReadModelPersistenceError::constraint_violation(operation, error);
    }
    ReportingReadModelPersistenceError::query_failure(operation, error)
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
    use domain::reporting::{
        DEFAULT_REPORTING_LIMIT, ReportingEvidenceMetadata, build_reporting_read_query,
    };

    const REPORTING_READ_MODELS_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407022400_reporting_read_models.sql");

    fn sample_query() -> ReportingReadQuery {
        build_reporting_read_query(
            "2026-04-07T01:00:00Z",
            "2026-04-07T02:00:00Z",
            Some("market-btc-election"),
            Some("alpha-momentum"),
            Some("corr-reporting-001"),
            DEFAULT_REPORTING_LIMIT,
        )
        .expect("query should build")
    }

    fn sample_evidence() -> ReportingEvidenceMetadata {
        ReportingEvidenceMetadata {
            as_of_utc: "2026-04-07T02:00:00Z".to_string(),
            source: "reporting.read-models.v1".to_string(),
            reason_code: "reporting_ready".to_string(),
            correlation_id: "corr-reporting-001".to_string(),
            run_id: Some("run-reporting-001".to_string()),
            snapshot_id: Some("snapshot-reporting-001".to_string()),
        }
    }

    #[test]
    fn migration_scope_remains_limited_to_story_4_1_read_models() {
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("CREATE OR REPLACE VIEW reporting_trade_read_models")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("CREATE OR REPLACE VIEW reporting_position_read_models")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("CREATE OR REPLACE VIEW reporting_risk_event_read_models")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("CREATE OR REPLACE VIEW reporting_performance_read_models")
        );
        assert!(!REPORTING_READ_MODELS_MIGRATION_SQL.contains("api_contract_versions"));
        assert!(!REPORTING_READ_MODELS_MIGRATION_SQL.contains("report_schedules"));
        assert!(!REPORTING_READ_MODELS_MIGRATION_SQL.contains("report_runs"));
        assert!(!REPORTING_READ_MODELS_MIGRATION_SQL.contains("export_jobs"));
        assert!(!REPORTING_READ_MODELS_MIGRATION_SQL.contains("export_artifacts"));
    }

    #[test]
    fn migration_includes_query_supporting_indexes_and_correlation_lookups() {
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("idx_user_stream_events_reporting_trade_window")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("idx_exposure_snapshots_reporting_position_window")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("idx_attribution_snapshots_reporting_performance_window")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("idx_pretrade_gate_decisions_reporting_risk_window")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("idx_incident_query_views_reporting_risk_window")
        );
    }

    #[test]
    fn incident_risk_event_projection_normalizes_optional_identifiers() {
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("NULLIF(lower(trim(incident.market_id)), '') AS market_id")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("NULLIF(lower(trim(incident.run_id)), '') AS run_id")
        );
        assert!(
            REPORTING_READ_MODELS_MIGRATION_SQL
                .contains("NULLIF(lower(trim(incident.snapshot_id)), '') AS snapshot_id")
        );
    }

    #[test]
    fn adapter_queries_enforce_deterministic_boundaries_and_ordering() {
        assert!(LOAD_TRADE_REPORTING_ROWS_SQL.contains("occurred_at_utc >= $1::timestamptz"));
        assert!(LOAD_TRADE_REPORTING_ROWS_SQL.contains("occurred_at_utc < $2::timestamptz"));
        assert!(LOAD_TRADE_REPORTING_ROWS_SQL
            .contains("ORDER BY occurred_at_utc DESC, market_id ASC, order_id ASC, trade_id ASC, report_row_id ASC"));

        assert!(LOAD_POSITION_REPORTING_ROWS_SQL.contains("as_of_utc >= $1::timestamptz"));
        assert!(LOAD_POSITION_REPORTING_ROWS_SQL.contains("as_of_utc < $2::timestamptz"));
        assert!(
            LOAD_POSITION_REPORTING_ROWS_SQL
                .contains("ORDER BY as_of_utc DESC, market_id ASC, run_id ASC, report_row_id ASC")
        );

        assert!(LOAD_RISK_EVENT_REPORTING_ROWS_SQL.contains("event_at_utc >= $1::timestamptz"));
        assert!(LOAD_RISK_EVENT_REPORTING_ROWS_SQL.contains("event_at_utc < $2::timestamptz"));
        assert!(
            LOAD_RISK_EVENT_REPORTING_ROWS_SQL
                .contains("ORDER BY event_at_utc DESC, event_type ASC, report_row_id ASC")
        );

        assert!(LOAD_PERFORMANCE_REPORTING_ROWS_SQL.contains("as_of_utc >= $1::timestamptz"));
        assert!(LOAD_PERFORMANCE_REPORTING_ROWS_SQL.contains("as_of_utc < $2::timestamptz"));
        assert!(LOAD_PERFORMANCE_REPORTING_ROWS_SQL.contains(
            "ORDER BY period_end_utc DESC, market_id ASC, alpha_id ASC, report_row_id ASC"
        ));
    }

    #[test]
    fn query_validation_rejects_out_of_bounds_limits() {
        let mut query = sample_query();
        query.limit = 0;
        let error = validate_reporting_query_or_error(&query)
            .expect_err("limit=0 should fail query validation");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );
    }

    #[test]
    fn query_validation_rejects_non_utc_offsets_and_malformed_identifiers() {
        let mut query = sample_query();
        query.start_inclusive_utc = "2026-04-07T01:00:00+01:00".to_string();
        query.market_id = Some("market/btc-election".to_string());
        query.correlation_id = Some("corr/reporting-001".to_string());

        let error = validate_reporting_query_or_error(&query).expect_err(
            "non-UTC timestamps and malformed identifiers should fail query validation",
        );
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "start_inclusive_utc")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "correlation_id")
        );
    }

    #[test]
    fn row_validation_accepts_canonical_payloads_for_all_datasets() {
        let trade = TradeReportingRow {
            report_row_id: "trade-row-001".to_string(),
            order_id: "order-001".to_string(),
            trade_id: "trade-001".to_string(),
            market_id: "market-btc-election".to_string(),
            asset_id: "asset-btc".to_string(),
            lifecycle_state: "filled".to_string(),
            event_status: "matched".to_string(),
            occurred_at_utc: "2026-04-07T01:30:00Z".to_string(),
            evidence: sample_evidence(),
        };
        assert!(validate_trade_reporting_row(&trade).is_ok());

        let position = PositionReportingRow {
            report_row_id: "position-row-001".to_string(),
            market_id: "market-btc-election".to_string(),
            net_exposure: 10.0,
            gross_exposure: 20.0,
            open_order_count: 1,
            run_status: "succeeded".to_string(),
            window_started_at_utc: "2026-04-07T01:00:00Z".to_string(),
            window_ended_at_utc: "2026-04-07T01:30:00Z".to_string(),
            evidence: sample_evidence(),
        };
        assert!(validate_position_reporting_row(&position).is_ok());

        let risk = RiskEventReportingRow {
            report_row_id: "risk-row-001".to_string(),
            event_type: "pretrade_gate".to_string(),
            severity: "critical".to_string(),
            outcome: "deny".to_string(),
            market_id: Some("market-btc-election".to_string()),
            event_at_utc: "2026-04-07T01:45:00Z".to_string(),
            evidence: sample_evidence(),
        };
        assert!(validate_risk_event_reporting_row(&risk).is_ok());

        let performance = PerformanceReportingRow {
            report_row_id: "performance-row-001".to_string(),
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period_scope: "24h".to_string(),
            period_start_utc: "2026-04-06T02:24:01Z".to_string(),
            period_end_utc: "2026-04-07T02:24:01Z".to_string(),
            realized_pnl_usd: 100.0,
            unrealized_pnl_usd: 20.0,
            gross_pnl_usd: 120.0,
            net_pnl_usd: 117.0,
            fees_usd: 6.0,
            rebates_usd: 1.0,
            incentives_usd: 2.0,
            evidence: sample_evidence(),
        };
        assert!(validate_performance_reporting_row(&performance).is_ok());
    }
}
