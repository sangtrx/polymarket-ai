use domain::research::{
    ValidationDiagnosticsPayload, ValidationWorkflowArtifactRecord,
    ValidationWorkflowContractError, ValidationWorkflowReasonCode, ValidationWorkflowStage,
    ValidationWorkflowStageOutcome, ValidationWorkflowValidationIssue,
    normalize_research_identifier, parse_validation_utc_timestamp,
    validate_validation_diagnostics_payload,
};
use serde_json::Value;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_VALIDATION_ARTIFACT_SQL: &str = r#"
    INSERT INTO validation_artifacts (
        artifact_id,
        run_id,
        candidate_id,
        stage,
        stage_index,
        stage_outcome,
        reason_code,
        diagnostics_json,
        actor_id,
        correlation_id,
        stage_started_at_utc,
        stage_completed_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12::timestamptz, NOW()
    )
    ON CONFLICT (artifact_id)
    DO UPDATE SET
        run_id = EXCLUDED.run_id,
        candidate_id = EXCLUDED.candidate_id,
        stage = EXCLUDED.stage,
        stage_index = EXCLUDED.stage_index,
        stage_outcome = EXCLUDED.stage_outcome,
        reason_code = EXCLUDED.reason_code,
        diagnostics_json = EXCLUDED.diagnostics_json,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        stage_started_at_utc = EXCLUDED.stage_started_at_utc,
        stage_completed_at_utc = EXCLUDED.stage_completed_at_utc,
        updated_at_utc = NOW()
"#;

const LOAD_VALIDATION_ARTIFACT_BY_STAGE_SQL: &str = r#"
    SELECT
        artifact_id,
        run_id,
        candidate_id,
        stage,
        stage_index,
        stage_outcome,
        reason_code,
        diagnostics_json,
        actor_id,
        correlation_id,
        to_char(stage_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stage_started_at_utc,
        to_char(stage_completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stage_completed_at_utc
    FROM validation_artifacts
    WHERE lower(trim(run_id)) = lower(trim($1))
      AND stage = $2
    ORDER BY stage_index ASC, stage_completed_at_utc DESC, artifact_id ASC
    LIMIT 1
"#;

const LOAD_VALIDATION_ARTIFACT_BY_ID_SQL: &str = r#"
    SELECT
        artifact_id,
        run_id,
        candidate_id,
        stage,
        stage_index,
        stage_outcome,
        reason_code,
        diagnostics_json,
        actor_id,
        correlation_id,
        to_char(stage_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stage_started_at_utc,
        to_char(stage_completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stage_completed_at_utc
    FROM validation_artifacts
    WHERE lower(trim(artifact_id)) = lower(trim($1))
    ORDER BY stage_index ASC, stage_completed_at_utc DESC, artifact_id ASC
    LIMIT 1
"#;

const LIST_VALIDATION_ARTIFACTS_BY_RUN_SQL: &str = r#"
    SELECT
        artifact_id,
        run_id,
        candidate_id,
        stage,
        stage_index,
        stage_outcome,
        reason_code,
        diagnostics_json,
        actor_id,
        correlation_id,
        to_char(stage_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stage_started_at_utc,
        to_char(stage_completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS stage_completed_at_utc
    FROM validation_artifacts
    WHERE lower(trim(run_id)) = lower(trim($1))
    ORDER BY stage_index ASC, stage_completed_at_utc ASC, artifact_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationArtifactPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationWorkflowValidationIssue>,
}

impl ValidationArtifactPersistenceError {
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
            code: "validation_artifact_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_artifact_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "validation_artifact_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: ValidationWorkflowContractError) -> Self {
        Self {
            code: "validation_artifact_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for ValidationArtifactPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ValidationArtifactPersistenceError {}

pub async fn upsert_validation_artifact<'e, E>(
    executor: E,
    artifact: &ValidationWorkflowArtifactRecord,
) -> Result<(), ValidationArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_artifact_for_persistence(artifact)?;
    let diagnostics_json = serde_json::to_value(&canonical.diagnostics).map_err(|error| {
        ValidationArtifactPersistenceError::invalid_payload(
            format!("diagnostics payload serialization failed: {error}"),
            Vec::new(),
        )
    })?;

    let result = sqlx::query(UPSERT_VALIDATION_ARTIFACT_SQL)
        .bind(&canonical.artifact_id)
        .bind(&canonical.run_id)
        .bind(&canonical.candidate_id)
        .bind(canonical.stage.as_str())
        .bind(canonical.stage_index)
        .bind(canonical.stage_outcome.as_str())
        .bind(&canonical.reason_code)
        .bind(diagnostics_json)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.stage_started_at_utc)
        .bind(&canonical.stage_completed_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_validation_artifact", error))?;

    if result.rows_affected() != 1 {
        return Err(ValidationArtifactPersistenceError::invalid_payload(
            format!(
                "upsert_validation_artifact expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_validation_artifact_by_stage<'e, E>(
    executor: E,
    run_id: &str,
    stage: ValidationWorkflowStage,
) -> Result<Option<ValidationWorkflowArtifactRecord>, ValidationArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("run_id", run_id)?;
    let normalized_run_id = normalize_research_identifier(run_id);

    let row = sqlx::query(LOAD_VALIDATION_ARTIFACT_BY_STAGE_SQL)
        .bind(&normalized_run_id)
        .bind(stage.as_str())
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            ValidationArtifactPersistenceError::query_failure(
                "load_validation_artifact_by_stage",
                error,
            )
        })?;

    row.map(decode_validation_artifact_row).transpose()
}

pub async fn load_validation_artifact_by_id<'e, E>(
    executor: E,
    artifact_id: &str,
) -> Result<Option<ValidationWorkflowArtifactRecord>, ValidationArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("artifact_id", artifact_id)?;
    let normalized_artifact_id = normalize_research_identifier(artifact_id);

    let row = sqlx::query(LOAD_VALIDATION_ARTIFACT_BY_ID_SQL)
        .bind(&normalized_artifact_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            ValidationArtifactPersistenceError::query_failure(
                "load_validation_artifact_by_id",
                error,
            )
        })?;

    row.map(decode_validation_artifact_row).transpose()
}

pub async fn list_validation_artifacts_by_run<'e, E>(
    executor: E,
    run_id: &str,
) -> Result<Vec<ValidationWorkflowArtifactRecord>, ValidationArtifactPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("run_id", run_id)?;
    let normalized_run_id = normalize_research_identifier(run_id);

    let rows = sqlx::query(LIST_VALIDATION_ARTIFACTS_BY_RUN_SQL)
        .bind(&normalized_run_id)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            ValidationArtifactPersistenceError::query_failure(
                "list_validation_artifacts_by_run",
                error,
            )
        })?;

    rows.into_iter()
        .map(decode_validation_artifact_row)
        .collect()
}

fn decode_validation_artifact_row(
    row: sqlx::postgres::PgRow,
) -> Result<ValidationWorkflowArtifactRecord, ValidationArtifactPersistenceError> {
    let stage_raw: String = row
        .try_get("stage")
        .map_err(|error| ValidationArtifactPersistenceError::row_decode_failure("stage", error))?;
    let stage = ValidationWorkflowStage::parse(&stage_raw).map_err(|error| {
        ValidationArtifactPersistenceError::row_contract_failure("stage", error)
    })?;
    let stage_outcome_raw: String = row.try_get("stage_outcome").map_err(|error| {
        ValidationArtifactPersistenceError::row_decode_failure("stage_outcome", error)
    })?;
    let stage_outcome =
        ValidationWorkflowStageOutcome::parse(&stage_outcome_raw).map_err(|error| {
            ValidationArtifactPersistenceError::row_contract_failure("stage_outcome", error)
        })?;
    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        ValidationArtifactPersistenceError::row_decode_failure("reason_code", error)
    })?;
    ValidationWorkflowReasonCode::parse(&reason_code).map_err(|error| {
        ValidationArtifactPersistenceError::row_contract_failure("reason_code", error)
    })?;

    let diagnostics_json: Value = row.try_get("diagnostics_json").map_err(|error| {
        ValidationArtifactPersistenceError::row_decode_failure("diagnostics_json", error)
    })?;
    let diagnostics: ValidationDiagnosticsPayload = serde_json::from_value(diagnostics_json)
        .map_err(|error| {
            ValidationArtifactPersistenceError::invalid_payload(
                format!("invalid persisted diagnostics payload: {error}"),
                Vec::new(),
            )
        })?;

    let artifact = ValidationWorkflowArtifactRecord {
        artifact_id: row.try_get("artifact_id").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("artifact_id", error)
        })?,
        run_id: row.try_get("run_id").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("run_id", error)
        })?,
        candidate_id: row.try_get("candidate_id").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("candidate_id", error)
        })?,
        stage,
        stage_index: row.try_get("stage_index").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("stage_index", error)
        })?,
        stage_outcome,
        reason_code,
        diagnostics,
        actor_id: row.try_get("actor_id").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        stage_started_at_utc: row.try_get("stage_started_at_utc").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("stage_started_at_utc", error)
        })?,
        stage_completed_at_utc: row.try_get("stage_completed_at_utc").map_err(|error| {
            ValidationArtifactPersistenceError::row_decode_failure("stage_completed_at_utc", error)
        })?,
    };

    validate_artifact_for_persistence(&artifact)
}

fn validate_artifact_for_persistence(
    artifact: &ValidationWorkflowArtifactRecord,
) -> Result<ValidationWorkflowArtifactRecord, ValidationArtifactPersistenceError> {
    let canonical_artifact_id = normalize_research_identifier(&artifact.artifact_id);
    let canonical_run_id = normalize_research_identifier(&artifact.run_id);
    let canonical_candidate_id = normalize_research_identifier(&artifact.candidate_id);
    let canonical_reason_code = normalize_research_identifier(&artifact.reason_code);
    validate_non_empty("artifact_id", &canonical_artifact_id)?;
    validate_non_empty("run_id", &canonical_run_id)?;
    validate_non_empty("candidate_id", &canonical_candidate_id)?;
    validate_non_empty("reason_code", &canonical_reason_code)?;
    validate_non_empty("actor_id", &artifact.actor_id)?;
    validate_non_empty("correlation_id", &artifact.correlation_id)?;

    if artifact.stage.stage_index() != artifact.stage_index {
        return Err(ValidationArtifactPersistenceError::invalid_payload(
            "stage_index must match the deterministic stage order index",
            vec![ValidationWorkflowValidationIssue {
                field: "stage_index".to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: format!(
                    "stage_index for `{}` must equal {}",
                    artifact.stage.as_str(),
                    artifact.stage.stage_index()
                ),
            }],
        ));
    }

    ValidationWorkflowReasonCode::parse(&canonical_reason_code).map_err(map_contract_error)?;
    validate_validation_diagnostics_payload(&artifact.diagnostics).map_err(map_contract_error)?;
    let stage_started_at = parse_validation_utc_timestamp(&artifact.stage_started_at_utc)
        .map_err(map_contract_error)?;
    let stage_completed_at = parse_validation_utc_timestamp(&artifact.stage_completed_at_utc)
        .map_err(map_contract_error)?;
    if stage_completed_at < stage_started_at {
        return Err(ValidationArtifactPersistenceError::invalid_payload(
            "stage_completed_at_utc must be greater than or equal to stage_started_at_utc",
            vec![ValidationWorkflowValidationIssue {
                field: "stage_completed_at_utc".to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: "stage_completed_at_utc must be >= stage_started_at_utc".to_string(),
            }],
        ));
    }

    Ok(ValidationWorkflowArtifactRecord {
        artifact_id: canonical_artifact_id,
        run_id: canonical_run_id,
        candidate_id: canonical_candidate_id,
        stage: artifact.stage,
        stage_index: artifact.stage_index,
        stage_outcome: artifact.stage_outcome,
        reason_code: canonical_reason_code,
        diagnostics: artifact.diagnostics.clone(),
        actor_id: artifact.actor_id.trim().to_string(),
        correlation_id: artifact.correlation_id.trim().to_string(),
        stage_started_at_utc: artifact.stage_started_at_utc.trim().to_string(),
        stage_completed_at_utc: artifact.stage_completed_at_utc.trim().to_string(),
    })
}

fn map_contract_error(
    error: ValidationWorkflowContractError,
) -> ValidationArtifactPersistenceError {
    ValidationArtifactPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ValidationArtifactPersistenceError> {
    if value.trim().is_empty() {
        return Err(ValidationArtifactPersistenceError::invalid_payload(
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
) -> ValidationArtifactPersistenceError {
    if is_constraint_error(&error) {
        return ValidationArtifactPersistenceError::constraint_violation(operation, error);
    }
    ValidationArtifactPersistenceError::query_failure(operation, error)
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

    const VALIDATION_WORKFLOW_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407193000_validation_runs_validation_artifacts.sql");

    fn sample_artifact() -> ValidationWorkflowArtifactRecord {
        ValidationWorkflowArtifactRecord {
            artifact_id: "candidate::alpha-1::1712448000::quality".to_string(),
            run_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            stage: ValidationWorkflowStage::Quality,
            stage_index: ValidationWorkflowStage::Quality.stage_index(),
            stage_outcome: ValidationWorkflowStageOutcome::Passed,
            reason_code: ValidationWorkflowReasonCode::RunStarted.code().to_string(),
            diagnostics: ValidationDiagnosticsPayload {
                out_of_sample_sharpe: 1.42,
                max_drawdown: -0.18,
                brier_score: Some(0.11),
                expected_calibration_error: None,
                overfit_indicator: 0.23,
                overfit_flag: false,
            },
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-validation-artifact-001".to_string(),
            stage_started_at_utc: "2026-04-07T00:00:10Z".to_string(),
            stage_completed_at_utc: "2026-04-07T00:00:40Z".to_string(),
        }
    }

    #[test]
    fn migration_enforces_artifact_constraints_and_indexes() {
        assert!(VALIDATION_WORKFLOW_MIGRATION_SQL.contains(
            "stage IN ('quality', 'labeling', 'purged_cv', 'cpcv', 'overfit_diagnostics')"
        ));
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL
                .contains("stage_outcome IN ('passed', 'failed', 'blocked')")
        );
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL.contains("idx_validation_artifacts_run_stage_unique")
        );
        assert!(
            VALIDATION_WORKFLOW_MIGRATION_SQL.contains("diagnostics_json ? 'out_of_sample_sharpe'")
        );
    }

    #[test]
    fn artifact_validation_rejects_stage_index_mismatch() {
        let mut artifact = sample_artifact();
        artifact.stage_index = ValidationWorkflowStage::Labeling.stage_index();

        let error = validate_artifact_for_persistence(&artifact)
            .expect_err("stage index mismatch must fail");
        assert_eq!(
            error.code,
            ValidationWorkflowReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "stage_index")
        );
    }

    #[test]
    fn artifact_validation_rejects_invalid_diagnostics_payload() {
        let mut artifact = sample_artifact();
        artifact.diagnostics.brier_score = None;
        artifact.diagnostics.expected_calibration_error = None;

        let error = validate_artifact_for_persistence(&artifact)
            .expect_err("diagnostics payload must include calibration metric");
        assert_eq!(
            error.code,
            ValidationWorkflowReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "calibration_metric")
        );
    }

    #[test]
    fn list_query_orders_artifacts_deterministically() {
        assert!(
            LIST_VALIDATION_ARTIFACTS_BY_RUN_SQL
                .contains("ORDER BY stage_index ASC, stage_completed_at_utc ASC, artifact_id ASC")
        );
    }
}
