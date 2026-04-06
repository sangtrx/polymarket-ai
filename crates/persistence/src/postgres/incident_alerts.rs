use domain::alerts::{
    AlertContractError, AlertDeliveryAttempt, AlertDeliveryChannel, AlertDeliveryOutcome,
    AlertDispatchStatus, AlertReasonCode, AlertValidationIssue, IncidentAlert,
    normalize_alert_identifier, validate_alert_delivery_attempt, validate_incident_alert,
};
use sqlx::{PgExecutor, PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_INCIDENT_ALERT_SQL: &str = r#"
    INSERT INTO incident_alerts (
        alert_id,
        severity,
        impacted_subsystem,
        cause,
        recommended_next_action,
        evidence_link,
        issued_at,
        correlation_id,
        reason_code,
        status,
        delivered_at,
        failed_at
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7::timestamptz, $8, $9, $10, $11::timestamptz, $12::timestamptz
    )
"#;

const UPDATE_INCIDENT_ALERT_STATUS_SQL: &str = r#"
    UPDATE incident_alerts
    SET
        status = $2,
        reason_code = $3,
        delivered_at = $4::timestamptz,
        failed_at = $5::timestamptz
    WHERE alert_id = $1
"#;

const INSERT_ALERT_DELIVERY_ATTEMPT_SQL: &str = r#"
    INSERT INTO alert_delivery_attempts (
        alert_id,
        attempt_number,
        channel,
        outcome,
        reason_code,
        correlation_id,
        attempted_at,
        delivered_at,
        failed_at
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz, $9::timestamptz
    )
"#;

const LOAD_RECENT_INCIDENT_ALERTS_SQL: &str = r#"
    SELECT
        alert_id,
        severity,
        impacted_subsystem,
        cause,
        recommended_next_action,
        evidence_link,
        to_char(issued_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS issued_at,
        correlation_id,
        reason_code,
        status,
        to_char(delivered_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS delivered_at,
        to_char(failed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS failed_at
    FROM incident_alerts
    ORDER BY issued_at DESC, alert_id ASC
    LIMIT $1
"#;

const LOAD_ALERT_DELIVERY_ATTEMPTS_SQL: &str = r#"
    SELECT
        alert_id,
        attempt_number,
        channel,
        outcome,
        reason_code,
        correlation_id,
        to_char(attempted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS attempted_at,
        to_char(delivered_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS delivered_at,
        to_char(failed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS failed_at
    FROM alert_delivery_attempts
    WHERE alert_id = $1
    ORDER BY attempt_number ASC
    LIMIT $2
"#;

const ALERT_QUERY_FAILED: &str = "alert_query_failed";
const ALERT_CONSTRAINT_VIOLATION: &str = "alert_constraint_violation";
const ALERT_ROW_DECODE_FAILED: &str = "alert_row_decode_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlertValidationIssue>,
}

impl AlertPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlertValidationIssue>,
    ) -> Self {
        Self {
            code: AlertReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failed(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ALERT_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ALERT_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ALERT_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for AlertPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AlertPersistenceError {}

pub async fn create_incident_alert<'e, E>(
    executor: E,
    alert: &IncidentAlert,
) -> Result<(), AlertPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_incident_alert(alert).map_err(map_contract_error)?;
    let result = sqlx::query(INSERT_INCIDENT_ALERT_SQL)
        .bind(&alert.alert_id)
        .bind(alert.severity.as_str())
        .bind(&alert.impacted_subsystem)
        .bind(&alert.cause)
        .bind(&alert.recommended_next_action)
        .bind(&alert.evidence_link)
        .bind(&alert.issued_at)
        .bind(&alert.correlation_id)
        .bind(&alert.reason_code)
        .bind(alert.status.as_str())
        .bind(alert.delivered_at.as_deref())
        .bind(alert.failed_at.as_deref())
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("create_incident_alert", error))?;

    if result.rows_affected() != 1 {
        return Err(AlertPersistenceError::invalid_payload(
            format!(
                "create_incident_alert expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn update_incident_alert_status<'e, E>(
    executor: E,
    alert_id: &str,
    status: AlertDispatchStatus,
    reason_code: &str,
    delivered_at: Option<&str>,
    failed_at: Option<&str>,
) -> Result<(), AlertPersistenceError>
where
    E: PgExecutor<'e>,
{
    let normalized_alert_id = validate_lookup_identifier("alert_id", alert_id)?;
    AlertReasonCode::parse(reason_code).map_err(map_contract_error)?;
    let result = sqlx::query(UPDATE_INCIDENT_ALERT_STATUS_SQL)
        .bind(normalized_alert_id)
        .bind(status.as_str())
        .bind(reason_code)
        .bind(delivered_at)
        .bind(failed_at)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("update_incident_alert_status", error))?;

    if result.rows_affected() != 1 {
        return Err(AlertPersistenceError::invalid_payload(
            "update_incident_alert_status expected exactly one alert row".to_string(),
            vec![AlertValidationIssue {
                field: "alert_id",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "alert_id did not match any existing alert record".to_string(),
            }],
        ));
    }
    Ok(())
}

pub async fn append_alert_delivery_attempt<'e, E>(
    executor: E,
    attempt: &AlertDeliveryAttempt,
) -> Result<(), AlertPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_alert_delivery_attempt(attempt).map_err(map_contract_error)?;
    let result = sqlx::query(INSERT_ALERT_DELIVERY_ATTEMPT_SQL)
        .bind(&attempt.alert_id)
        .bind(i32::from(attempt.attempt_number))
        .bind(attempt.channel.as_str())
        .bind(attempt.outcome.as_str())
        .bind(&attempt.reason_code)
        .bind(&attempt.correlation_id)
        .bind(&attempt.attempted_at)
        .bind(attempt.delivered_at.as_deref())
        .bind(attempt.failed_at.as_deref())
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("append_alert_delivery_attempt", error))?;

    if result.rows_affected() != 1 {
        return Err(AlertPersistenceError::invalid_payload(
            format!(
                "append_alert_delivery_attempt expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_recent_incident_alerts(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<IncidentAlert>, AlertPersistenceError> {
    if limit <= 0 {
        return Err(AlertPersistenceError::invalid_payload(
            "limit must be greater than 0".to_string(),
            vec![AlertValidationIssue {
                field: "limit",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let rows = sqlx::query(LOAD_RECENT_INCIDENT_ALERTS_SQL)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_recent_incident_alerts", error))?;

    rows.into_iter().map(decode_incident_alert_row).collect()
}

pub async fn load_alert_delivery_attempts(
    pool: &PgPool,
    alert_id: &str,
    limit: i64,
) -> Result<Vec<AlertDeliveryAttempt>, AlertPersistenceError> {
    let normalized_alert_id = validate_lookup_identifier("alert_id", alert_id)?;
    if limit <= 0 {
        return Err(AlertPersistenceError::invalid_payload(
            "limit must be greater than 0".to_string(),
            vec![AlertValidationIssue {
                field: "limit",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let rows = sqlx::query(LOAD_ALERT_DELIVERY_ATTEMPTS_SQL)
        .bind(normalized_alert_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_alert_delivery_attempts", error))?;
    rows.into_iter().map(decode_delivery_attempt_row).collect()
}

fn decode_incident_alert_row(
    row: sqlx::postgres::PgRow,
) -> Result<IncidentAlert, AlertPersistenceError> {
    let severity: String = row
        .try_get("severity")
        .map_err(|error| AlertPersistenceError::row_decode_failure("severity", error))?;
    let severity = domain::alerts::AlertSeverity::parse(&severity).map_err(map_contract_error)?;

    let status: String = row
        .try_get("status")
        .map_err(|error| AlertPersistenceError::row_decode_failure("status", error))?;
    let status = AlertDispatchStatus::parse(&status).map_err(map_contract_error)?;

    let alert = IncidentAlert {
        alert_id: row
            .try_get("alert_id")
            .map_err(|error| AlertPersistenceError::row_decode_failure("alert_id", error))?,
        severity,
        impacted_subsystem: row.try_get("impacted_subsystem").map_err(|error| {
            AlertPersistenceError::row_decode_failure("impacted_subsystem", error)
        })?,
        cause: row
            .try_get("cause")
            .map_err(|error| AlertPersistenceError::row_decode_failure("cause", error))?,
        recommended_next_action: row.try_get("recommended_next_action").map_err(|error| {
            AlertPersistenceError::row_decode_failure("recommended_next_action", error)
        })?,
        evidence_link: row
            .try_get("evidence_link")
            .map_err(|error| AlertPersistenceError::row_decode_failure("evidence_link", error))?,
        issued_at: row
            .try_get("issued_at")
            .map_err(|error| AlertPersistenceError::row_decode_failure("issued_at", error))?,
        correlation_id: row
            .try_get("correlation_id")
            .map_err(|error| AlertPersistenceError::row_decode_failure("correlation_id", error))?,
        reason_code: row
            .try_get("reason_code")
            .map_err(|error| AlertPersistenceError::row_decode_failure("reason_code", error))?,
        status,
        delivered_at: row
            .try_get("delivered_at")
            .map_err(|error| AlertPersistenceError::row_decode_failure("delivered_at", error))?,
        failed_at: row
            .try_get("failed_at")
            .map_err(|error| AlertPersistenceError::row_decode_failure("failed_at", error))?,
    };
    validate_incident_alert(&alert).map_err(map_contract_error)?;
    Ok(alert)
}

fn decode_delivery_attempt_row(
    row: sqlx::postgres::PgRow,
) -> Result<AlertDeliveryAttempt, AlertPersistenceError> {
    let channel: String = row
        .try_get("channel")
        .map_err(|error| AlertPersistenceError::row_decode_failure("channel", error))?;
    let channel = AlertDeliveryChannel::parse(&channel).map_err(map_contract_error)?;
    let outcome: String = row
        .try_get("outcome")
        .map_err(|error| AlertPersistenceError::row_decode_failure("outcome", error))?;
    let outcome = AlertDeliveryOutcome::parse(&outcome).map_err(map_contract_error)?;
    let attempt_number: i32 = row
        .try_get("attempt_number")
        .map_err(|error| AlertPersistenceError::row_decode_failure("attempt_number", error))?;

    let attempt = AlertDeliveryAttempt {
        alert_id: row
            .try_get("alert_id")
            .map_err(|error| AlertPersistenceError::row_decode_failure("alert_id", error))?,
        attempt_number: u16::try_from(attempt_number).map_err(|_| {
            AlertPersistenceError::invalid_payload(
                "attempt_number must fit into u16".to_string(),
                vec![AlertValidationIssue {
                    field: "attempt_number",
                    code: AlertReasonCode::InvalidPayload.code(),
                    message: "attempt_number must fit into u16".to_string(),
                }],
            )
        })?,
        channel,
        outcome,
        reason_code: row
            .try_get("reason_code")
            .map_err(|error| AlertPersistenceError::row_decode_failure("reason_code", error))?,
        correlation_id: row
            .try_get("correlation_id")
            .map_err(|error| AlertPersistenceError::row_decode_failure("correlation_id", error))?,
        attempted_at: row
            .try_get("attempted_at")
            .map_err(|error| AlertPersistenceError::row_decode_failure("attempted_at", error))?,
        delivered_at: row
            .try_get("delivered_at")
            .map_err(|error| AlertPersistenceError::row_decode_failure("delivered_at", error))?,
        failed_at: row
            .try_get("failed_at")
            .map_err(|error| AlertPersistenceError::row_decode_failure("failed_at", error))?,
    };
    validate_alert_delivery_attempt(&attempt).map_err(map_contract_error)?;
    Ok(attempt)
}

fn validate_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, AlertPersistenceError> {
    let normalized = normalize_alert_identifier(value);
    if normalized.is_empty() || !(3..=120).contains(&normalized.len()) {
        return Err(AlertPersistenceError::invalid_payload(
            format!("{field} must contain 3-120 canonical characters"),
            vec![AlertValidationIssue {
                field,
                code: AlertReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-120 canonical characters"),
            }],
        ));
    }
    Ok(normalized)
}

fn map_contract_error(error: AlertContractError) -> AlertPersistenceError {
    AlertPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> AlertPersistenceError {
    if is_constraint_error(&error) {
        return AlertPersistenceError::constraint_violation(operation, error);
    }
    AlertPersistenceError::query_failed(operation, error)
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
    use domain::alerts::{AlertSeverity, compose_alert_identifier};

    const INCIDENT_ALERT_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406210000_incident_alerts_delivery_attempts.sql");

    fn sample_alert(status: AlertDispatchStatus) -> IncidentAlert {
        IncidentAlert {
            alert_id: compose_alert_identifier(
                "corr-alert-001",
                AlertReasonCode::DrawdownLimitExceeded,
                "2026-04-06T16:00:00Z",
            )
            .expect("identifier should compose"),
            severity: AlertSeverity::Critical,
            impacted_subsystem: "portfolio-risk".to_string(),
            cause: "Drawdown exceeded 80% of daily limit.".to_string(),
            recommended_next_action: "Trigger reduce-only containment and inspect risk limits."
                .to_string(),
            evidence_link: "https://docs.example.com/operations/severity-alert-delivery#drawdown"
                .to_string(),
            issued_at: "2026-04-06T16:00:00Z".to_string(),
            correlation_id: "corr-alert-001".to_string(),
            reason_code: AlertReasonCode::DrawdownLimitExceeded.code().to_string(),
            status,
            delivered_at: if status == AlertDispatchStatus::Delivered {
                Some("2026-04-06T16:00:10Z".to_string())
            } else {
                None
            },
            failed_at: if status == AlertDispatchStatus::Failed {
                Some("2026-04-06T16:00:15Z".to_string())
            } else {
                None
            },
        }
    }

    fn sample_attempt(outcome: AlertDeliveryOutcome) -> AlertDeliveryAttempt {
        AlertDeliveryAttempt {
            alert_id: sample_alert(AlertDispatchStatus::Pending).alert_id,
            attempt_number: 1,
            channel: AlertDeliveryChannel::PagerDuty,
            outcome,
            reason_code: if outcome == AlertDeliveryOutcome::Delivered {
                AlertReasonCode::Ready.code().to_string()
            } else {
                AlertReasonCode::DeliveryPrimaryFailed.code().to_string()
            },
            correlation_id: "corr-alert-001".to_string(),
            attempted_at: "2026-04-06T16:00:03Z".to_string(),
            delivered_at: if outcome == AlertDeliveryOutcome::Delivered {
                Some("2026-04-06T16:00:03Z".to_string())
            } else {
                None
            },
            failed_at: if outcome == AlertDeliveryOutcome::Failed {
                Some("2026-04-06T16:00:03Z".to_string())
            } else {
                None
            },
        }
    }

    #[test]
    fn migration_scope_creates_only_alert_tables() {
        assert!(
            INCIDENT_ALERT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS incident_alerts")
        );
        assert!(
            INCIDENT_ALERT_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS alert_delivery_attempts")
        );
        assert!(!INCIDENT_ALERT_MIGRATION_SQL.contains("incident_query_views"));
        assert!(!INCIDENT_ALERT_MIGRATION_SQL.contains("allocation_policies"));
        assert!(!INCIDENT_ALERT_MIGRATION_SQL.contains("risk_limit_profiles"));
    }

    #[test]
    fn migration_includes_required_enums_status_constraints_and_indexes() {
        assert!(INCIDENT_ALERT_MIGRATION_SQL.contains("severity IN ('warning', 'critical')"));
        assert!(
            INCIDENT_ALERT_MIGRATION_SQL.contains("status IN ('pending', 'delivered', 'failed')")
        );
        assert!(
            INCIDENT_ALERT_MIGRATION_SQL.contains("channel IN ('pagerduty', 'slack', 'email')")
        );
        assert!(INCIDENT_ALERT_MIGRATION_SQL.contains("idx_incident_alerts_severity_issued_at"));
        assert!(INCIDENT_ALERT_MIGRATION_SQL.contains("idx_incident_alerts_status_issued_at"));
        assert!(INCIDENT_ALERT_MIGRATION_SQL.contains("idx_incident_alerts_correlation_issued_at"));
        assert!(
            INCIDENT_ALERT_MIGRATION_SQL
                .contains("idx_alert_delivery_attempts_alert_id_attempt_number")
        );
    }

    #[test]
    fn adapter_queries_preserve_deterministic_ordering_contract() {
        assert!(LOAD_RECENT_INCIDENT_ALERTS_SQL.contains("ORDER BY issued_at DESC, alert_id ASC"));
        assert!(LOAD_ALERT_DELIVERY_ATTEMPTS_SQL.contains("ORDER BY attempt_number ASC"));
        assert!(LOAD_ALERT_DELIVERY_ATTEMPTS_SQL.contains("WHERE alert_id = $1"));
        assert!(UPDATE_INCIDENT_ALERT_STATUS_SQL.contains("WHERE alert_id = $1"));
    }

    #[test]
    fn sample_contract_payloads_are_valid_for_insert_adapters() {
        assert!(validate_incident_alert(&sample_alert(AlertDispatchStatus::Pending)).is_ok());
        assert!(validate_incident_alert(&sample_alert(AlertDispatchStatus::Delivered)).is_ok());
        assert!(validate_incident_alert(&sample_alert(AlertDispatchStatus::Failed)).is_ok());
        assert!(
            validate_alert_delivery_attempt(&sample_attempt(AlertDeliveryOutcome::Delivered))
                .is_ok()
        );
        assert!(
            validate_alert_delivery_attempt(&sample_attempt(AlertDeliveryOutcome::Failed)).is_ok()
        );
    }

    #[test]
    fn lookup_identifier_rejects_empty_values() {
        let error = validate_lookup_identifier("alert_id", "   ")
            .expect_err("empty lookup identifier should fail");
        assert_eq!(error.code, AlertReasonCode::InvalidPayload.code());
        assert_eq!(error.field_errors[0].field, "alert_id");
    }
}
