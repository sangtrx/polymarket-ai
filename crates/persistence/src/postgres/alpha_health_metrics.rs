use domain::research::{
    AlphaHealthAttributionWindowMetrics, AlphaHealthContractError, AlphaHealthMetricKey,
    AlphaHealthMetricRecord, AlphaHealthReasonCode, AlphaHealthValidationIssue,
    AlphaThresholdBreachRecord, ValidationGateComparator, canonicalize_alpha_health_metric_record,
    canonicalize_alpha_threshold_breach_record, normalize_research_identifier,
    parse_alpha_health_utc_timestamp,
};
use serde_json::{Value, json};
use sqlx::{PgExecutor, PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_ALPHA_HEALTH_METRIC_SQL: &str = r#"
    INSERT INTO alpha_health_metrics (
        metric_id,
        alpha_id,
        rolling_sharpe,
        rolling_hit_rate,
        rolling_drawdown,
        stability_score,
        windows_json,
        reason_code,
        actor_id,
        correlation_id,
        recorded_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, NOW()
    )
    ON CONFLICT (metric_id)
    DO UPDATE SET
        alpha_id = EXCLUDED.alpha_id,
        rolling_sharpe = EXCLUDED.rolling_sharpe,
        rolling_hit_rate = EXCLUDED.rolling_hit_rate,
        rolling_drawdown = EXCLUDED.rolling_drawdown,
        stability_score = EXCLUDED.stability_score,
        windows_json = EXCLUDED.windows_json,
        reason_code = EXCLUDED.reason_code,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        recorded_at_utc = EXCLUDED.recorded_at_utc,
        updated_at_utc = NOW()
"#;

const LOAD_ALPHA_HEALTH_METRIC_SQL: &str = r#"
    SELECT
        metric_id,
        alpha_id,
        rolling_sharpe,
        rolling_hit_rate,
        rolling_drawdown,
        stability_score,
        windows_json,
        reason_code,
        actor_id,
        correlation_id,
        to_char(recorded_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS recorded_at_utc
    FROM alpha_health_metrics
    WHERE lower(trim(metric_id)) = lower(trim($1))
    ORDER BY recorded_at_utc DESC, metric_id ASC
    LIMIT 1
"#;

const LIST_ALPHA_HEALTH_METRICS_BY_ALPHA_SQL: &str = r#"
    SELECT
        metric_id,
        alpha_id,
        rolling_sharpe,
        rolling_hit_rate,
        rolling_drawdown,
        stability_score,
        windows_json,
        reason_code,
        actor_id,
        correlation_id,
        to_char(recorded_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS recorded_at_utc
    FROM alpha_health_metrics
    WHERE lower(trim(alpha_id)) = lower(trim($1))
      AND ($2::timestamptz IS NULL OR recorded_at_utc >= $2::timestamptz)
      AND ($3::timestamptz IS NULL OR recorded_at_utc < $3::timestamptz)
    ORDER BY recorded_at_utc DESC, metric_id ASC
    LIMIT $4
"#;

const UPSERT_ALPHA_THRESHOLD_BREACH_SQL: &str = r#"
    INSERT INTO alpha_threshold_breaches (
        breach_id,
        metric_id,
        alpha_id,
        metric_key,
        comparator,
        observed_value,
        threshold_value,
        breach_reason,
        reason_code,
        actor_id,
        correlation_id,
        breached_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::timestamptz, NOW()
    )
    ON CONFLICT (breach_id)
    DO UPDATE SET
        metric_id = EXCLUDED.metric_id,
        alpha_id = EXCLUDED.alpha_id,
        metric_key = EXCLUDED.metric_key,
        comparator = EXCLUDED.comparator,
        observed_value = EXCLUDED.observed_value,
        threshold_value = EXCLUDED.threshold_value,
        breach_reason = EXCLUDED.breach_reason,
        reason_code = EXCLUDED.reason_code,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        breached_at_utc = EXCLUDED.breached_at_utc,
        updated_at_utc = NOW()
"#;

const LOAD_ALPHA_THRESHOLD_BREACH_SQL: &str = r#"
    SELECT
        breach_id,
        metric_id,
        alpha_id,
        metric_key,
        comparator,
        observed_value,
        threshold_value,
        breach_reason,
        reason_code,
        actor_id,
        correlation_id,
        to_char(breached_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS breached_at_utc
    FROM alpha_threshold_breaches
    WHERE lower(trim(breach_id)) = lower(trim($1))
    ORDER BY breached_at_utc DESC, breach_id ASC
    LIMIT 1
"#;

const LIST_ALPHA_THRESHOLD_BREACHES_BY_ALPHA_SQL: &str = r#"
    SELECT
        breach_id,
        metric_id,
        alpha_id,
        metric_key,
        comparator,
        observed_value,
        threshold_value,
        breach_reason,
        reason_code,
        actor_id,
        correlation_id,
        to_char(breached_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS breached_at_utc
    FROM alpha_threshold_breaches
    WHERE lower(trim(alpha_id)) = lower(trim($1))
      AND ($2::timestamptz IS NULL OR breached_at_utc >= $2::timestamptz)
      AND ($3::timestamptz IS NULL OR breached_at_utc < $3::timestamptz)
    ORDER BY breached_at_utc DESC, breach_id ASC
    LIMIT $4
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphaHealthPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlphaHealthValidationIssue>,
}

impl AlphaHealthPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlphaHealthValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_health_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_health_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_health_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: AlphaHealthContractError) -> Self {
        Self {
            code: "alpha_health_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for AlphaHealthPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AlphaHealthPersistenceError {}

pub async fn upsert_alpha_health_metric<'e, E>(
    executor: E,
    record: &AlphaHealthMetricRecord,
) -> Result<(), AlphaHealthPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_metric_record_for_persistence(record).map_err(map_contract_error)?;
    let windows_json = json!({
        "windows": canonical.windows,
    });

    let result = sqlx::query(UPSERT_ALPHA_HEALTH_METRIC_SQL)
        .bind(&canonical.metric_id)
        .bind(&canonical.alpha_id)
        .bind(canonical.rolling_sharpe)
        .bind(canonical.rolling_hit_rate)
        .bind(canonical.rolling_drawdown)
        .bind(canonical.stability_score)
        .bind(windows_json)
        .bind(&canonical.reason_code)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.recorded_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_alpha_health_metric", error))?;

    if result.rows_affected() != 1 {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            format!(
                "upsert_alpha_health_metric expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_alpha_health_metric<'e, E>(
    executor: E,
    metric_id: &str,
) -> Result<Option<AlphaHealthMetricRecord>, AlphaHealthPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("metric_id", metric_id)?;
    let normalized_metric_id = normalize_research_identifier(metric_id);

    let row = sqlx::query(LOAD_ALPHA_HEALTH_METRIC_SQL)
        .bind(&normalized_metric_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            AlphaHealthPersistenceError::query_failure("load_alpha_health_metric", error)
        })?;

    row.map(decode_alpha_health_metric_row).transpose()
}

pub async fn upsert_alpha_health_metric_with_breaches(
    pool: &PgPool,
    metric: &AlphaHealthMetricRecord,
    breaches: &[AlphaThresholdBreachRecord],
) -> Result<(), AlphaHealthPersistenceError> {
    let mut transaction = pool.begin().await.map_err(|error| {
        AlphaHealthPersistenceError::query_failure(
            "upsert_alpha_health_metric_with_breaches_begin",
            error,
        )
    })?;
    upsert_alpha_health_metric(&mut *transaction, metric).await?;
    for breach in breaches {
        upsert_alpha_threshold_breach(&mut *transaction, breach).await?;
    }
    transaction.commit().await.map_err(|error| {
        AlphaHealthPersistenceError::query_failure(
            "upsert_alpha_health_metric_with_breaches_commit",
            error,
        )
    })?;
    Ok(())
}

pub async fn list_alpha_health_metrics_by_alpha<'e, E>(
    executor: E,
    alpha_id: &str,
    recorded_after_utc: Option<&str>,
    recorded_before_utc: Option<&str>,
    limit: i64,
) -> Result<Vec<AlphaHealthMetricRecord>, AlphaHealthPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("alpha_id", alpha_id)?;
    if limit <= 0 {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            "limit must be greater than 0",
            vec![AlphaHealthValidationIssue {
                field: "limit".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }

    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    let normalized_after = normalize_optional_timestamp("recorded_after_utc", recorded_after_utc)?;
    let normalized_before =
        normalize_optional_timestamp("recorded_before_utc", recorded_before_utc)?;

    if let (Some(recorded_after), Some(recorded_before)) =
        (normalized_after.as_deref(), normalized_before.as_deref())
    {
        let recorded_after_ts = parse_alpha_health_utc_timestamp(recorded_after)
            .map_err(map_contract_error)?;
        let recorded_before_ts = parse_alpha_health_utc_timestamp(recorded_before)
            .map_err(map_contract_error)?;
        if recorded_before_ts <= recorded_after_ts {
            return Err(AlphaHealthPersistenceError::invalid_payload(
                "recorded_before_utc must be greater than recorded_after_utc",
                vec![AlphaHealthValidationIssue {
                    field: "recorded_before_utc".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "recorded_before_utc must be greater than recorded_after_utc"
                        .to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LIST_ALPHA_HEALTH_METRICS_BY_ALPHA_SQL)
        .bind(&normalized_alpha_id)
        .bind(normalized_after.as_deref())
        .bind(normalized_before.as_deref())
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            AlphaHealthPersistenceError::query_failure("list_alpha_health_metrics_by_alpha", error)
        })?;

    rows.into_iter().map(decode_alpha_health_metric_row).collect()
}

pub async fn upsert_alpha_threshold_breach<'e, E>(
    executor: E,
    breach: &AlphaThresholdBreachRecord,
) -> Result<(), AlphaHealthPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_breach_record_for_persistence(breach).map_err(map_contract_error)?;
    let result = sqlx::query(UPSERT_ALPHA_THRESHOLD_BREACH_SQL)
        .bind(&canonical.breach_id)
        .bind(&canonical.metric_id)
        .bind(&canonical.alpha_id)
        .bind(canonical.metric_key.as_str())
        .bind(canonical.comparator.as_str())
        .bind(canonical.observed_value)
        .bind(canonical.threshold_value)
        .bind(&canonical.breach_reason)
        .bind(&canonical.reason_code)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.breached_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_alpha_threshold_breach", error))?;

    if result.rows_affected() != 1 {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            format!(
                "upsert_alpha_threshold_breach expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_alpha_threshold_breach<'e, E>(
    executor: E,
    breach_id: &str,
) -> Result<Option<AlphaThresholdBreachRecord>, AlphaHealthPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("breach_id", breach_id)?;
    let normalized_breach_id = normalize_research_identifier(breach_id);

    let row = sqlx::query(LOAD_ALPHA_THRESHOLD_BREACH_SQL)
        .bind(&normalized_breach_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            AlphaHealthPersistenceError::query_failure("load_alpha_threshold_breach", error)
        })?;

    row.map(decode_alpha_threshold_breach_row).transpose()
}

pub async fn list_alpha_threshold_breaches_by_alpha<'e, E>(
    executor: E,
    alpha_id: &str,
    breached_after_utc: Option<&str>,
    breached_before_utc: Option<&str>,
    limit: i64,
) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("alpha_id", alpha_id)?;
    if limit <= 0 {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            "limit must be greater than 0",
            vec![AlphaHealthValidationIssue {
                field: "limit".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    let normalized_after = normalize_optional_timestamp("breached_after_utc", breached_after_utc)?;
    let normalized_before =
        normalize_optional_timestamp("breached_before_utc", breached_before_utc)?;

    if let (Some(breached_after), Some(breached_before)) =
        (normalized_after.as_deref(), normalized_before.as_deref())
    {
        let breached_after_ts = parse_alpha_health_utc_timestamp(breached_after)
            .map_err(map_contract_error)?;
        let breached_before_ts = parse_alpha_health_utc_timestamp(breached_before)
            .map_err(map_contract_error)?;
        if breached_before_ts <= breached_after_ts {
            return Err(AlphaHealthPersistenceError::invalid_payload(
                "breached_before_utc must be greater than breached_after_utc",
                vec![AlphaHealthValidationIssue {
                    field: "breached_before_utc".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "breached_before_utc must be greater than breached_after_utc"
                        .to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LIST_ALPHA_THRESHOLD_BREACHES_BY_ALPHA_SQL)
        .bind(&normalized_alpha_id)
        .bind(normalized_after.as_deref())
        .bind(normalized_before.as_deref())
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            AlphaHealthPersistenceError::query_failure(
                "list_alpha_threshold_breaches_by_alpha",
                error,
            )
        })?;

    rows.into_iter()
        .map(decode_alpha_threshold_breach_row)
        .collect()
}

fn decode_alpha_health_metric_row(
    row: sqlx::postgres::PgRow,
) -> Result<AlphaHealthMetricRecord, AlphaHealthPersistenceError> {
    let windows_json: Value = row
        .try_get("windows_json")
        .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("windows_json", error))?;
    let windows = decode_windows_json(windows_json)?;

    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("reason_code", error))?;
    AlphaHealthReasonCode::parse(&reason_code)
        .map_err(|error| AlphaHealthPersistenceError::row_contract_failure("reason_code", error))?;

    let record = AlphaHealthMetricRecord {
        metric_id: row
            .try_get("metric_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("metric_id", error))?,
        alpha_id: row
            .try_get("alpha_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("alpha_id", error))?,
        rolling_sharpe: row.try_get("rolling_sharpe").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("rolling_sharpe", error)
        })?,
        rolling_hit_rate: row.try_get("rolling_hit_rate").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("rolling_hit_rate", error)
        })?,
        rolling_drawdown: row.try_get("rolling_drawdown").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("rolling_drawdown", error)
        })?,
        stability_score: row.try_get("stability_score").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("stability_score", error)
        })?,
        windows,
        reason_code,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        recorded_at_utc: row.try_get("recorded_at_utc").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("recorded_at_utc", error)
        })?,
    };

    validate_metric_record_for_persistence(&record)
        .map_err(|error| AlphaHealthPersistenceError::row_contract_failure("metric_record", error))
}

fn decode_alpha_threshold_breach_row(
    row: sqlx::postgres::PgRow,
) -> Result<AlphaThresholdBreachRecord, AlphaHealthPersistenceError> {
    let metric_key_raw: String = row
        .try_get("metric_key")
        .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("metric_key", error))?;
    let metric_key = AlphaHealthMetricKey::parse(&metric_key_raw)
        .map_err(|error| AlphaHealthPersistenceError::row_contract_failure("metric_key", error))?;

    let comparator_raw: String = row
        .try_get("comparator")
        .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("comparator", error))?;
    let comparator = parse_validation_comparator(&comparator_raw)?;

    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("reason_code", error))?;
    AlphaHealthReasonCode::parse(&reason_code)
        .map_err(|error| AlphaHealthPersistenceError::row_contract_failure("reason_code", error))?;

    let breach = AlphaThresholdBreachRecord {
        breach_id: row
            .try_get("breach_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("breach_id", error))?,
        metric_id: row
            .try_get("metric_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("metric_id", error))?,
        alpha_id: row
            .try_get("alpha_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("alpha_id", error))?,
        metric_key,
        comparator,
        observed_value: row.try_get("observed_value").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("observed_value", error)
        })?,
        threshold_value: row.try_get("threshold_value").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("threshold_value", error)
        })?,
        breach_reason: row.try_get("breach_reason").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("breach_reason", error)
        })?,
        reason_code,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| AlphaHealthPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        breached_at_utc: row.try_get("breached_at_utc").map_err(|error| {
            AlphaHealthPersistenceError::row_decode_failure("breached_at_utc", error)
        })?,
    };

    validate_breach_record_for_persistence(&breach)
        .map_err(|error| AlphaHealthPersistenceError::row_contract_failure("breach_record", error))
}

fn decode_windows_json(
    windows_json: Value,
) -> Result<Vec<AlphaHealthAttributionWindowMetrics>, AlphaHealthPersistenceError> {
    let Some(windows_value) = windows_json.get("windows") else {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            "windows_json payload must include `windows`",
            vec![AlphaHealthValidationIssue {
                field: "windows_json".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "windows_json payload must include `windows`".to_string(),
            }],
        ));
    };
    if !windows_value.is_array() {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            "windows_json.windows must be an array",
            vec![AlphaHealthValidationIssue {
                field: "windows_json.windows".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "windows_json.windows must be an array".to_string(),
            }],
        ));
    }
    serde_json::from_value(windows_value.clone()).map_err(|error| {
        AlphaHealthPersistenceError::invalid_payload(
            format!("invalid windows_json.windows payload: {error}"),
            vec![AlphaHealthValidationIssue {
                field: "windows_json.windows".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "windows_json.windows payload is invalid".to_string(),
            }],
        )
    })
}

fn parse_validation_comparator(
    value: &str,
) -> Result<ValidationGateComparator, AlphaHealthPersistenceError> {
    let comparator = match normalize_research_identifier(value).as_str() {
        "lt" => ValidationGateComparator::Lt,
        "lte" => ValidationGateComparator::Lte,
        "gt" => ValidationGateComparator::Gt,
        "gte" => ValidationGateComparator::Gte,
        _ => {
            return Err(AlphaHealthPersistenceError::invalid_payload(
                format!("unsupported comparator `{value}`"),
                vec![AlphaHealthValidationIssue {
                    field: "comparator".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: format!("unsupported comparator `{value}`"),
                }],
            ))
        }
    };
    Ok(comparator)
}

fn validate_metric_record_for_persistence(
    record: &AlphaHealthMetricRecord,
) -> Result<AlphaHealthMetricRecord, AlphaHealthContractError> {
    canonicalize_alpha_health_metric_record(record)
}

fn validate_breach_record_for_persistence(
    breach: &AlphaThresholdBreachRecord,
) -> Result<AlphaThresholdBreachRecord, AlphaHealthContractError> {
    canonicalize_alpha_threshold_breach_record(breach)
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, AlphaHealthPersistenceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_alpha_health_utc_timestamp(trimmed).map_err(|_| {
        AlphaHealthPersistenceError::invalid_payload(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![AlphaHealthValidationIssue {
                field: field.to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    Ok(Some(trimmed.to_string()))
}

fn map_contract_error(error: AlphaHealthContractError) -> AlphaHealthPersistenceError {
    AlphaHealthPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AlphaHealthPersistenceError> {
    if value.trim().is_empty() {
        return Err(AlphaHealthPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaHealthValidationIssue {
                field: field.to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> AlphaHealthPersistenceError {
    if is_constraint_error(&error) {
        return AlphaHealthPersistenceError::constraint_violation(operation, error);
    }
    AlphaHealthPersistenceError::query_failure(operation, error)
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

    const ALPHA_HEALTH_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260408023000_alpha_health_metrics_threshold_breaches.sql");

    fn sample_metric_record() -> AlphaHealthMetricRecord {
        AlphaHealthMetricRecord {
            metric_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            rolling_sharpe: 1.2,
            rolling_hit_rate: 0.61,
            rolling_drawdown: 0.07,
            stability_score: 0.82,
            windows: vec![
                AlphaHealthAttributionWindowMetrics {
                    window: domain::research::AlphaHealthMetricWindow::OneHour,
                    net_pnl: 14.0,
                    rolling_sharpe: 1.24,
                    rolling_hit_rate: 0.63,
                    rolling_drawdown: 0.03,
                    stability_score: 0.85,
                },
                AlphaHealthAttributionWindowMetrics {
                    window: domain::research::AlphaHealthMetricWindow::TwentyFourHours,
                    net_pnl: 79.0,
                    rolling_sharpe: 1.22,
                    rolling_hit_rate: 0.62,
                    rolling_drawdown: 0.05,
                    stability_score: 0.83,
                },
                AlphaHealthAttributionWindowMetrics {
                    window: domain::research::AlphaHealthMetricWindow::ThirtyDays,
                    net_pnl: 312.0,
                    rolling_sharpe: 1.2,
                    rolling_hit_rate: 0.61,
                    rolling_drawdown: 0.07,
                    stability_score: 0.82,
                },
            ],
            reason_code: AlphaHealthReasonCode::MetricRecorded.code().to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-health-001".to_string(),
            recorded_at_utc: "2026-04-08T01:00:00Z".to_string(),
        }
    }

    fn sample_breach_record() -> AlphaThresholdBreachRecord {
        AlphaThresholdBreachRecord {
            breach_id: "alpha::mean-reversion::rolling_sharpe::1712534400000000000".to_string(),
            metric_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            metric_key: AlphaHealthMetricKey::RollingSharpe,
            comparator: ValidationGateComparator::Lt,
            observed_value: 0.91,
            threshold_value: 1.0,
            breach_reason: "rolling_sharpe breached lt 1.0 with observed 0.91".to_string(),
            reason_code: AlphaHealthReasonCode::ThresholdBreachDetected
                .code()
                .to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-health-001".to_string(),
            breached_at_utc: "2026-04-08T01:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_alpha_health_schema_scope() {
        assert!(
            ALPHA_HEALTH_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS alpha_health_metrics")
        );
        assert!(
            ALPHA_HEALTH_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS alpha_threshold_breaches")
        );
        assert!(
            !ALPHA_HEALTH_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS promotion_decisions")
        );
        assert!(
            !ALPHA_HEALTH_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS validation_runs")
        );
    }

    #[test]
    fn migration_enforces_alpha_health_constraints_and_indexes() {
        assert!(ALPHA_HEALTH_MIGRATION_SQL.contains("windows_json ? 'windows'"));
        assert!(ALPHA_HEALTH_MIGRATION_SQL.contains("comparator = 'lt'"));
        assert!(ALPHA_HEALTH_MIGRATION_SQL.contains("comparator = 'gt'"));
        assert!(ALPHA_HEALTH_MIGRATION_SQL.contains("idx_alpha_health_metrics_alpha_lookup"));
        assert!(
            ALPHA_HEALTH_MIGRATION_SQL.contains("idx_alpha_threshold_breaches_alpha_lookup")
        );
    }

    #[test]
    fn alpha_health_validation_canonicalizes_identifiers_and_windows() {
        let mut record = sample_metric_record();
        record.metric_id = " Alpha::Mean-Reversion::1712534400000000000 ".to_string();
        record.alpha_id = " Alpha::Mean-Reversion ".to_string();
        record.reason_code = " Alpha_Health_Metric_Recorded ".to_string();
        record.windows.reverse();

        let canonical =
            validate_metric_record_for_persistence(&record).expect("canonical metric should pass");
        assert_eq!(
            canonical.metric_id,
            "alpha::mean-reversion::1712534400000000000"
        );
        assert_eq!(canonical.alpha_id, "alpha::mean-reversion");
        assert_eq!(
            canonical.reason_code,
            AlphaHealthReasonCode::MetricRecorded.code()
        );
        assert_eq!(
            canonical.windows[0].window,
            domain::research::AlphaHealthMetricWindow::OneHour
        );
        assert_eq!(
            canonical.windows[2].window,
            domain::research::AlphaHealthMetricWindow::ThirtyDays
        );
    }

    #[test]
    fn alpha_health_breach_validation_rejects_mismatched_comparators() {
        let mut breach = sample_breach_record();
        breach.metric_key = AlphaHealthMetricKey::RollingDrawdown;
        breach.comparator = ValidationGateComparator::Lt;
        let error = validate_breach_record_for_persistence(&breach)
            .expect_err("drawdown breaches must use gt comparator semantics");
        assert_eq!(error.code, AlphaHealthReasonCode::InvalidPayload.code());
        assert!(error.field_errors.iter().any(|issue| issue.field == "comparator"));
    }

    #[test]
    fn list_query_orders_alpha_health_metrics_deterministically() {
        assert!(
            LIST_ALPHA_HEALTH_METRICS_BY_ALPHA_SQL
                .contains("ORDER BY recorded_at_utc DESC, metric_id ASC")
        );
        assert!(
            LIST_ALPHA_THRESHOLD_BREACHES_BY_ALPHA_SQL
                .contains("ORDER BY breached_at_utc DESC, breach_id ASC")
        );
    }
}
