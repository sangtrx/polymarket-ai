use domain::alerts::{AlertDispatchStatus, AlertReasonCode, AlertSeverity, validate_evidence_link};
use domain::risk::{
    RegimeShiftContractError, RegimeShiftReasonCode, RegimeShiftValidationIssue,
    VenueEligibilityState,
};
use sqlx::{PgExecutor, PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const INSERT_REGIME_SHIFT_ALERT_SQL: &str = r#"
    INSERT INTO regime_shift_alerts (
        alert_id,
        market_id,
        cluster_id,
        reason_code,
        severity,
        correlation_id,
        observed_at,
        issued_at,
        dispatch_status,
        dispatch_reason_code,
        recommended_next_action,
        evidence_link,
        previous_maker_rebate_bps,
        current_maker_rebate_bps,
        rebate_delta_bps,
        previous_spread_bps,
        current_spread_bps,
        spread_widening_bps,
        previous_eligibility_state,
        current_eligibility_state,
        threshold_rebate_delta_bps,
        threshold_spread_widening_bps
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz, $9, $10, $11, $12,
        $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
    )
"#;

const LOAD_REGIME_SHIFT_ALERTS_SQL: &str = r#"
    SELECT
        alert_id,
        market_id,
        cluster_id,
        reason_code,
        severity,
        correlation_id,
        to_char(observed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS observed_at,
        to_char(issued_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS issued_at,
        dispatch_status,
        dispatch_reason_code,
        recommended_next_action,
        evidence_link,
        previous_maker_rebate_bps,
        current_maker_rebate_bps,
        rebate_delta_bps,
        previous_spread_bps,
        current_spread_bps,
        spread_widening_bps,
        previous_eligibility_state,
        current_eligibility_state,
        threshold_rebate_delta_bps,
        threshold_spread_widening_bps
    FROM regime_shift_alerts
    WHERE ($1::text IS NULL OR market_id = lower(trim($1)))
      AND ($2::text IS NULL OR reason_code = $2)
      AND ($3::text IS NULL OR correlation_id = lower(trim($3)))
      AND ($4::timestamptz IS NULL OR observed_at >= $4::timestamptz)
      AND ($5::timestamptz IS NULL OR observed_at < $5::timestamptz)
    ORDER BY observed_at DESC, alert_id ASC
    LIMIT $6
"#;

const REGIME_SHIFT_QUERY_FAILED: &str = "regime_shift_query_failed";
const REGIME_SHIFT_CONSTRAINT_VIOLATION: &str = "regime_shift_constraint_violation";
const REGIME_SHIFT_ROW_DECODE_FAILED: &str = "regime_shift_row_decode_failed";

#[derive(Debug, Clone, PartialEq)]
pub struct RegimeShiftAlertRecord {
    pub alert_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub reason_code: String,
    pub severity: AlertSeverity,
    pub correlation_id: String,
    pub observed_at: String,
    pub issued_at: String,
    pub dispatch_status: AlertDispatchStatus,
    pub dispatch_reason_code: String,
    pub recommended_next_action: String,
    pub evidence_link: String,
    pub previous_maker_rebate_bps: Option<f64>,
    pub current_maker_rebate_bps: Option<f64>,
    pub rebate_delta_bps: Option<f64>,
    pub previous_spread_bps: Option<f64>,
    pub current_spread_bps: Option<f64>,
    pub spread_widening_bps: Option<f64>,
    pub previous_eligibility_state: Option<VenueEligibilityState>,
    pub current_eligibility_state: Option<VenueEligibilityState>,
    pub threshold_rebate_delta_bps: f64,
    pub threshold_spread_widening_bps: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegimeShiftPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RegimeShiftValidationIssue>,
}

impl RegimeShiftPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RegimeShiftValidationIssue>,
    ) -> Self {
        Self {
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failed(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REGIME_SHIFT_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REGIME_SHIFT_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REGIME_SHIFT_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RegimeShiftPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RegimeShiftPersistenceError {}

pub async fn create_regime_shift_alert<'e, E>(
    executor: E,
    alert: &RegimeShiftAlertRecord,
) -> Result<(), RegimeShiftPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_regime_shift_alert_record(alert)?;
    let result = sqlx::query(INSERT_REGIME_SHIFT_ALERT_SQL)
        .bind(&alert.alert_id)
        .bind(normalize_identifier(&alert.market_id))
        .bind(normalize_identifier(&alert.cluster_id))
        .bind(&alert.reason_code)
        .bind(alert.severity.as_str())
        .bind(normalize_identifier(&alert.correlation_id))
        .bind(&alert.observed_at)
        .bind(&alert.issued_at)
        .bind(alert.dispatch_status.as_str())
        .bind(&alert.dispatch_reason_code)
        .bind(&alert.recommended_next_action)
        .bind(&alert.evidence_link)
        .bind(alert.previous_maker_rebate_bps)
        .bind(alert.current_maker_rebate_bps)
        .bind(alert.rebate_delta_bps)
        .bind(alert.previous_spread_bps)
        .bind(alert.current_spread_bps)
        .bind(alert.spread_widening_bps)
        .bind(
            alert
                .previous_eligibility_state
                .as_ref()
                .map(|state| state.as_str()),
        )
        .bind(
            alert
                .current_eligibility_state
                .as_ref()
                .map(|state| state.as_str()),
        )
        .bind(alert.threshold_rebate_delta_bps)
        .bind(alert.threshold_spread_widening_bps)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("create_regime_shift_alert", error))?;

    if result.rows_affected() != 1 {
        return Err(RegimeShiftPersistenceError::invalid_payload(
            format!(
                "create_regime_shift_alert expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn load_regime_shift_alerts(
    pool: &PgPool,
    market_id: Option<&str>,
    reason_code: Option<&str>,
    correlation_id: Option<&str>,
    start_ts: Option<&str>,
    end_ts: Option<&str>,
    limit: i64,
) -> Result<Vec<RegimeShiftAlertRecord>, RegimeShiftPersistenceError> {
    let limit = validate_query_limit(limit)?;
    let market_id = normalize_optional_identifier("market_id", market_id)?;
    let correlation_id = normalize_optional_identifier("correlation_id", correlation_id)?;
    let reason_code = normalize_optional_reason_code(reason_code)?;
    let start_ts = normalize_optional_timestamp("start_ts", start_ts)?;
    let end_ts = normalize_optional_timestamp("end_ts", end_ts)?;
    if let (Some(start_ts), Some(end_ts)) = (start_ts.as_ref(), end_ts.as_ref()) {
        let start = parse_utc_timestamp(start_ts).map_err(map_contract_error)?;
        let end = parse_utc_timestamp(end_ts).map_err(map_contract_error)?;
        if end <= start {
            return Err(RegimeShiftPersistenceError::invalid_payload(
                "end_ts must be greater than start_ts".to_string(),
                vec![RegimeShiftValidationIssue {
                    field: "end_ts",
                    code: RegimeShiftReasonCode::InvalidPayload.code(),
                    message: "end_ts must be greater than start_ts".to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LOAD_REGIME_SHIFT_ALERTS_SQL)
        .bind(market_id.as_deref())
        .bind(reason_code.as_deref())
        .bind(correlation_id.as_deref())
        .bind(start_ts.as_deref())
        .bind(end_ts.as_deref())
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_regime_shift_alerts", error))?;

    rows.into_iter()
        .map(decode_regime_shift_alert_row)
        .collect()
}

fn decode_regime_shift_alert_row(
    row: sqlx::postgres::PgRow,
) -> Result<RegimeShiftAlertRecord, RegimeShiftPersistenceError> {
    let severity: String = row
        .try_get("severity")
        .map_err(|error| RegimeShiftPersistenceError::row_decode_failure("severity", error))?;
    let severity = AlertSeverity::parse(&severity).map_err(map_alert_contract_error)?;
    let dispatch_status: String = row.try_get("dispatch_status").map_err(|error| {
        RegimeShiftPersistenceError::row_decode_failure("dispatch_status", error)
    })?;
    let dispatch_status =
        AlertDispatchStatus::parse(&dispatch_status).map_err(map_alert_contract_error)?;

    let previous_eligibility_state = decode_eligibility_state(
        "previous_eligibility_state",
        row.try_get("previous_eligibility_state").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("previous_eligibility_state", error)
        })?,
    )?;
    let current_eligibility_state = decode_eligibility_state(
        "current_eligibility_state",
        row.try_get("current_eligibility_state").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("current_eligibility_state", error)
        })?,
    )?;

    let alert = RegimeShiftAlertRecord {
        alert_id: row
            .try_get("alert_id")
            .map_err(|error| RegimeShiftPersistenceError::row_decode_failure("alert_id", error))?,
        market_id: row
            .try_get("market_id")
            .map_err(|error| RegimeShiftPersistenceError::row_decode_failure("market_id", error))?,
        cluster_id: row.try_get("cluster_id").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("cluster_id", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("reason_code", error)
        })?,
        severity,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        observed_at: row.try_get("observed_at").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("observed_at", error)
        })?,
        issued_at: row
            .try_get("issued_at")
            .map_err(|error| RegimeShiftPersistenceError::row_decode_failure("issued_at", error))?,
        dispatch_status,
        dispatch_reason_code: row.try_get("dispatch_reason_code").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("dispatch_reason_code", error)
        })?,
        recommended_next_action: row.try_get("recommended_next_action").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("recommended_next_action", error)
        })?,
        evidence_link: row.try_get("evidence_link").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("evidence_link", error)
        })?,
        previous_maker_rebate_bps: row.try_get("previous_maker_rebate_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("previous_maker_rebate_bps", error)
        })?,
        current_maker_rebate_bps: row.try_get("current_maker_rebate_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("current_maker_rebate_bps", error)
        })?,
        rebate_delta_bps: row.try_get("rebate_delta_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("rebate_delta_bps", error)
        })?,
        previous_spread_bps: row.try_get("previous_spread_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("previous_spread_bps", error)
        })?,
        current_spread_bps: row.try_get("current_spread_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("current_spread_bps", error)
        })?,
        spread_widening_bps: row.try_get("spread_widening_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("spread_widening_bps", error)
        })?,
        previous_eligibility_state,
        current_eligibility_state,
        threshold_rebate_delta_bps: row.try_get("threshold_rebate_delta_bps").map_err(|error| {
            RegimeShiftPersistenceError::row_decode_failure("threshold_rebate_delta_bps", error)
        })?,
        threshold_spread_widening_bps: row.try_get("threshold_spread_widening_bps").map_err(
            |error| {
                RegimeShiftPersistenceError::row_decode_failure(
                    "threshold_spread_widening_bps",
                    error,
                )
            },
        )?,
    };
    validate_regime_shift_alert_record(&alert)?;
    Ok(alert)
}

fn decode_eligibility_state(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<VenueEligibilityState>, RegimeShiftPersistenceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let state = VenueEligibilityState::parse(&value).map_err(map_contract_error)?;
    if state.as_str() != value.trim().to_ascii_lowercase() {
        return Err(RegimeShiftPersistenceError::invalid_payload(
            format!("{field} must be canonical lower snake_case"),
            vec![RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: format!("{field} must be canonical lower snake_case"),
            }],
        ));
    }
    Ok(Some(state))
}

fn validate_regime_shift_alert_record(
    alert: &RegimeShiftAlertRecord,
) -> Result<(), RegimeShiftPersistenceError> {
    let mut field_errors = Vec::new();
    validate_identifier(&mut field_errors, "alert_id", &alert.alert_id);
    validate_identifier(&mut field_errors, "market_id", &alert.market_id);
    validate_identifier(&mut field_errors, "cluster_id", &alert.cluster_id);
    validate_identifier(&mut field_errors, "correlation_id", &alert.correlation_id);
    validate_non_empty_field(
        &mut field_errors,
        "recommended_next_action",
        &alert.recommended_next_action,
    );
    validate_non_empty_field(&mut field_errors, "evidence_link", &alert.evidence_link);
    validate_non_empty_field(&mut field_errors, "reason_code", &alert.reason_code);
    validate_non_empty_field(
        &mut field_errors,
        "dispatch_reason_code",
        &alert.dispatch_reason_code,
    );
    let observed_at = match parse_utc_timestamp(&alert.observed_at) {
        Ok(timestamp) => Some(timestamp),
        Err(_) => {
            field_errors.push(RegimeShiftValidationIssue {
                field: "observed_at",
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: "observed_at must be an RFC3339 UTC timestamp".to_string(),
            });
            None
        }
    };
    let issued_at = match parse_utc_timestamp(&alert.issued_at) {
        Ok(timestamp) => Some(timestamp),
        Err(_) => {
            field_errors.push(RegimeShiftValidationIssue {
                field: "issued_at",
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: "issued_at must be an RFC3339 UTC timestamp".to_string(),
            });
            None
        }
    };
    if let (Some(observed_at), Some(issued_at)) = (observed_at, issued_at)
        && issued_at < observed_at
    {
        field_errors.push(RegimeShiftValidationIssue {
            field: "issued_at",
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: "issued_at must be greater than or equal to observed_at".to_string(),
        });
    }
    if RegimeShiftReasonCode::parse(&alert.reason_code).is_err() {
        field_errors.push(RegimeShiftValidationIssue {
            field: "reason_code",
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported FR40 regime-shift reason code".to_string(),
        });
    }
    if AlertReasonCode::parse(&alert.dispatch_reason_code).is_err() {
        field_errors.push(RegimeShiftValidationIssue {
            field: "dispatch_reason_code",
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: "dispatch_reason_code is not a supported alert reason code".to_string(),
        });
    }
    if let Err(error) = validate_evidence_link(&alert.evidence_link) {
        for issue in error.field_errors {
            field_errors.push(RegimeShiftValidationIssue {
                field: issue.field,
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: issue.message,
            });
        }
    }
    validate_optional_finite(
        &mut field_errors,
        "previous_maker_rebate_bps",
        alert.previous_maker_rebate_bps,
    );
    validate_optional_finite(
        &mut field_errors,
        "current_maker_rebate_bps",
        alert.current_maker_rebate_bps,
    );
    validate_optional_finite(
        &mut field_errors,
        "rebate_delta_bps",
        alert.rebate_delta_bps,
    );
    validate_optional_finite(
        &mut field_errors,
        "previous_spread_bps",
        alert.previous_spread_bps,
    );
    validate_optional_finite(
        &mut field_errors,
        "current_spread_bps",
        alert.current_spread_bps,
    );
    validate_optional_finite(
        &mut field_errors,
        "spread_widening_bps",
        alert.spread_widening_bps,
    );
    validate_non_negative_threshold(
        &mut field_errors,
        "threshold_rebate_delta_bps",
        alert.threshold_rebate_delta_bps,
    );
    validate_non_negative_threshold(
        &mut field_errors,
        "threshold_spread_widening_bps",
        alert.threshold_spread_widening_bps,
    );
    if field_errors.is_empty() {
        return Ok(());
    }
    Err(RegimeShiftPersistenceError::invalid_payload(
        "regime-shift alert record failed validation",
        field_errors,
    ))
}

fn validate_query_limit(limit: i64) -> Result<i64, RegimeShiftPersistenceError> {
    if (1..=200).contains(&limit) {
        return Ok(limit);
    }
    Err(RegimeShiftPersistenceError::invalid_payload(
        "limit must be between 1 and 200".to_string(),
        vec![RegimeShiftValidationIssue {
            field: "limit",
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: "limit must be between 1 and 200".to_string(),
        }],
    ))
}

fn normalize_optional_identifier(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, RegimeShiftPersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = normalize_identifier(value);
    if !is_canonical_identifier(&normalized) {
        return Err(RegimeShiftPersistenceError::invalid_payload(
            format!("{field} must contain 3-120 canonical characters"),
            vec![RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-120 canonical characters"),
            }],
        ));
    }
    Ok(Some(normalized))
}

fn normalize_optional_reason_code(
    reason_code: Option<&str>,
) -> Result<Option<String>, RegimeShiftPersistenceError> {
    let Some(reason_code) = reason_code.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    RegimeShiftReasonCode::parse(reason_code).map_err(map_contract_error)?;
    Ok(Some(reason_code.to_string()))
}

fn normalize_optional_timestamp(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, RegimeShiftPersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    parse_utc_timestamp(value).map_err(|_| {
        RegimeShiftPersistenceError::invalid_payload(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    Ok(Some(value.to_string()))
}

fn validate_identifier(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: &str,
) {
    let normalized = normalize_identifier(value);
    if normalized.is_empty() || !is_canonical_identifier(&normalized) {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-120 canonical characters"),
        });
    }
}

fn validate_non_empty_field(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_optional_finite(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: Option<f64>,
) {
    if value.is_some_and(|value| !value.is_finite()) {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    }
}

fn validate_non_negative_threshold(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() || value < 0.0 {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite and >= 0"),
        });
    }
}

fn map_contract_error(error: RegimeShiftContractError) -> RegimeShiftPersistenceError {
    RegimeShiftPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn map_alert_contract_error(
    error: domain::alerts::AlertContractError,
) -> RegimeShiftPersistenceError {
    RegimeShiftPersistenceError::invalid_payload(
        error.message,
        error
            .field_errors
            .into_iter()
            .map(|issue| RegimeShiftValidationIssue {
                field: issue.field,
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: issue.message,
            })
            .collect(),
    )
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> RegimeShiftPersistenceError {
    if is_constraint_error(&error) {
        return RegimeShiftPersistenceError::constraint_violation(operation, error);
    }
    RegimeShiftPersistenceError::query_failed(operation, error)
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

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, RegimeShiftContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        RegimeShiftContractError::invalid_payload_with_issues(
            format!("timestamp `{value}` must be RFC3339 UTC"),
            vec![RegimeShiftValidationIssue {
                field: "timestamp",
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: format!("timestamp `{value}` must be RFC3339 UTC"),
            }],
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(RegimeShiftContractError::invalid_payload_with_issues(
            "timestamps must use UTC `Z` offset",
            vec![RegimeShiftValidationIssue {
                field: "timestamp",
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: "timestamps must use UTC `Z` offset".to_string(),
            }],
        ));
    }
    Ok(parsed)
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
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

#[cfg(test)]
mod tests {
    use super::*;

    const REGIME_SHIFT_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407103000_regime_shift_alerts.sql");

    fn sample_alert() -> RegimeShiftAlertRecord {
        RegimeShiftAlertRecord {
            alert_id:
                "incident::alert::alert_regime_rebate_delta_exceeded::corr-regime-001::1712422800"
                    .to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            reason_code: RegimeShiftReasonCode::RebateDeltaExceeded
                .code()
                .to_string(),
            severity: AlertSeverity::Critical,
            correlation_id: "corr-regime-001".to_string(),
            observed_at: "2026-04-07T03:00:00Z".to_string(),
            issued_at: "2026-04-07T03:00:01Z".to_string(),
            dispatch_status: AlertDispatchStatus::Delivered,
            dispatch_reason_code: AlertReasonCode::Ready.code().to_string(),
            recommended_next_action:
                "Review liquidity incentives and tune participation thresholds before next cycle."
                    .to_string(),
            evidence_link: "https://docs.example.com/operations/incentive-regime-shift-alerts#fr40"
                .to_string(),
            previous_maker_rebate_bps: Some(8.0),
            current_maker_rebate_bps: Some(31.0),
            rebate_delta_bps: Some(23.0),
            previous_spread_bps: Some(80.0),
            current_spread_bps: Some(135.0),
            spread_widening_bps: Some(55.0),
            previous_eligibility_state: Some(VenueEligibilityState::Eligible),
            current_eligibility_state: Some(VenueEligibilityState::Restricted),
            threshold_rebate_delta_bps: 20.0,
            threshold_spread_widening_bps: 50.0,
        }
    }

    #[test]
    fn migration_scope_creates_only_regime_shift_alerts_table() {
        assert!(
            REGIME_SHIFT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS regime_shift_alerts")
        );
        assert!(!REGIME_SHIFT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS incident_alerts"));
        assert!(
            !REGIME_SHIFT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS risk_limit_profiles")
        );
    }

    #[test]
    fn migration_includes_required_constraints_and_triage_indexes() {
        assert!(REGIME_SHIFT_MIGRATION_SQL.contains("severity IN ('warning', 'critical')"));
        assert!(
            REGIME_SHIFT_MIGRATION_SQL
                .contains("dispatch_status IN ('pending', 'delivered', 'failed')")
        );
        assert!(REGIME_SHIFT_MIGRATION_SQL.contains("CHECK (issued_at >= observed_at)"));
        assert!(REGIME_SHIFT_MIGRATION_SQL.contains("idx_regime_shift_alerts_market_observed_at"));
        assert!(REGIME_SHIFT_MIGRATION_SQL.contains("idx_regime_shift_alerts_reason_observed_at"));
        assert!(
            REGIME_SHIFT_MIGRATION_SQL.contains("idx_regime_shift_alerts_correlation_observed_at")
        );
    }

    #[test]
    fn adapter_query_contract_preserves_deterministic_ordering() {
        assert!(LOAD_REGIME_SHIFT_ALERTS_SQL.contains("ORDER BY observed_at DESC, alert_id ASC"));
        assert!(LOAD_REGIME_SHIFT_ALERTS_SQL.contains("market_id = lower(trim($1))"));
        assert!(LOAD_REGIME_SHIFT_ALERTS_SQL.contains("reason_code = $2"));
        assert!(LOAD_REGIME_SHIFT_ALERTS_SQL.contains("correlation_id = lower(trim($3))"));
    }

    #[test]
    fn validation_rejects_unknown_regime_reason_codes() {
        let mut alert = sample_alert();
        alert.reason_code = "fr40_regime_unknown".to_string();
        let error = validate_regime_shift_alert_record(&alert)
            .expect_err("unknown FR40 reason code should fail validation");
        assert_eq!(error.code, RegimeShiftReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "reason_code")
        );
    }

    #[test]
    fn validation_rejects_issued_at_before_observed_at() {
        let mut alert = sample_alert();
        alert.observed_at = "2026-04-07T03:00:01Z".to_string();
        alert.issued_at = "2026-04-07T03:00:00Z".to_string();

        let error = validate_regime_shift_alert_record(&alert)
            .expect_err("issued_at before observed_at should fail validation");
        assert_eq!(error.code, RegimeShiftReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "issued_at")
        );
    }

    #[test]
    fn query_filter_validation_rejects_invalid_identifier() {
        let error = normalize_optional_identifier("market_id", Some("invalid whitespace"))
            .expect_err("invalid identifiers should fail");
        assert_eq!(error.code, RegimeShiftReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
    }
}
