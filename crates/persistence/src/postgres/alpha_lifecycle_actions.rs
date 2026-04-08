use domain::research::{
    AlphaLifecycleActionRecord, AlphaLifecycleActionStatus, AlphaLifecycleActionType,
    AlphaLifecycleContractError, AlphaLifecycleReasonCode, AlphaLifecycleValidationIssue,
    StopResearchCriteriaSnapshot, canonicalize_alpha_lifecycle_action_record,
    normalize_research_identifier, parse_alpha_lifecycle_utc_timestamp,
};
use serde_json::Value;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const MAX_LIST_LIMIT: i64 = 200;

const UPSERT_ALPHA_LIFECYCLE_ACTION_SQL: &str = r#"
    INSERT INTO alpha_lifecycle_actions (
        action_id,
        alpha_id,
        action_type,
        action_status,
        reason_code,
        trigger_evidence_json,
        stop_research_criteria_json,
        remediation_guidance,
        actor_id,
        correlation_id,
        acted_at_utc,
        approval_request_id,
        approval_reference,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12, $13, NOW()
    )
    ON CONFLICT (action_id)
    DO UPDATE SET
        alpha_id = EXCLUDED.alpha_id,
        action_type = EXCLUDED.action_type,
        action_status = EXCLUDED.action_status,
        reason_code = EXCLUDED.reason_code,
        trigger_evidence_json = EXCLUDED.trigger_evidence_json,
        stop_research_criteria_json = EXCLUDED.stop_research_criteria_json,
        remediation_guidance = EXCLUDED.remediation_guidance,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        acted_at_utc = EXCLUDED.acted_at_utc,
        approval_request_id = EXCLUDED.approval_request_id,
        approval_reference = EXCLUDED.approval_reference,
        updated_at_utc = NOW()
"#;

const LOAD_ALPHA_LIFECYCLE_ACTION_SQL: &str = r#"
    SELECT
        action_id,
        alpha_id,
        action_type,
        action_status,
        reason_code,
        trigger_evidence_json,
        stop_research_criteria_json,
        remediation_guidance,
        actor_id,
        correlation_id,
        to_char(acted_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS acted_at_utc,
        approval_request_id,
        approval_reference
    FROM alpha_lifecycle_actions
    WHERE lower(trim(action_id)) = lower(trim($1))
    ORDER BY acted_at_utc DESC, action_id ASC
    LIMIT 1
"#;

const LIST_ALPHA_LIFECYCLE_ACTIONS_BY_ALPHA_SQL: &str = r#"
    SELECT
        action_id,
        alpha_id,
        action_type,
        action_status,
        reason_code,
        trigger_evidence_json,
        stop_research_criteria_json,
        remediation_guidance,
        actor_id,
        correlation_id,
        to_char(acted_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS acted_at_utc,
        approval_request_id,
        approval_reference
    FROM alpha_lifecycle_actions
    WHERE lower(trim(alpha_id)) = lower(trim($1))
      AND ($2::timestamptz IS NULL OR acted_at_utc >= $2::timestamptz)
      AND ($3::timestamptz IS NULL OR acted_at_utc < $3::timestamptz)
    ORDER BY acted_at_utc DESC, action_id ASC
    LIMIT $4
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphaLifecyclePersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlphaLifecycleValidationIssue>,
}

impl AlphaLifecyclePersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlphaLifecycleValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_lifecycle_action_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_lifecycle_action_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_lifecycle_action_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: AlphaLifecycleContractError) -> Self {
        Self {
            code: "alpha_lifecycle_action_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for AlphaLifecyclePersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AlphaLifecyclePersistenceError {}

pub async fn upsert_alpha_lifecycle_action<'e, E>(
    executor: E,
    record: &AlphaLifecycleActionRecord,
) -> Result<(), AlphaLifecyclePersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_action_record_for_persistence(record).map_err(map_contract_error)?;

    let result = sqlx::query(UPSERT_ALPHA_LIFECYCLE_ACTION_SQL)
        .bind(&canonical.action_id)
        .bind(&canonical.alpha_id)
        .bind(canonical.action_type.as_str())
        .bind(canonical.action_status.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.trigger_evidence)
        .bind(
            canonical
                .stop_research_criteria
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| {
                    AlphaLifecyclePersistenceError::invalid_payload(
                        format!("unable to serialize stop_research_criteria: {error}"),
                        vec![AlphaLifecycleValidationIssue {
                            field: "stop_research_criteria".to_string(),
                            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                            message: "stop_research_criteria payload is invalid".to_string(),
                        }],
                    )
                })?,
        )
        .bind(canonical.remediation_guidance.as_deref())
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.acted_at_utc)
        .bind(canonical.approval_request_id.as_deref())
        .bind(canonical.approval_reference.as_deref())
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_alpha_lifecycle_action", error))?;

    if result.rows_affected() != 1 {
        return Err(AlphaLifecyclePersistenceError::invalid_payload(
            format!(
                "upsert_alpha_lifecycle_action expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_alpha_lifecycle_action<'e, E>(
    executor: E,
    action_id: &str,
) -> Result<Option<AlphaLifecycleActionRecord>, AlphaLifecyclePersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("action_id", action_id)?;
    let normalized_action_id = normalize_research_identifier(action_id);

    let row = sqlx::query(LOAD_ALPHA_LIFECYCLE_ACTION_SQL)
        .bind(&normalized_action_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            AlphaLifecyclePersistenceError::query_failure("load_alpha_lifecycle_action", error)
        })?;

    row.map(decode_alpha_lifecycle_action_row).transpose()
}

pub async fn list_alpha_lifecycle_actions_by_alpha<'e, E>(
    executor: E,
    alpha_id: &str,
    acted_after_utc: Option<&str>,
    acted_before_utc: Option<&str>,
    limit: i64,
) -> Result<Vec<AlphaLifecycleActionRecord>, AlphaLifecyclePersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("alpha_id", alpha_id)?;
    validate_list_limit(limit)?;

    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    let normalized_after = normalize_optional_timestamp("acted_after_utc", acted_after_utc)?;
    let normalized_before = normalize_optional_timestamp("acted_before_utc", acted_before_utc)?;

    if let (Some(acted_after), Some(acted_before)) =
        (normalized_after.as_deref(), normalized_before.as_deref())
    {
        let acted_after_ts =
            parse_alpha_lifecycle_utc_timestamp(acted_after).map_err(map_contract_error)?;
        let acted_before_ts =
            parse_alpha_lifecycle_utc_timestamp(acted_before).map_err(map_contract_error)?;
        if acted_before_ts <= acted_after_ts {
            return Err(AlphaLifecyclePersistenceError::invalid_payload(
                "acted_before_utc must be greater than acted_after_utc",
                vec![AlphaLifecycleValidationIssue {
                    field: "acted_before_utc".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: "acted_before_utc must be greater than acted_after_utc".to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LIST_ALPHA_LIFECYCLE_ACTIONS_BY_ALPHA_SQL)
        .bind(&normalized_alpha_id)
        .bind(normalized_after.as_deref())
        .bind(normalized_before.as_deref())
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            AlphaLifecyclePersistenceError::query_failure(
                "list_alpha_lifecycle_actions_by_alpha",
                error,
            )
        })?;

    rows.into_iter()
        .map(decode_alpha_lifecycle_action_row)
        .collect()
}

fn decode_alpha_lifecycle_action_row(
    row: sqlx::postgres::PgRow,
) -> Result<AlphaLifecycleActionRecord, AlphaLifecyclePersistenceError> {
    let action_type_raw: String = row.try_get("action_type").map_err(|error| {
        AlphaLifecyclePersistenceError::row_decode_failure("action_type", error)
    })?;
    let action_type = AlphaLifecycleActionType::parse(&action_type_raw).map_err(|error| {
        AlphaLifecyclePersistenceError::row_contract_failure("action_type", error)
    })?;

    let action_status_raw: String = row.try_get("action_status").map_err(|error| {
        AlphaLifecyclePersistenceError::row_decode_failure("action_status", error)
    })?;
    let action_status = AlphaLifecycleActionStatus::parse(&action_status_raw).map_err(|error| {
        AlphaLifecyclePersistenceError::row_contract_failure("action_status", error)
    })?;

    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        AlphaLifecyclePersistenceError::row_decode_failure("reason_code", error)
    })?;
    AlphaLifecycleReasonCode::parse(&reason_code).map_err(|error| {
        AlphaLifecyclePersistenceError::row_contract_failure("reason_code", error)
    })?;

    let trigger_evidence: Value = row.try_get("trigger_evidence_json").map_err(|error| {
        AlphaLifecyclePersistenceError::row_decode_failure("trigger_evidence_json", error)
    })?;

    let stop_research_criteria = decode_stop_research_criteria(
        row.try_get("stop_research_criteria_json")
            .map_err(|error| {
                AlphaLifecyclePersistenceError::row_decode_failure(
                    "stop_research_criteria_json",
                    error,
                )
            })?,
    )?;

    let record = AlphaLifecycleActionRecord {
        action_id: row.try_get("action_id").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("action_id", error)
        })?,
        alpha_id: row.try_get("alpha_id").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("alpha_id", error)
        })?,
        action_type,
        action_status,
        reason_code,
        trigger_evidence,
        stop_research_criteria,
        remediation_guidance: row.try_get("remediation_guidance").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("remediation_guidance", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("correlation_id", error)
        })?,
        acted_at_utc: row.try_get("acted_at_utc").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("acted_at_utc", error)
        })?,
        approval_request_id: row.try_get("approval_request_id").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("approval_request_id", error)
        })?,
        approval_reference: row.try_get("approval_reference").map_err(|error| {
            AlphaLifecyclePersistenceError::row_decode_failure("approval_reference", error)
        })?,
    };

    validate_action_record_for_persistence(&record)
        .map_err(|error| AlphaLifecyclePersistenceError::row_contract_failure("record", error))
}

fn decode_stop_research_criteria(
    payload: Option<Value>,
) -> Result<Option<StopResearchCriteriaSnapshot>, AlphaLifecyclePersistenceError> {
    let Some(payload) = payload else {
        return Ok(None);
    };
    if !payload.is_object() {
        return Err(AlphaLifecyclePersistenceError::invalid_payload(
            "stop_research_criteria_json must be a JSON object",
            vec![AlphaLifecycleValidationIssue {
                field: "stop_research_criteria_json".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "stop_research_criteria_json must be a JSON object".to_string(),
            }],
        ));
    }
    serde_json::from_value(payload).map(Some).map_err(|error| {
        AlphaLifecyclePersistenceError::invalid_payload(
            format!("invalid stop_research_criteria_json payload: {error}"),
            vec![AlphaLifecycleValidationIssue {
                field: "stop_research_criteria_json".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "stop_research_criteria_json payload is invalid".to_string(),
            }],
        )
    })
}

fn validate_action_record_for_persistence(
    record: &AlphaLifecycleActionRecord,
) -> Result<AlphaLifecycleActionRecord, AlphaLifecycleContractError> {
    canonicalize_alpha_lifecycle_action_record(record)
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, AlphaLifecyclePersistenceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_alpha_lifecycle_utc_timestamp(trimmed).map_err(|_| {
        AlphaLifecyclePersistenceError::invalid_payload(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    Ok(Some(trimmed.to_string()))
}

fn map_contract_error(error: AlphaLifecycleContractError) -> AlphaLifecyclePersistenceError {
    AlphaLifecyclePersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AlphaLifecyclePersistenceError> {
    if value.trim().is_empty() {
        return Err(AlphaLifecyclePersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn validate_list_limit(limit: i64) -> Result<(), AlphaLifecyclePersistenceError> {
    if limit <= 0 || limit > MAX_LIST_LIMIT {
        return Err(AlphaLifecyclePersistenceError::invalid_payload(
            format!("limit must be between 1 and {MAX_LIST_LIMIT}"),
            vec![AlphaLifecycleValidationIssue {
                field: "limit".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("limit must be between 1 and {MAX_LIST_LIMIT}"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> AlphaLifecyclePersistenceError {
    if is_constraint_error(&error) {
        return AlphaLifecyclePersistenceError::constraint_violation(operation, error);
    }
    AlphaLifecyclePersistenceError::query_failure(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .map(|code| is_constraint_sqlstate(code.as_ref()))
            .unwrap_or(false),
        _ => false,
    }
}

fn is_constraint_sqlstate(code: &str) -> bool {
    code.starts_with("23") || code == "55000"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ALPHA_LIFECYCLE_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260408033000_alpha_lifecycle_actions.sql");

    fn sample_action_record() -> AlphaLifecycleActionRecord {
        AlphaLifecycleActionRecord {
            action_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            action_type: AlphaLifecycleActionType::Deallocate,
            action_status: AlphaLifecycleActionStatus::Applied,
            reason_code: AlphaLifecycleReasonCode::DeallocationThresholdBreached
                .code()
                .to_string(),
            trigger_evidence: json!({
                "criterion_keys": ["deallocation_threshold:rolling_drawdown"],
                "breach_id": "alpha::mean-reversion::rolling_drawdown::1712534400000000000"
            }),
            stop_research_criteria: None,
            remediation_guidance: None,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-lifecycle-001".to_string(),
            acted_at_utc: "2026-04-08T01:00:00Z".to_string(),
            approval_request_id: None,
            approval_reference: None,
        }
    }

    #[test]
    fn migration_creates_expected_alpha_lifecycle_schema_scope() {
        assert!(
            ALPHA_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS alpha_lifecycle_actions")
        );
        assert!(
            !ALPHA_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS promotion_decisions")
        );
        assert!(
            !ALPHA_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS alpha_health_metrics")
        );
    }

    #[test]
    fn migration_enforces_alpha_lifecycle_constraints_and_indexes() {
        assert!(
            ALPHA_LIFECYCLE_MIGRATION_SQL
                .contains("action_type IN ('deallocate', 'stop_research')")
        );
        assert!(
            ALPHA_LIFECYCLE_MIGRATION_SQL
                .contains("action_status IN ('applied', 'denied', 'unapplied')")
        );
        assert!(
            ALPHA_LIFECYCLE_MIGRATION_SQL
                .contains("jsonb_typeof(trigger_evidence_json) = 'object'")
        );
        assert!(ALPHA_LIFECYCLE_MIGRATION_SQL.contains("idx_alpha_lifecycle_actions_alpha_lookup"));
        assert!(ALPHA_LIFECYCLE_MIGRATION_SQL.contains("idx_alpha_lifecycle_actions_type_lookup"));
    }

    #[test]
    fn alpha_lifecycle_validation_canonicalizes_identifiers() {
        let mut record = sample_action_record();
        record.action_id = " Alpha::Mean-Reversion::1712534400000000000 ".to_string();
        record.alpha_id = " Alpha::Mean-Reversion ".to_string();
        record.reason_code = " Alpha_Lifecycle_Action_Deallocation_Threshold_Breached ".to_string();

        let canonical =
            validate_action_record_for_persistence(&record).expect("canonical action should pass");
        assert_eq!(
            canonical.action_id,
            "alpha::mean-reversion::1712534400000000000"
        );
        assert_eq!(canonical.alpha_id, "alpha::mean-reversion");
        assert_eq!(
            canonical.reason_code,
            AlphaLifecycleReasonCode::DeallocationThresholdBreached.code()
        );
    }

    #[test]
    fn alpha_lifecycle_unapplied_actions_require_remediation_guidance() {
        let mut record = sample_action_record();
        record.action_status = AlphaLifecycleActionStatus::Unapplied;
        record.reason_code = AlphaLifecycleReasonCode::AuthorizationFailed
            .code()
            .to_string();
        let error = validate_action_record_for_persistence(&record)
            .expect_err("unapplied actions require remediation guidance");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "remediation_guidance")
        );
    }

    #[test]
    fn list_query_orders_alpha_lifecycle_actions_deterministically() {
        assert!(
            LIST_ALPHA_LIFECYCLE_ACTIONS_BY_ALPHA_SQL
                .contains("ORDER BY acted_at_utc DESC, action_id ASC")
        );
    }

    #[test]
    fn alpha_lifecycle_list_limit_enforces_upper_and_lower_bounds() {
        let lower_error = validate_list_limit(0).expect_err("limit=0 must fail");
        assert_eq!(
            lower_error.code,
            AlphaLifecycleReasonCode::InvalidPayload.code()
        );
        assert!(
            lower_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );

        let upper_error =
            validate_list_limit(MAX_LIST_LIMIT + 1).expect_err("limit above max must fail");
        assert_eq!(
            upper_error.code,
            AlphaLifecycleReasonCode::InvalidPayload.code()
        );
        assert!(
            upper_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );
    }

    #[test]
    fn constraint_sqlstate_classifier_handles_expected_classes() {
        assert!(is_constraint_sqlstate("23505"));
        assert!(is_constraint_sqlstate("23514"));
        assert!(is_constraint_sqlstate("55000"));
        assert!(!is_constraint_sqlstate("22000"));
        assert!(!is_constraint_sqlstate("42P01"));
    }
}
