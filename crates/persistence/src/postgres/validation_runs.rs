use domain::research::{
    ValidationWorkflowContractError, ValidationWorkflowReasonCode, ValidationWorkflowRunRecord,
    ValidationWorkflowRunState, ValidationWorkflowValidationIssue, normalize_research_identifier,
    parse_validation_utc_timestamp,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_VALIDATION_RUN_SQL: &str = r#"
    INSERT INTO validation_runs (
        run_id,
        candidate_id,
        run_state,
        reason_code,
        gate_evaluation_json,
        comparison_ready,
        actor_id,
        correlation_id,
        started_at_utc,
        completed_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz, $10::timestamptz, NOW()
    )
    ON CONFLICT (run_id)
    DO UPDATE SET
        run_state = EXCLUDED.run_state,
        reason_code = EXCLUDED.reason_code,
        gate_evaluation_json = EXCLUDED.gate_evaluation_json,
        comparison_ready = EXCLUDED.comparison_ready,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        started_at_utc = EXCLUDED.started_at_utc,
        completed_at_utc = EXCLUDED.completed_at_utc,
        updated_at_utc = NOW()
"#;

const LOAD_VALIDATION_RUN_SQL: &str = r#"
    SELECT
        run_id,
        candidate_id,
        run_state,
        reason_code,
        gate_evaluation_json,
        comparison_ready,
        actor_id,
        correlation_id,
        to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS started_at_utc,
        CASE
            WHEN completed_at_utc IS NULL THEN NULL
            ELSE to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS completed_at_utc
    FROM validation_runs
    WHERE lower(trim(run_id)) = lower(trim($1))
    ORDER BY started_at_utc DESC, run_id ASC
    LIMIT 1
"#;

const LIST_VALIDATION_RUNS_BY_CANDIDATE_SQL: &str = r#"
    SELECT
        run_id,
        candidate_id,
        run_state,
        reason_code,
        gate_evaluation_json,
        comparison_ready,
        actor_id,
        correlation_id,
        to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS started_at_utc,
        CASE
            WHEN completed_at_utc IS NULL THEN NULL
            ELSE to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS completed_at_utc
    FROM validation_runs
    WHERE lower(trim(candidate_id)) = lower(trim($1))
    ORDER BY started_at_utc DESC, run_id ASC
    LIMIT $2
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationRunPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationWorkflowValidationIssue>,
}

impl ValidationRunPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ValidationWorkflowValidationIssue>,
    ) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_run_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_run_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_run_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: ValidationWorkflowContractError) -> Self {
        Self {
            code: "validation_run_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for ValidationRunPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ValidationRunPersistenceError {}

pub async fn upsert_validation_run<'e, E>(
    executor: E,
    run: &ValidationWorkflowRunRecord,
) -> Result<(), ValidationRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_run_for_persistence(run)?;

    let result = sqlx::query(UPSERT_VALIDATION_RUN_SQL)
        .bind(&canonical.run_id)
        .bind(&canonical.candidate_id)
        .bind(canonical.run_state.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.gate_evaluation)
        .bind(canonical.comparison_ready)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.started_at_utc)
        .bind(&canonical.completed_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_validation_run", error))?;

    if result.rows_affected() != 1 {
        return Err(ValidationRunPersistenceError::invalid_payload(
            format!(
                "upsert_validation_run expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_validation_run<'e, E>(
    executor: E,
    run_id: &str,
) -> Result<Option<ValidationWorkflowRunRecord>, ValidationRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("run_id", run_id)?;
    let normalized_run_id = normalize_research_identifier(run_id);

    let row = sqlx::query(LOAD_VALIDATION_RUN_SQL)
        .bind(&normalized_run_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            ValidationRunPersistenceError::query_failure("load_validation_run", error)
        })?;

    row.map(decode_validation_run_row).transpose()
}

pub async fn list_validation_runs_by_candidate<'e, E>(
    executor: E,
    candidate_id: &str,
    limit: i64,
) -> Result<Vec<ValidationWorkflowRunRecord>, ValidationRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("candidate_id", candidate_id)?;
    if limit <= 0 {
        return Err(ValidationRunPersistenceError::invalid_payload(
            "limit must be greater than 0",
            vec![ValidationWorkflowValidationIssue {
                field: "limit".to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let normalized_candidate_id = normalize_research_identifier(candidate_id);

    let rows = sqlx::query(LIST_VALIDATION_RUNS_BY_CANDIDATE_SQL)
        .bind(&normalized_candidate_id)
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            ValidationRunPersistenceError::query_failure("list_validation_runs_by_candidate", error)
        })?;

    rows.into_iter().map(decode_validation_run_row).collect()
}

fn decode_validation_run_row(
    row: sqlx::postgres::PgRow,
) -> Result<ValidationWorkflowRunRecord, ValidationRunPersistenceError> {
    let run_state_raw: String = row
        .try_get("run_state")
        .map_err(|error| ValidationRunPersistenceError::row_decode_failure("run_state", error))?;
    let run_state = ValidationWorkflowRunState::parse(&run_state_raw)
        .map_err(|error| ValidationRunPersistenceError::row_contract_failure("run_state", error))?;

    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| ValidationRunPersistenceError::row_decode_failure("reason_code", error))?;
    ValidationWorkflowReasonCode::parse(&reason_code).map_err(|error| {
        ValidationRunPersistenceError::row_contract_failure("reason_code", error)
    })?;

    let run = ValidationWorkflowRunRecord {
        run_id: row
            .try_get("run_id")
            .map_err(|error| ValidationRunPersistenceError::row_decode_failure("run_id", error))?,
        candidate_id: row.try_get("candidate_id").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("candidate_id", error)
        })?,
        run_state,
        reason_code,
        gate_evaluation: row.try_get("gate_evaluation_json").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("gate_evaluation_json", error)
        })?,
        comparison_ready: row.try_get("comparison_ready").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("comparison_ready", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        started_at_utc: row.try_get("started_at_utc").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("started_at_utc", error)
        })?,
        completed_at_utc: row.try_get("completed_at_utc").map_err(|error| {
            ValidationRunPersistenceError::row_decode_failure("completed_at_utc", error)
        })?,
    };

    validate_run_for_persistence(&run)
}

fn validate_run_for_persistence(
    run: &ValidationWorkflowRunRecord,
) -> Result<ValidationWorkflowRunRecord, ValidationRunPersistenceError> {
    let canonical_run_id = normalize_research_identifier(&run.run_id);
    let canonical_candidate_id = normalize_research_identifier(&run.candidate_id);
    let canonical_reason_code = normalize_research_identifier(&run.reason_code);
    validate_non_empty("run_id", &canonical_run_id)?;
    validate_non_empty("candidate_id", &canonical_candidate_id)?;
    validate_non_empty("reason_code", &canonical_reason_code)?;
    validate_non_empty("actor_id", &run.actor_id)?;
    validate_non_empty("correlation_id", &run.correlation_id)?;
    ValidationWorkflowReasonCode::parse(&canonical_reason_code).map_err(map_contract_error)?;
    parse_validation_utc_timestamp(&run.started_at_utc).map_err(map_contract_error)?;
    if let Some(completed_at_utc) = run.completed_at_utc.as_deref() {
        let started =
            parse_validation_utc_timestamp(&run.started_at_utc).map_err(map_contract_error)?;
        let completed =
            parse_validation_utc_timestamp(completed_at_utc).map_err(map_contract_error)?;
        if completed < started {
            return Err(ValidationRunPersistenceError::invalid_payload(
                "completed_at_utc must be greater than or equal to started_at_utc",
                vec![ValidationWorkflowValidationIssue {
                    field: "completed_at_utc".to_string(),
                    code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                    message: "completed_at_utc must be >= started_at_utc".to_string(),
                }],
            ));
        }
    }
    if !run.gate_evaluation.is_object() {
        return Err(ValidationRunPersistenceError::invalid_payload(
            "gate_evaluation must be a JSON object",
            vec![ValidationWorkflowValidationIssue {
                field: "gate_evaluation".to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: "gate_evaluation must be a JSON object".to_string(),
            }],
        ));
    }

    Ok(ValidationWorkflowRunRecord {
        run_id: canonical_run_id,
        candidate_id: canonical_candidate_id,
        run_state: run.run_state,
        reason_code: canonical_reason_code,
        gate_evaluation: run.gate_evaluation.clone(),
        comparison_ready: run.comparison_ready,
        actor_id: run.actor_id.trim().to_string(),
        correlation_id: run.correlation_id.trim().to_string(),
        started_at_utc: run.started_at_utc.trim().to_string(),
        completed_at_utc: run
            .completed_at_utc
            .as_ref()
            .map(|value| value.trim().to_string()),
    })
}

fn map_contract_error(error: ValidationWorkflowContractError) -> ValidationRunPersistenceError {
    ValidationRunPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ValidationRunPersistenceError> {
    if value.trim().is_empty() {
        return Err(ValidationRunPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![ValidationWorkflowValidationIssue {
                field: field.to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ValidationRunPersistenceError {
    if is_constraint_error(&error) {
        return ValidationRunPersistenceError::constraint_violation(operation, error);
    }
    ValidationRunPersistenceError::query_failure(operation, error)
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

    const VALIDATION_WORKFLOW_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407193000_validation_runs_validation_artifacts.sql");

    fn sample_run_record() -> ValidationWorkflowRunRecord {
        ValidationWorkflowRunRecord {
            run_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            run_state: ValidationWorkflowRunState::Completed,
            reason_code: ValidationWorkflowReasonCode::RunStarted.code().to_string(),
            gate_evaluation: json!({
                "outcome": "allow",
                "reason_code": "validation_gate_evaluation_allowed"
            }),
            comparison_ready: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-validation-run-001".to_string(),
            started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:01:00Z".to_string()),
        }
    }

    #[test]
    fn migration_creates_expected_validation_run_schema_scope() {
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS validation_runs")
        );
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS validation_artifacts")
        );
        assert!(
            !VALIDATION_WORKFLOW_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS validation_gate_policies")
        );
    }

    #[test]
    fn migration_enforces_run_constraints_and_indexes() {
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL
                .contains("run_state IN ('running', 'completed', 'blocked', 'failed')")
        );
        assert!(VALIDATION_WORKFLOW_MIGRATION_SQL.contains("idx_validation_runs_candidate_lookup"));
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL
                .contains("idx_validation_runs_run_id_canonical_unique")
        );
    }

    #[test]
    fn run_validation_canonicalizes_identifiers() {
        let mut run = sample_run_record();
        run.run_id = " Candidate::Alpha-1::1712448000 ".to_string();
        run.candidate_id = " Candidate::Alpha-1 ".to_string();
        run.reason_code = " Validation_Run_Started ".to_string();

        let canonical =
            validate_run_for_persistence(&run).expect("canonical validation run should pass");
        assert_eq!(canonical.run_id, "candidate::alpha-1::1712448000");
        assert_eq!(canonical.candidate_id, "candidate::alpha-1");
        assert_eq!(canonical.reason_code, "validation_run_started");
    }

    #[test]
    fn run_validation_rejects_non_object_gate_evaluation() {
        let mut run = sample_run_record();
        run.gate_evaluation = json!(["allow"]);

        let error =
            validate_run_for_persistence(&run).expect_err("gate_evaluation must be a json object");
        assert_eq!(
            error.code,
            ValidationWorkflowReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "gate_evaluation")
        );
    }

    #[test]
    fn list_query_orders_runs_deterministically() {
        assert!(
            LIST_VALIDATION_RUNS_BY_CANDIDATE_SQL
                .contains("ORDER BY started_at_utc DESC, run_id ASC")
        );
    }
}
