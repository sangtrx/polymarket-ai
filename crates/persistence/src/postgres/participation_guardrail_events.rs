use domain::risk::{
    ParticipationGuardrailContractError, ParticipationGuardrailReasonCode,
    ParticipationGuardrailValidationIssue, PreTradeParticipationGuardrailEvidence,
    normalize_pretrade_identifier, validate_pretrade_participation_guardrail_evidence,
};
use sqlx::{PgExecutor, PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const INSERT_PARTICIPATION_GUARDRAIL_EVENT_SQL: &str = r#"
    INSERT INTO participation_guardrail_events (
        event_id,
        guardrail_mode,
        reason_code,
        market_id,
        cluster_id,
        correlation_id,
        observed_at_utc,
        evaluated_at_utc,
        liquidity_depth_usd,
        inactivity_gap_seconds,
        threshold_liquidity_depth_usd,
        threshold_inactivity_pause_seconds,
        threshold_overnight_gap_seconds,
        normal_max_order_size_units,
        capped_max_order_size_units
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz, $9, $10, $11, $12, $13, $14, $15
    )
"#;

const LOAD_PARTICIPATION_GUARDRAIL_EVENTS_SQL: &str = r#"
    SELECT
        event_id,
        guardrail_mode,
        reason_code,
        market_id,
        cluster_id,
        correlation_id,
        to_char(observed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS observed_at_utc,
        to_char(evaluated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS evaluated_at_utc,
        liquidity_depth_usd,
        inactivity_gap_seconds,
        threshold_liquidity_depth_usd,
        threshold_inactivity_pause_seconds,
        threshold_overnight_gap_seconds,
        normal_max_order_size_units,
        capped_max_order_size_units
    FROM participation_guardrail_events
    WHERE ($1::text IS NULL OR market_id = lower(trim($1)))
      AND ($2::text IS NULL OR reason_code = $2)
      AND ($3::text IS NULL OR correlation_id = lower(trim($3)))
      AND ($4::timestamptz IS NULL OR observed_at_utc >= $4::timestamptz)
      AND ($5::timestamptz IS NULL OR observed_at_utc < $5::timestamptz)
    ORDER BY observed_at_utc DESC, event_id ASC
    LIMIT $6
"#;

const PARTICIPATION_GUARDRAIL_QUERY_FAILED: &str = "participation_guardrail_query_failed";
const PARTICIPATION_GUARDRAIL_CONSTRAINT_VIOLATION: &str =
    "participation_guardrail_constraint_violation";
const PARTICIPATION_GUARDRAIL_ROW_DECODE_FAILED: &str = "participation_guardrail_row_decode_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipationGuardrailPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ParticipationGuardrailValidationIssue>,
}

impl ParticipationGuardrailPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ParticipationGuardrailValidationIssue>,
    ) -> Self {
        Self {
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failed(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: PARTICIPATION_GUARDRAIL_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: PARTICIPATION_GUARDRAIL_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: PARTICIPATION_GUARDRAIL_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ParticipationGuardrailPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ParticipationGuardrailPersistenceError {}

pub async fn create_participation_guardrail_event<'e, E>(
    executor: E,
    event: &PreTradeParticipationGuardrailEvidence,
) -> Result<(), ParticipationGuardrailPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_pretrade_participation_guardrail_evidence(event).map_err(map_contract_error)?;
    let result = sqlx::query(INSERT_PARTICIPATION_GUARDRAIL_EVENT_SQL)
        .bind(&event.event_id)
        .bind(&event.guardrail_mode)
        .bind(&event.reason_code)
        .bind(normalize_pretrade_identifier(&event.market_id))
        .bind(normalize_pretrade_identifier(&event.cluster_id))
        .bind(normalize_pretrade_identifier(&event.correlation_id))
        .bind(&event.observed_at_utc)
        .bind(&event.evaluated_at_utc)
        .bind(event.liquidity_depth_usd)
        .bind(event.inactivity_gap_seconds)
        .bind(event.threshold_liquidity_depth_usd)
        .bind(event.threshold_inactivity_pause_seconds)
        .bind(event.threshold_overnight_gap_seconds)
        .bind(event.normal_max_order_size_units)
        .bind(event.capped_max_order_size_units)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("create_participation_guardrail_event", error))?;

    if result.rows_affected() != 1 {
        return Err(ParticipationGuardrailPersistenceError::invalid_payload(
            format!(
                "create_participation_guardrail_event expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn load_participation_guardrail_events(
    pool: &PgPool,
    market_id: Option<&str>,
    reason_code: Option<&str>,
    correlation_id: Option<&str>,
    start_ts: Option<&str>,
    end_ts: Option<&str>,
    limit: i64,
) -> Result<Vec<PreTradeParticipationGuardrailEvidence>, ParticipationGuardrailPersistenceError> {
    let limit = validate_query_limit(limit)?;
    let market_id = normalize_optional_identifier("market_id", market_id)?;
    let correlation_id = normalize_optional_identifier("correlation_id", correlation_id)?;
    let reason_code = normalize_optional_reason_code(reason_code)?;
    let start_ts = normalize_optional_timestamp("start_ts", start_ts)?;
    let end_ts = normalize_optional_timestamp("end_ts", end_ts)?;
    if let (Some(start_ts), Some(end_ts)) = (start_ts.as_ref(), end_ts.as_ref()) {
        let start = parse_utc_timestamp(start_ts).map_err(|_| {
            ParticipationGuardrailPersistenceError::invalid_payload(
                "start_ts must be an RFC3339 UTC timestamp".to_string(),
                vec![ParticipationGuardrailValidationIssue {
                    field: "start_ts",
                    code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                    message: "start_ts must be an RFC3339 UTC timestamp".to_string(),
                }],
            )
        })?;
        let end = parse_utc_timestamp(end_ts).map_err(|_| {
            ParticipationGuardrailPersistenceError::invalid_payload(
                "end_ts must be an RFC3339 UTC timestamp".to_string(),
                vec![ParticipationGuardrailValidationIssue {
                    field: "end_ts",
                    code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                    message: "end_ts must be an RFC3339 UTC timestamp".to_string(),
                }],
            )
        })?;
        if end <= start {
            return Err(ParticipationGuardrailPersistenceError::invalid_payload(
                "end_ts must be greater than start_ts".to_string(),
                vec![ParticipationGuardrailValidationIssue {
                    field: "end_ts",
                    code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                    message: "end_ts must be greater than start_ts".to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LOAD_PARTICIPATION_GUARDRAIL_EVENTS_SQL)
        .bind(market_id.as_deref())
        .bind(reason_code.as_deref())
        .bind(correlation_id.as_deref())
        .bind(start_ts.as_deref())
        .bind(end_ts.as_deref())
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_participation_guardrail_events", error))?;

    rows.into_iter()
        .map(decode_participation_guardrail_row)
        .collect()
}

fn decode_participation_guardrail_row(
    row: sqlx::postgres::PgRow,
) -> Result<PreTradeParticipationGuardrailEvidence, ParticipationGuardrailPersistenceError> {
    let event = PreTradeParticipationGuardrailEvidence {
        event_id: row.try_get("event_id").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("event_id", error)
        })?,
        guardrail_mode: row.try_get("guardrail_mode").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("guardrail_mode", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("reason_code", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("market_id", error)
        })?,
        cluster_id: row.try_get("cluster_id").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("cluster_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        observed_at_utc: row.try_get("observed_at_utc").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("observed_at_utc", error)
        })?,
        evaluated_at_utc: row.try_get("evaluated_at_utc").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("evaluated_at_utc", error)
        })?,
        liquidity_depth_usd: row.try_get("liquidity_depth_usd").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure("liquidity_depth_usd", error)
        })?,
        inactivity_gap_seconds: row.try_get("inactivity_gap_seconds").map_err(|error| {
            ParticipationGuardrailPersistenceError::row_decode_failure(
                "inactivity_gap_seconds",
                error,
            )
        })?,
        threshold_liquidity_depth_usd: row.try_get("threshold_liquidity_depth_usd").map_err(
            |error| {
                ParticipationGuardrailPersistenceError::row_decode_failure(
                    "threshold_liquidity_depth_usd",
                    error,
                )
            },
        )?,
        threshold_inactivity_pause_seconds: row
            .try_get("threshold_inactivity_pause_seconds")
            .map_err(|error| {
                ParticipationGuardrailPersistenceError::row_decode_failure(
                    "threshold_inactivity_pause_seconds",
                    error,
                )
            })?,
        threshold_overnight_gap_seconds: row.try_get("threshold_overnight_gap_seconds").map_err(
            |error| {
                ParticipationGuardrailPersistenceError::row_decode_failure(
                    "threshold_overnight_gap_seconds",
                    error,
                )
            },
        )?,
        normal_max_order_size_units: row.try_get("normal_max_order_size_units").map_err(
            |error| {
                ParticipationGuardrailPersistenceError::row_decode_failure(
                    "normal_max_order_size_units",
                    error,
                )
            },
        )?,
        capped_max_order_size_units: row.try_get("capped_max_order_size_units").map_err(
            |error| {
                ParticipationGuardrailPersistenceError::row_decode_failure(
                    "capped_max_order_size_units",
                    error,
                )
            },
        )?,
    };
    validate_pretrade_participation_guardrail_evidence(&event).map_err(map_contract_error)?;
    Ok(event)
}

fn validate_query_limit(limit: i64) -> Result<i64, ParticipationGuardrailPersistenceError> {
    if (1..=200).contains(&limit) {
        return Ok(limit);
    }
    Err(ParticipationGuardrailPersistenceError::invalid_payload(
        "limit must be between 1 and 200".to_string(),
        vec![ParticipationGuardrailValidationIssue {
            field: "limit",
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: "limit must be between 1 and 200".to_string(),
        }],
    ))
}

fn normalize_optional_identifier(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, ParticipationGuardrailPersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = normalize_pretrade_identifier(value);
    if !is_canonical_identifier(&normalized) {
        return Err(ParticipationGuardrailPersistenceError::invalid_payload(
            format!("{field} must contain 3-120 canonical characters"),
            vec![ParticipationGuardrailValidationIssue {
                field,
                code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-120 canonical characters"),
            }],
        ));
    }
    Ok(Some(normalized))
}

fn normalize_optional_reason_code(
    reason_code: Option<&str>,
) -> Result<Option<String>, ParticipationGuardrailPersistenceError> {
    let Some(reason_code) = reason_code.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    ParticipationGuardrailReasonCode::parse(reason_code).map_err(map_contract_error)?;
    Ok(Some(reason_code.to_string()))
}

fn normalize_optional_timestamp(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, ParticipationGuardrailPersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    parse_utc_timestamp(value).map_err(|_| {
        ParticipationGuardrailPersistenceError::invalid_payload(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![ParticipationGuardrailValidationIssue {
                field,
                code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    Ok(Some(value.to_string()))
}

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

fn is_canonical_identifier(value: &str) -> bool {
    let length = value.len();
    if !(3..=120).contains(&length) {
        return false;
    }
    value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
    })
}

fn map_contract_error(
    error: ParticipationGuardrailContractError,
) -> ParticipationGuardrailPersistenceError {
    ParticipationGuardrailPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ParticipationGuardrailPersistenceError {
    if is_constraint_error(&error) {
        return ParticipationGuardrailPersistenceError::constraint_violation(operation, error);
    }
    ParticipationGuardrailPersistenceError::query_failed(operation, error)
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

    const PARTICIPATION_GUARDRAIL_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407113000_participation_guardrail_events.sql");

    fn sample_evidence() -> PreTradeParticipationGuardrailEvidence {
        PreTradeParticipationGuardrailEvidence {
            event_id: "fr41::event-0001".to_string(),
            guardrail_mode: "size_cap".to_string(),
            reason_code: ParticipationGuardrailReasonCode::OvernightSizeCapActive
                .code()
                .to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            correlation_id: "corr-fr41-001".to_string(),
            observed_at_utc: "2026-04-07T10:00:00Z".to_string(),
            evaluated_at_utc: "2026-04-07T10:00:00Z".to_string(),
            liquidity_depth_usd: 12_500.0,
            inactivity_gap_seconds: 15_000.0,
            threshold_liquidity_depth_usd: 10_000.0,
            threshold_inactivity_pause_seconds: 900.0,
            threshold_overnight_gap_seconds: 14_400.0,
            normal_max_order_size_units: Some(40.0),
            capped_max_order_size_units: Some(10.0),
        }
    }

    #[test]
    fn migration_creates_expected_participation_guardrail_schema_scope() {
        assert!(
            PARTICIPATION_GUARDRAIL_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS participation_guardrail_events")
        );
        assert!(!PARTICIPATION_GUARDRAIL_MIGRATION_SQL.contains("risk_limit_profiles"));
        assert!(!PARTICIPATION_GUARDRAIL_MIGRATION_SQL.contains("pretrade_gate_decisions"));
        assert!(!PARTICIPATION_GUARDRAIL_MIGRATION_SQL.contains("regime_shift_alerts"));
    }

    #[test]
    fn migration_enforces_constraints_and_traceability_indexes() {
        assert!(
            PARTICIPATION_GUARDRAIL_MIGRATION_SQL
                .contains("guardrail_mode IN ('pass', 'pause', 'size_cap', 'unavailable')")
        );
        assert!(
            PARTICIPATION_GUARDRAIL_MIGRATION_SQL
                .contains("idx_participation_guardrail_events_market_observed_at")
        );
        assert!(
            PARTICIPATION_GUARDRAIL_MIGRATION_SQL
                .contains("idx_participation_guardrail_events_reason_observed_at")
        );
        assert!(
            PARTICIPATION_GUARDRAIL_MIGRATION_SQL
                .contains("idx_participation_guardrail_events_correlation_observed_at")
        );
    }

    #[test]
    fn persistence_validation_accepts_canonical_payload() {
        assert!(validate_pretrade_participation_guardrail_evidence(&sample_evidence()).is_ok());
    }

    #[test]
    fn query_limit_rejects_out_of_range_values() {
        let error = validate_query_limit(0).expect_err("lower-bound query limit should fail");
        assert_eq!(
            error.code,
            ParticipationGuardrailReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );
    }

    #[test]
    fn identifier_filters_reject_non_canonical_values() {
        let error = normalize_optional_identifier("market_id", Some("bad id!"))
            .expect_err("non-canonical query filter should fail");
        assert_eq!(
            error.code,
            ParticipationGuardrailReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
    }

    #[test]
    fn reason_code_filters_require_supported_fr41_reason_codes() {
        let error = normalize_optional_reason_code(Some("fr41_participation_unknown"))
            .expect_err("unknown reason_code filter should fail");
        assert_eq!(
            error.code,
            ParticipationGuardrailReasonCode::InvalidPayload.code()
        );
    }

    #[test]
    fn timestamp_filters_require_rfc3339_utc_values() {
        let error = normalize_optional_timestamp("start_ts", Some("2026-04-07T10:00:00+01:00"))
            .expect_err("non-UTC offset should fail timestamp filter validation");
        assert_eq!(
            error.code,
            ParticipationGuardrailReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "start_ts")
        );
    }

    #[test]
    fn query_sql_preserves_deterministic_sorting_contract() {
        assert!(
            LOAD_PARTICIPATION_GUARDRAIL_EVENTS_SQL
                .contains("ORDER BY observed_at_utc DESC, event_id ASC")
        );
    }
}
