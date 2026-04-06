use domain::risk::{
    EmergencyControlAction, EmergencyControlContractError, EmergencyControlMode,
    EmergencyControlReasonCode, EmergencyControlSource, EmergencyControlTriggerSource,
    EmergencyControlValidationIssue, SafetyControlActionRecord,
    normalize_emergency_control_identifier, validate_safety_control_action_record,
};
use serde_json::json;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_SAFETY_CONTROL_ACTION_SQL: &str = r#"
    INSERT INTO safety_control_actions (
        action_id,
        correlation_id,
        source,
        action,
        trigger_source,
        actor_id,
        actor_role,
        resulting_mode,
        reason_code,
        audit_reference,
        dedupe_key,
        requested_at_utc,
        acknowledged_at_utc,
        effective_at_utc,
        completed_at_utc,
        details
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::timestamptz, $13::timestamptz, $14::timestamptz, $15::timestamptz, $16
    )
"#;

const LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_ACTION_ID_SQL: &str = r#"
    SELECT
        action_id,
        correlation_id,
        source,
        action,
        trigger_source,
        actor_id,
        actor_role,
        resulting_mode,
        reason_code,
        audit_reference,
        dedupe_key,
        to_char(requested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS requested_at_utc,
        to_char(acknowledged_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS acknowledged_at_utc,
        to_char(effective_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS effective_at_utc,
        to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS completed_at_utc
    FROM safety_control_actions
    WHERE action_id = $1
    ORDER BY effective_at_utc DESC, action_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_CORRELATION_SQL: &str = r#"
    SELECT
        action_id,
        correlation_id,
        source,
        action,
        trigger_source,
        actor_id,
        actor_role,
        resulting_mode,
        reason_code,
        audit_reference,
        dedupe_key,
        to_char(requested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS requested_at_utc,
        to_char(acknowledged_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS acknowledged_at_utc,
        to_char(effective_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS effective_at_utc,
        to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS completed_at_utc
    FROM safety_control_actions
    WHERE correlation_id = $1
    ORDER BY effective_at_utc DESC, action_id DESC
    LIMIT 1
"#;

const LOAD_CURRENT_EFFECTIVE_SAFETY_MODE_SQL: &str = r#"
    SELECT
        action_id,
        correlation_id,
        resulting_mode,
        reason_code,
        to_char(effective_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS effective_at_utc
    FROM safety_control_actions
    ORDER BY effective_at_utc DESC, action_id DESC
    LIMIT 1
"#;

const SAFETY_CONTROL_CONSTRAINT_VIOLATION: &str = "safety_control_constraint_violation";
const SAFETY_CONTROL_QUERY_FAILED: &str = "safety_control_query_failed";
const SAFETY_CONTROL_ROW_DECODE_FAILED: &str = "safety_control_row_decode_failed";
const SAFETY_CONTROL_RUNTIME_UNAVAILABLE: &str = "safety_control_runtime_unavailable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyControlPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<EmergencyControlValidationIssue>,
}

impl SafetyControlPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<EmergencyControlValidationIssue>,
    ) -> Self {
        Self {
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failed(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: SAFETY_CONTROL_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: SAFETY_CONTROL_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: SAFETY_CONTROL_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    pub fn runtime_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: SAFETY_CONTROL_RUNTIME_UNAVAILABLE,
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for SafetyControlPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for SafetyControlPersistenceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSafetyControlMode {
    pub action_id: String,
    pub correlation_id: String,
    pub resulting_mode: EmergencyControlMode,
    pub reason_code: String,
    pub effective_at_utc: String,
}

pub async fn insert_safety_control_action<'e, E>(
    executor: E,
    action: &SafetyControlActionRecord,
) -> Result<(), SafetyControlPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_safety_control_action_record(action).map_err(map_contract_error)?;
    let details = json!({
        "source": action.source.as_str(),
        "action": action.action.as_str(),
        "trigger_source": action.trigger_source.as_str(),
        "resulting_mode": action.resulting_mode.as_str(),
        "reason_code": action.reason_code,
    });

    let result = sqlx::query(INSERT_SAFETY_CONTROL_ACTION_SQL)
        .bind(&action.action_id)
        .bind(&action.correlation_id)
        .bind(action.source.as_str())
        .bind(action.action.as_str())
        .bind(action.trigger_source.as_str())
        .bind(action.actor_id.as_deref())
        .bind(action.actor_role.as_deref())
        .bind(action.resulting_mode.as_str())
        .bind(&action.reason_code)
        .bind(&action.audit_reference)
        .bind(&action.dedupe_key)
        .bind(&action.requested_at_utc)
        .bind(&action.acknowledged_at_utc)
        .bind(&action.effective_at_utc)
        .bind(&action.completed_at_utc)
        .bind(details)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_safety_control_action", error))?;

    if result.rows_affected() != 1 {
        return Err(SafetyControlPersistenceError::invalid_payload(
            format!(
                "insert_safety_control_action expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_latest_safety_control_action_by_action_id<'e, E>(
    executor: E,
    action_id: &str,
) -> Result<Option<SafetyControlActionRecord>, SafetyControlPersistenceError>
where
    E: PgExecutor<'e>,
{
    let normalized_action_id = normalize_lookup_identifier("action_id", action_id)?;
    let row = sqlx::query(LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_ACTION_ID_SQL)
        .bind(&normalized_action_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_safety_control_action_by_action_id", error)
        })?;
    row.map(decode_safety_control_row).transpose()
}

pub async fn load_latest_safety_control_action_by_correlation<'e, E>(
    executor: E,
    correlation_id: &str,
) -> Result<Option<SafetyControlActionRecord>, SafetyControlPersistenceError>
where
    E: PgExecutor<'e>,
{
    let normalized_correlation_id = normalize_lookup_identifier("correlation_id", correlation_id)?;
    let row = sqlx::query(LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_CORRELATION_SQL)
        .bind(&normalized_correlation_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_safety_control_action_by_correlation", error)
        })?;
    row.map(decode_safety_control_row).transpose()
}

pub async fn load_current_effective_safety_mode<'e, E>(
    executor: E,
) -> Result<Option<EffectiveSafetyControlMode>, SafetyControlPersistenceError>
where
    E: PgExecutor<'e>,
{
    let row = sqlx::query(LOAD_CURRENT_EFFECTIVE_SAFETY_MODE_SQL)
        .fetch_optional(executor)
        .await
        .map_err(|error| classify_query_error("load_current_effective_safety_mode", error))?;

    row.map(|row| {
        let resulting_mode: String = row.try_get("resulting_mode").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("resulting_mode", error)
        })?;
        let resulting_mode =
            EmergencyControlMode::parse(&resulting_mode).map_err(map_contract_error)?;

        let reason_code: String = row.try_get("reason_code").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("reason_code", error)
        })?;
        let reason_code =
            EmergencyControlReasonCode::parse(&reason_code).map_err(map_contract_error)?;

        Ok(EffectiveSafetyControlMode {
            action_id: row.try_get("action_id").map_err(|error| {
                SafetyControlPersistenceError::row_decode_failure("action_id", error)
            })?,
            correlation_id: row.try_get("correlation_id").map_err(|error| {
                SafetyControlPersistenceError::row_decode_failure("correlation_id", error)
            })?,
            resulting_mode,
            reason_code: reason_code.code().to_string(),
            effective_at_utc: row.try_get("effective_at_utc").map_err(|error| {
                SafetyControlPersistenceError::row_decode_failure("effective_at_utc", error)
            })?,
        })
    })
    .transpose()
}

fn decode_safety_control_row(
    row: sqlx::postgres::PgRow,
) -> Result<SafetyControlActionRecord, SafetyControlPersistenceError> {
    let source: String = row
        .try_get("source")
        .map_err(|error| SafetyControlPersistenceError::row_decode_failure("source", error))?;
    let source = EmergencyControlSource::parse(&source).map_err(map_contract_error)?;

    let action: String = row
        .try_get("action")
        .map_err(|error| SafetyControlPersistenceError::row_decode_failure("action", error))?;
    let action = EmergencyControlAction::parse(&action).map_err(map_contract_error)?;

    let trigger_source: String = row.try_get("trigger_source").map_err(|error| {
        SafetyControlPersistenceError::row_decode_failure("trigger_source", error)
    })?;
    let trigger_source =
        EmergencyControlTriggerSource::parse(&trigger_source).map_err(map_contract_error)?;

    let resulting_mode: String = row.try_get("resulting_mode").map_err(|error| {
        SafetyControlPersistenceError::row_decode_failure("resulting_mode", error)
    })?;
    let resulting_mode =
        EmergencyControlMode::parse(&resulting_mode).map_err(map_contract_error)?;

    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| SafetyControlPersistenceError::row_decode_failure("reason_code", error))?;
    let reason_code =
        EmergencyControlReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let action_record = SafetyControlActionRecord {
        action_id: row.try_get("action_id").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("action_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        source,
        action,
        trigger_source,
        actor_id: row.try_get("actor_id").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("actor_id", error)
        })?,
        actor_role: row.try_get("actor_role").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("actor_role", error)
        })?,
        resulting_mode,
        reason_code: reason_code.code().to_string(),
        audit_reference: row.try_get("audit_reference").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("audit_reference", error)
        })?,
        dedupe_key: row.try_get("dedupe_key").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("dedupe_key", error)
        })?,
        requested_at_utc: row.try_get("requested_at_utc").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("requested_at_utc", error)
        })?,
        acknowledged_at_utc: row.try_get("acknowledged_at_utc").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("acknowledged_at_utc", error)
        })?,
        effective_at_utc: row.try_get("effective_at_utc").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("effective_at_utc", error)
        })?,
        completed_at_utc: row.try_get("completed_at_utc").map_err(|error| {
            SafetyControlPersistenceError::row_decode_failure("completed_at_utc", error)
        })?,
    };
    validate_safety_control_action_record(&action_record).map_err(map_contract_error)?;
    Ok(action_record)
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, SafetyControlPersistenceError> {
    let normalized = normalize_emergency_control_identifier(value);
    if normalized.is_empty() {
        return Err(SafetyControlPersistenceError::invalid_payload(
            format!("{field} must not be empty"),
            vec![EmergencyControlValidationIssue {
                field,
                code: EmergencyControlReasonCode::InvalidPayload.code(),
                message: format!("{field} must not be empty"),
            }],
        ));
    }
    Ok(normalized)
}

fn map_contract_error(error: EmergencyControlContractError) -> SafetyControlPersistenceError {
    SafetyControlPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> SafetyControlPersistenceError {
    if is_constraint_error(&error) {
        return SafetyControlPersistenceError::constraint_violation(operation, error);
    }
    SafetyControlPersistenceError::query_failed(operation, error)
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

    const SAFETY_CONTROL_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406100000_safety_control_actions.sql");

    fn sample_action_record() -> SafetyControlActionRecord {
        SafetyControlActionRecord {
            action_id: "emergency::pause::0001".to_string(),
            source: EmergencyControlSource::Manual,
            action: EmergencyControlAction::Pause,
            trigger_source: EmergencyControlTriggerSource::OperatorCommand,
            actor_id: Some("ops-1".to_string()),
            actor_role: Some("operational_control".to_string()),
            resulting_mode: EmergencyControlMode::Paused,
            reason_code: EmergencyControlReasonCode::PauseActivated
                .code()
                .to_string(),
            correlation_id: "corr-emergency-0001".to_string(),
            audit_reference: "audit::emergency::0001".to_string(),
            dedupe_key: "manual::pause::corr-emergency-0001".to_string(),
            requested_at_utc: "2026-04-06T10:00:00Z".to_string(),
            acknowledged_at_utc: "2026-04-06T10:00:01Z".to_string(),
            effective_at_utc: "2026-04-06T10:00:05Z".to_string(),
            completed_at_utc: "2026-04-06T10:00:05Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_safety_control_schema_scope() {
        assert!(
            SAFETY_CONTROL_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS safety_control_actions")
        );
        assert!(!SAFETY_CONTROL_MIGRATION_SQL.contains("risk_limit_profiles"));
        assert!(!SAFETY_CONTROL_MIGRATION_SQL.contains("pretrade_gate_decisions"));
        assert!(!SAFETY_CONTROL_MIGRATION_SQL.contains("approval_requests"));
    }

    #[test]
    fn migration_enforces_constraints_and_indexes_for_traceability() {
        assert!(SAFETY_CONTROL_MIGRATION_SQL.contains("source IN ('manual', 'automatic')"));
        assert!(
            SAFETY_CONTROL_MIGRATION_SQL
                .contains("action IN ('pause', 'reduce_only', 'cancel_all')")
        );
        assert!(SAFETY_CONTROL_MIGRATION_SQL.contains("idx_safety_control_actions_action_id"));
        assert!(SAFETY_CONTROL_MIGRATION_SQL.contains("idx_safety_control_actions_dedupe_key"));
        assert!(
            SAFETY_CONTROL_MIGRATION_SQL
                .contains("idx_safety_control_actions_correlation_reason_source_time")
        );
        assert!(
            SAFETY_CONTROL_MIGRATION_SQL
                .contains("idx_safety_control_actions_resulting_mode_effective_time")
        );
    }

    #[test]
    fn action_validation_accepts_canonical_payload() {
        assert!(validate_safety_control_action_record(&sample_action_record()).is_ok());
    }

    #[test]
    fn lookup_identifier_normalization_rejects_empty_values() {
        let error = normalize_lookup_identifier("action_id", "   ")
            .expect_err("empty lookup values must be rejected");
        assert_eq!(
            error.code,
            EmergencyControlReasonCode::InvalidPayload.code()
        );
        assert_eq!(error.field_errors[0].field, "action_id");
    }

    #[test]
    fn latest_lookup_queries_remain_deterministic() {
        assert!(
            LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_ACTION_ID_SQL.contains("WHERE action_id = $1")
        );
        assert!(
            LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_CORRELATION_SQL
                .contains("WHERE correlation_id = $1")
        );
        assert!(
            LOAD_LATEST_SAFETY_CONTROL_ACTION_BY_CORRELATION_SQL
                .contains("ORDER BY effective_at_utc DESC, action_id DESC")
        );
        assert!(
            LOAD_CURRENT_EFFECTIVE_SAFETY_MODE_SQL
                .contains("ORDER BY effective_at_utc DESC, action_id DESC")
        );
    }
}
