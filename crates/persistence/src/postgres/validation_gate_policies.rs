use domain::research::{
    ValidationGateContractError, ValidationGatePolicyDefinition, ValidationGateReasonCode,
    ValidationGateStageScope, ValidationGateThreshold, ValidationGateType,
    ValidationGateValidationIssue, canonicalize_validation_gate_policy_definition,
    normalize_research_identifier, parse_utc_timestamp,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_VALIDATION_GATE_POLICY_SQL: &str = r#"
    INSERT INTO validation_gate_policies (
        policy_key,
        gate_type,
        stage_scope,
        metric_key,
        comparator,
        threshold_value,
        mandatory,
        diagnostics_json,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz
    )
    ON CONFLICT (policy_key)
    DO UPDATE SET
        gate_type = EXCLUDED.gate_type,
        stage_scope = EXCLUDED.stage_scope,
        metric_key = EXCLUDED.metric_key,
        comparator = EXCLUDED.comparator,
        threshold_value = EXCLUDED.threshold_value,
        mandatory = EXCLUDED.mandatory,
        diagnostics_json = EXCLUDED.diagnostics_json,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_VALIDATION_GATE_POLICY_SQL: &str = r#"
    SELECT
        policy_key,
        gate_type,
        stage_scope,
        metric_key,
        comparator,
        threshold_value,
        mandatory,
        diagnostics_json,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM validation_gate_policies
    WHERE lower(trim(policy_key)) = lower(trim($1))
    ORDER BY updated_at_utc DESC, policy_key ASC
    LIMIT 1
"#;

const LIST_VALIDATION_GATE_POLICIES_SQL: &str = r#"
    SELECT
        policy_key,
        gate_type,
        stage_scope,
        metric_key,
        comparator,
        threshold_value,
        mandatory,
        diagnostics_json,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM validation_gate_policies
    ORDER BY stage_scope ASC, gate_type ASC, policy_key ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationGatePolicyPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationGateValidationIssue>,
}

impl ValidationGatePolicyPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ValidationGateValidationIssue>,
    ) -> Self {
        Self {
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_gate_policy_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_gate_policy_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_gate_policy_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: ValidationGateContractError) -> Self {
        Self {
            code: "validation_gate_policy_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for ValidationGatePolicyPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ValidationGatePolicyPersistenceError {}

pub async fn upsert_validation_gate_policy<'e, E>(
    executor: E,
    policy: &ValidationGatePolicyDefinition,
) -> Result<(), ValidationGatePolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_policy_for_persistence(policy)?;

    let result = sqlx::query(UPSERT_VALIDATION_GATE_POLICY_SQL)
        .bind(&canonical.policy_key)
        .bind(canonical.gate_type.as_str())
        .bind(canonical.stage_scope.as_str())
        .bind(&canonical.metric_key)
        .bind(canonical.threshold.comparator.as_str())
        .bind(canonical.threshold.value)
        .bind(canonical.mandatory)
        .bind(&canonical.diagnostics)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.updated_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_validation_gate_policy", error))?;

    if result.rows_affected() != 1 {
        return Err(ValidationGatePolicyPersistenceError::invalid_payload(
            format!(
                "upsert_validation_gate_policy expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_validation_gate_policy<'e, E>(
    executor: E,
    policy_key: &str,
) -> Result<Option<ValidationGatePolicyDefinition>, ValidationGatePolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("policy_key", policy_key)?;
    let normalized_policy_key = normalize_research_identifier(policy_key);

    let row = sqlx::query(LOAD_VALIDATION_GATE_POLICY_SQL)
        .bind(&normalized_policy_key)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            ValidationGatePolicyPersistenceError::query_failure(
                "load_validation_gate_policy",
                error,
            )
        })?;

    row.map(decode_validation_gate_policy_row).transpose()
}

pub async fn list_validation_gate_policies<'e, E>(
    executor: E,
) -> Result<Vec<ValidationGatePolicyDefinition>, ValidationGatePolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    let rows = sqlx::query(LIST_VALIDATION_GATE_POLICIES_SQL)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            ValidationGatePolicyPersistenceError::query_failure(
                "list_validation_gate_policies",
                error,
            )
        })?;

    rows.into_iter()
        .map(decode_validation_gate_policy_row)
        .collect()
}

fn decode_validation_gate_policy_row(
    row: sqlx::postgres::PgRow,
) -> Result<ValidationGatePolicyDefinition, ValidationGatePolicyPersistenceError> {
    let gate_type_raw: String = row.try_get("gate_type").map_err(|error| {
        ValidationGatePolicyPersistenceError::row_decode_failure("gate_type", error)
    })?;
    let stage_scope_raw: String = row.try_get("stage_scope").map_err(|error| {
        ValidationGatePolicyPersistenceError::row_decode_failure("stage_scope", error)
    })?;
    let comparator_raw: String = row.try_get("comparator").map_err(|error| {
        ValidationGatePolicyPersistenceError::row_decode_failure("comparator", error)
    })?;

    let gate_type = ValidationGateType::parse(&gate_type_raw).map_err(|error| {
        ValidationGatePolicyPersistenceError::row_contract_failure("gate_type", error)
    })?;
    let stage_scope = ValidationGateStageScope::parse(&stage_scope_raw).map_err(|error| {
        ValidationGatePolicyPersistenceError::row_contract_failure("stage_scope", error)
    })?;
    let comparator =
        domain::research::ValidationGateComparator::parse(&comparator_raw).map_err(|error| {
            ValidationGatePolicyPersistenceError::row_contract_failure("comparator", error)
        })?;

    let policy = ValidationGatePolicyDefinition {
        policy_key: row.try_get("policy_key").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("policy_key", error)
        })?,
        gate_type,
        stage_scope,
        metric_key: row.try_get("metric_key").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("metric_key", error)
        })?,
        threshold: ValidationGateThreshold {
            comparator,
            value: row.try_get("threshold_value").map_err(|error| {
                ValidationGatePolicyPersistenceError::row_decode_failure("threshold_value", error)
            })?,
        },
        mandatory: row.try_get("mandatory").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("mandatory", error)
        })?,
        diagnostics: row.try_get("diagnostics_json").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("diagnostics_json", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            ValidationGatePolicyPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_policy_for_persistence(&policy)
}

fn validate_policy_for_persistence(
    policy: &ValidationGatePolicyDefinition,
) -> Result<ValidationGatePolicyDefinition, ValidationGatePolicyPersistenceError> {
    let canonical =
        canonicalize_validation_gate_policy_definition(policy).map_err(map_contract_error)?;
    parse_utc_timestamp(&canonical.updated_at_utc).map_err(|error| {
        ValidationGatePolicyPersistenceError::invalid_payload(
            error.message,
            vec![ValidationGateValidationIssue {
                field: "updated_at_utc".to_string(),
                code: ValidationGateReasonCode::InvalidPayload.code(),
                message: "updated_at_utc must be RFC3339 UTC".to_string(),
            }],
        )
    })?;
    Ok(canonical)
}

fn map_contract_error(error: ValidationGateContractError) -> ValidationGatePolicyPersistenceError {
    ValidationGatePolicyPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(
    field: &str,
    value: &str,
) -> Result<(), ValidationGatePolicyPersistenceError> {
    if value.trim().is_empty() {
        return Err(ValidationGatePolicyPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![ValidationGateValidationIssue {
                field: field.to_string(),
                code: ValidationGateReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ValidationGatePolicyPersistenceError {
    if is_constraint_error(&error) {
        return ValidationGatePolicyPersistenceError::constraint_violation(operation, error);
    }
    ValidationGatePolicyPersistenceError::query_failure(operation, error)
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
    use serde_json::json;

    const VALIDATION_GATE_POLICIES_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407173000_validation_gate_policies.sql");

    fn sample_policy() -> ValidationGatePolicyDefinition {
        ValidationGatePolicyDefinition {
            policy_key: "fr43::forward-bias::primary".to_string(),
            gate_type: ValidationGateType::ForwardBias,
            stage_scope: ValidationGateStageScope::TrainingAndPromotion,
            metric_key: "forward_bias_score".to_string(),
            threshold: ValidationGateThreshold {
                comparator: domain::research::ValidationGateComparator::Lte,
                value: 0.12,
            },
            mandatory: true,
            diagnostics: json!({
                "failure_reason": "forward_bias_above_limit",
                "operator_action": "review feature windows"
            }),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-validation-gate-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_validation_gate_schema_scope() {
        assert!(
            VALIDATION_GATE_POLICIES_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS validation_gate_policies")
        );
        assert!(
            !VALIDATION_GATE_POLICIES_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS alpha_hypotheses")
        );
    }

    #[test]
    fn migration_enforces_constraints_and_indexes() {
        assert!(VALIDATION_GATE_POLICIES_MIGRATION_SQL.contains("gate_type IN"));
        assert!(VALIDATION_GATE_POLICIES_MIGRATION_SQL.contains("'forward_bias'"));
        assert!(VALIDATION_GATE_POLICIES_MIGRATION_SQL.contains("'data_leakage'"));
        assert!(VALIDATION_GATE_POLICIES_MIGRATION_SQL.contains("'regime_survivability'"));
        assert!(VALIDATION_GATE_POLICIES_MIGRATION_SQL.contains("'data_quality'"));
        assert!(VALIDATION_GATE_POLICIES_MIGRATION_SQL.contains("mandatory = TRUE"));
        assert!(
            VALIDATION_GATE_POLICIES_MIGRATION_SQL
                .contains("stage_scope IN ('training', 'promotion', 'training_and_promotion')")
        );
        assert!(
            VALIDATION_GATE_POLICIES_MIGRATION_SQL
                .contains("idx_validation_gate_policies_policy_key_canonical_unique")
        );
        assert!(
            VALIDATION_GATE_POLICIES_MIGRATION_SQL
                .contains("idx_validation_gate_policies_stage_lookup")
        );
    }

    #[test]
    fn policy_validation_canonicalizes_identifiers() {
        let mut policy = sample_policy();
        policy.policy_key = " FR43::Forward-Bias::Primary ".to_string();
        policy.metric_key = " Forward_Bias_Score ".to_string();

        let canonical =
            validate_policy_for_persistence(&policy).expect("canonical policy should pass");
        assert_eq!(canonical.policy_key, "fr43::forward-bias::primary");
        assert_eq!(canonical.metric_key, "forward_bias_score");
    }

    #[test]
    fn policy_validation_rejects_non_mandatory_fr43_gate() {
        let mut policy = sample_policy();
        policy.mandatory = false;

        let error =
            validate_policy_for_persistence(&policy).expect_err("mandatory FR43 gates must fail");
        assert_eq!(error.code, ValidationGateReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "mandatory")
        );
    }

    #[test]
    fn policy_validation_rejects_non_mandatory_data_quality_gate() {
        let mut policy = sample_policy();
        policy.gate_type = ValidationGateType::DataQuality;
        policy.policy_key = "fr43::data-quality::training-core".to_string();
        policy.metric_key = "data_quality_score".to_string();
        policy.mandatory = false;

        let error = validate_policy_for_persistence(&policy)
            .expect_err("data_quality gate policies must remain mandatory");
        assert_eq!(error.code, ValidationGateReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "mandatory")
        );
    }

    #[test]
    fn list_query_orders_policies_deterministically() {
        assert!(
            LIST_VALIDATION_GATE_POLICIES_SQL
                .contains("ORDER BY stage_scope ASC, gate_type ASC, policy_key ASC")
        );
    }

    #[test]
    fn row_contract_failures_are_classified_as_row_decode_errors() {
        let error = ValidationGatePolicyPersistenceError::row_contract_failure(
            "gate_type",
            ValidationGateContractError::invalid_payload("unknown gate type"),
        );
        assert_eq!(error.code, "validation_gate_policy_row_decode_failed");
    }
}
