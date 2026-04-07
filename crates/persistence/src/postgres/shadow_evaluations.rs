use domain::research::{
    ShadowEvaluationContractError, ShadowEvaluationReasonCode, ShadowEvaluationRecord,
    ShadowEvaluationState, ShadowEvaluationValidationIssue, ShadowSimulationOutcome,
    canonicalize_shadow_evaluation_record, normalize_research_identifier,
    parse_shadow_utc_timestamp,
};
use serde_json::{Value, json};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_SHADOW_EVALUATION_SQL: &str = r#"
    INSERT INTO shadow_evaluations (
        evaluation_id,
        candidate_id,
        validation_run_id,
        evaluation_state,
        reason_code,
        market_context_json,
        signal_decisions_json,
        simulation_outcomes_json,
        actor_id,
        correlation_id,
        started_at_utc,
        completed_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12::timestamptz, NOW()
    )
    ON CONFLICT (evaluation_id)
    DO UPDATE SET
        candidate_id = EXCLUDED.candidate_id,
        validation_run_id = EXCLUDED.validation_run_id,
        evaluation_state = EXCLUDED.evaluation_state,
        reason_code = EXCLUDED.reason_code,
        market_context_json = EXCLUDED.market_context_json,
        signal_decisions_json = EXCLUDED.signal_decisions_json,
        simulation_outcomes_json = EXCLUDED.simulation_outcomes_json,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        started_at_utc = EXCLUDED.started_at_utc,
        completed_at_utc = EXCLUDED.completed_at_utc,
        updated_at_utc = NOW()
"#;

const LOAD_SHADOW_EVALUATION_SQL: &str = r#"
    SELECT
        evaluation_id,
        candidate_id,
        validation_run_id,
        evaluation_state,
        reason_code,
        market_context_json,
        signal_decisions_json,
        simulation_outcomes_json,
        actor_id,
        correlation_id,
        to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS started_at_utc,
        CASE
            WHEN completed_at_utc IS NULL THEN NULL
            ELSE to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS completed_at_utc
    FROM shadow_evaluations
    WHERE lower(trim(evaluation_id)) = lower(trim($1))
    ORDER BY started_at_utc DESC, evaluation_id ASC
    LIMIT 1
"#;

const LOAD_LATEST_SHADOW_EVALUATION_BY_CANDIDATE_AND_RUN_SQL: &str = r#"
    SELECT
        evaluation_id,
        candidate_id,
        validation_run_id,
        evaluation_state,
        reason_code,
        market_context_json,
        signal_decisions_json,
        simulation_outcomes_json,
        actor_id,
        correlation_id,
        to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS started_at_utc,
        CASE
            WHEN completed_at_utc IS NULL THEN NULL
            ELSE to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS completed_at_utc
    FROM shadow_evaluations
    WHERE lower(trim(candidate_id)) = lower(trim($1))
      AND lower(trim(validation_run_id)) = lower(trim($2))
    ORDER BY started_at_utc DESC, evaluation_id ASC
    LIMIT 1
"#;

const LIST_SHADOW_EVALUATIONS_BY_CANDIDATE_SQL: &str = r#"
    SELECT
        evaluation_id,
        candidate_id,
        validation_run_id,
        evaluation_state,
        reason_code,
        market_context_json,
        signal_decisions_json,
        simulation_outcomes_json,
        actor_id,
        correlation_id,
        to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS started_at_utc,
        CASE
            WHEN completed_at_utc IS NULL THEN NULL
            ELSE to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS completed_at_utc
    FROM shadow_evaluations
    WHERE lower(trim(candidate_id)) = lower(trim($1))
      AND ($2::timestamptz IS NULL OR started_at_utc >= $2::timestamptz)
      AND ($3::timestamptz IS NULL OR started_at_utc < $3::timestamptz)
    ORDER BY started_at_utc DESC, evaluation_id ASC
    LIMIT $4
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowEvaluationPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ShadowEvaluationValidationIssue>,
}

impl ShadowEvaluationPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ShadowEvaluationValidationIssue>,
    ) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "shadow_evaluation_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "shadow_evaluation_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "shadow_evaluation_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: ShadowEvaluationContractError) -> Self {
        Self {
            code: "shadow_evaluation_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for ShadowEvaluationPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ShadowEvaluationPersistenceError {}

pub async fn upsert_shadow_evaluation<'e, E>(
    executor: E,
    record: &ShadowEvaluationRecord,
) -> Result<(), ShadowEvaluationPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_record_for_persistence(record)?;
    let simulation_outcomes_json = json!({
        "outcomes": canonical.simulation_outcomes
    });

    let result = sqlx::query(UPSERT_SHADOW_EVALUATION_SQL)
        .bind(&canonical.evaluation_id)
        .bind(&canonical.candidate_id)
        .bind(&canonical.validation_run_id)
        .bind(canonical.evaluation_state.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.market_context)
        .bind(&canonical.signal_decisions)
        .bind(simulation_outcomes_json)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.started_at_utc)
        .bind(&canonical.completed_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_shadow_evaluation", error))?;

    if result.rows_affected() != 1 {
        return Err(ShadowEvaluationPersistenceError::invalid_payload(
            format!(
                "upsert_shadow_evaluation expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_shadow_evaluation<'e, E>(
    executor: E,
    evaluation_id: &str,
) -> Result<Option<ShadowEvaluationRecord>, ShadowEvaluationPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("evaluation_id", evaluation_id)?;
    let normalized_evaluation_id = normalize_research_identifier(evaluation_id);

    let row = sqlx::query(LOAD_SHADOW_EVALUATION_SQL)
        .bind(&normalized_evaluation_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            ShadowEvaluationPersistenceError::query_failure("load_shadow_evaluation", error)
        })?;

    row.map(decode_shadow_evaluation_row).transpose()
}

pub async fn list_shadow_evaluations_by_candidate<'e, E>(
    executor: E,
    candidate_id: &str,
    started_after_utc: Option<&str>,
    started_before_utc: Option<&str>,
    limit: i64,
) -> Result<Vec<ShadowEvaluationRecord>, ShadowEvaluationPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("candidate_id", candidate_id)?;
    if limit <= 0 {
        return Err(ShadowEvaluationPersistenceError::invalid_payload(
            "limit must be greater than 0",
            vec![ShadowEvaluationValidationIssue {
                field: "limit".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    let normalized_started_after =
        normalize_optional_timestamp("started_after_utc", started_after_utc)?;
    let normalized_started_before =
        normalize_optional_timestamp("started_before_utc", started_before_utc)?;

    if let (Some(started_after), Some(started_before)) = (
        normalized_started_after.as_deref(),
        normalized_started_before.as_deref(),
    ) {
        let started_after_ts =
            parse_shadow_utc_timestamp(started_after).map_err(map_contract_error)?;
        let started_before_ts =
            parse_shadow_utc_timestamp(started_before).map_err(map_contract_error)?;
        if started_before_ts <= started_after_ts {
            return Err(ShadowEvaluationPersistenceError::invalid_payload(
                "started_before_utc must be greater than started_after_utc",
                vec![ShadowEvaluationValidationIssue {
                    field: "started_before_utc".to_string(),
                    code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                    message: "started_before_utc must be greater than started_after_utc"
                        .to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LIST_SHADOW_EVALUATIONS_BY_CANDIDATE_SQL)
        .bind(&normalized_candidate_id)
        .bind(normalized_started_after.as_deref())
        .bind(normalized_started_before.as_deref())
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            ShadowEvaluationPersistenceError::query_failure(
                "list_shadow_evaluations_by_candidate",
                error,
            )
        })?;

    rows.into_iter().map(decode_shadow_evaluation_row).collect()
}

pub async fn load_latest_shadow_evaluation_by_candidate_and_run<'e, E>(
    executor: E,
    candidate_id: &str,
    validation_run_id: &str,
) -> Result<Option<ShadowEvaluationRecord>, ShadowEvaluationPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("candidate_id", candidate_id)?;
    validate_non_empty("validation_run_id", validation_run_id)?;
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    let normalized_validation_run_id = normalize_research_identifier(validation_run_id);

    let row = sqlx::query(LOAD_LATEST_SHADOW_EVALUATION_BY_CANDIDATE_AND_RUN_SQL)
        .bind(&normalized_candidate_id)
        .bind(&normalized_validation_run_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            ShadowEvaluationPersistenceError::query_failure(
                "load_latest_shadow_evaluation_by_candidate_and_run",
                error,
            )
        })?;

    row.map(decode_shadow_evaluation_row).transpose()
}

fn decode_shadow_evaluation_row(
    row: sqlx::postgres::PgRow,
) -> Result<ShadowEvaluationRecord, ShadowEvaluationPersistenceError> {
    let evaluation_state_raw: String = row.try_get("evaluation_state").map_err(|error| {
        ShadowEvaluationPersistenceError::row_decode_failure("evaluation_state", error)
    })?;
    let evaluation_state =
        ShadowEvaluationState::parse(&evaluation_state_raw).map_err(|error| {
            ShadowEvaluationPersistenceError::row_contract_failure("evaluation_state", error)
        })?;
    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        ShadowEvaluationPersistenceError::row_decode_failure("reason_code", error)
    })?;
    ShadowEvaluationReasonCode::parse(&reason_code).map_err(|error| {
        ShadowEvaluationPersistenceError::row_contract_failure("reason_code", error)
    })?;

    let simulation_outcomes_json: Value =
        row.try_get("simulation_outcomes_json").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("simulation_outcomes_json", error)
        })?;
    let simulation_outcomes = decode_simulation_outcomes_json(simulation_outcomes_json)?;

    let record = ShadowEvaluationRecord {
        evaluation_id: row.try_get("evaluation_id").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("evaluation_id", error)
        })?,
        candidate_id: row.try_get("candidate_id").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("candidate_id", error)
        })?,
        validation_run_id: row.try_get("validation_run_id").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("validation_run_id", error)
        })?,
        evaluation_state,
        reason_code,
        market_context: row.try_get("market_context_json").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("market_context_json", error)
        })?,
        signal_decisions: row.try_get("signal_decisions_json").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("signal_decisions_json", error)
        })?,
        simulation_outcomes,
        actor_id: row.try_get("actor_id").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        started_at_utc: row.try_get("started_at_utc").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("started_at_utc", error)
        })?,
        completed_at_utc: row.try_get("completed_at_utc").map_err(|error| {
            ShadowEvaluationPersistenceError::row_decode_failure("completed_at_utc", error)
        })?,
    };

    validate_record_for_persistence(&record)
}

fn decode_simulation_outcomes_json(
    value: Value,
) -> Result<Vec<ShadowSimulationOutcome>, ShadowEvaluationPersistenceError> {
    let Value::Object(map) = value else {
        return Err(ShadowEvaluationPersistenceError::invalid_payload(
            "simulation_outcomes_json must be a JSON object",
            vec![ShadowEvaluationValidationIssue {
                field: "simulation_outcomes_json".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "simulation_outcomes_json must be a JSON object".to_string(),
            }],
        ));
    };
    let Some(outcomes) = map.get("outcomes") else {
        return Err(ShadowEvaluationPersistenceError::invalid_payload(
            "simulation_outcomes_json must include `outcomes`",
            vec![ShadowEvaluationValidationIssue {
                field: "simulation_outcomes_json.outcomes".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "simulation_outcomes_json must include `outcomes`".to_string(),
            }],
        ));
    };
    let Value::Array(outcomes) = outcomes else {
        return Err(ShadowEvaluationPersistenceError::invalid_payload(
            "simulation_outcomes_json.outcomes must be an array",
            vec![ShadowEvaluationValidationIssue {
                field: "simulation_outcomes_json.outcomes".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "simulation_outcomes_json.outcomes must be an array".to_string(),
            }],
        ));
    };
    outcomes
        .iter()
        .enumerate()
        .map(|(index, outcome)| {
            serde_json::from_value::<ShadowSimulationOutcome>(outcome.clone()).map_err(|error| {
                ShadowEvaluationPersistenceError::invalid_payload(
                    format!("invalid simulation outcome at index {index}: {error}"),
                    vec![ShadowEvaluationValidationIssue {
                        field: format!("simulation_outcomes_json.outcomes[{index}]"),
                        code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                        message: "invalid shadow simulation outcome payload".to_string(),
                    }],
                )
            })
        })
        .collect()
}

fn validate_record_for_persistence(
    record: &ShadowEvaluationRecord,
) -> Result<ShadowEvaluationRecord, ShadowEvaluationPersistenceError> {
    canonicalize_shadow_evaluation_record(record).map_err(map_contract_error)
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, ShadowEvaluationPersistenceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_shadow_utc_timestamp(trimmed).map_err(|_| {
        ShadowEvaluationPersistenceError::invalid_payload(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![ShadowEvaluationValidationIssue {
                field: field.to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    Ok(Some(trimmed.to_string()))
}

fn map_contract_error(error: ShadowEvaluationContractError) -> ShadowEvaluationPersistenceError {
    ShadowEvaluationPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ShadowEvaluationPersistenceError> {
    if value.trim().is_empty() {
        return Err(ShadowEvaluationPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![ShadowEvaluationValidationIssue {
                field: field.to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ShadowEvaluationPersistenceError {
    if is_constraint_error(&error) {
        return ShadowEvaluationPersistenceError::constraint_violation(operation, error);
    }
    ShadowEvaluationPersistenceError::query_failure(operation, error)
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

    const SHADOW_EVALUATION_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407210000_shadow_evaluations.sql");

    fn sample_shadow_evaluation_record() -> ShadowEvaluationRecord {
        ShadowEvaluationRecord {
            evaluation_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            evaluation_state: ShadowEvaluationState::Completed,
            reason_code: ShadowEvaluationReasonCode::EvaluationCompleted
                .code()
                .to_string(),
            market_context: json!({
                "best_bid": 0.42,
                "best_ask": 0.44
            }),
            signal_decisions: json!({
                "signals": [
                    {
                        "decision_side": "buy",
                        "intended_size": 10.0
                    }
                ]
            }),
            simulation_outcomes: vec![ShadowSimulationOutcome {
                decision_side: domain::research::ShadowSimulationDecisionSide::Buy,
                intended_size: 10.0,
                simulated_fill_size: 10.0,
                simulated_fill_price: 0.43,
                simulated_slippage_bps: 5.0,
                simulation_reason_code: "shadow_simulation_read_only_enforced".to_string(),
                decision_timestamp_utc: "2026-04-07T00:00:01Z".to_string(),
                simulated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            }],
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-shadow-001".to_string(),
            started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:00:03Z".to_string()),
        }
    }

    #[test]
    fn migration_creates_expected_shadow_evaluation_schema_scope() {
        assert!(
            SHADOW_EVALUATION_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS shadow_evaluations")
        );
        assert!(
            !SHADOW_EVALUATION_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS validation_runs")
        );
        assert!(
            !SHADOW_EVALUATION_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS validation_artifacts")
        );
    }

    #[test]
    fn migration_enforces_shadow_evaluation_constraints_and_indexes() {
        assert!(
            SHADOW_EVALUATION_MIGRATION_SQL
                .contains("evaluation_state IN ('running', 'completed', 'denied', 'failed')")
        );
        assert!(SHADOW_EVALUATION_MIGRATION_SQL.contains("simulation_outcomes_json ? 'outcomes'"));
        assert!(
            SHADOW_EVALUATION_MIGRATION_SQL.contains("idx_shadow_evaluations_candidate_lookup")
        );
        assert!(
            SHADOW_EVALUATION_MIGRATION_SQL.contains("idx_shadow_evaluations_id_canonical_unique")
        );
    }

    #[test]
    fn shadow_evaluation_validation_canonicalizes_identifiers() {
        let mut record = sample_shadow_evaluation_record();
        record.evaluation_id = " Candidate::Alpha-1::1712448000 ".to_string();
        record.candidate_id = " Candidate::Alpha-1 ".to_string();
        record.validation_run_id = " Candidate::Alpha-1::1712447000 ".to_string();
        record.reason_code = " Shadow_Evaluation_Completed ".to_string();

        let canonical = validate_record_for_persistence(&record)
            .expect("canonical shadow evaluation should pass");
        assert_eq!(canonical.evaluation_id, "candidate::alpha-1::1712448000");
        assert_eq!(canonical.candidate_id, "candidate::alpha-1");
        assert_eq!(
            canonical.validation_run_id,
            "candidate::alpha-1::1712447000"
        );
        assert_eq!(canonical.reason_code, "shadow_evaluation_completed");
    }

    #[test]
    fn shadow_evaluation_validation_rejects_non_object_signal_decisions() {
        let mut record = sample_shadow_evaluation_record();
        record.signal_decisions = json!(["buy"]);

        let error = validate_record_for_persistence(&record)
            .expect_err("signal_decisions must be a JSON object");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "signal_decisions")
        );
    }

    #[test]
    fn list_query_orders_shadow_evaluations_deterministically() {
        assert!(
            LIST_SHADOW_EVALUATIONS_BY_CANDIDATE_SQL
                .contains("ORDER BY started_at_utc DESC, evaluation_id ASC")
        );
    }

    #[test]
    fn latest_query_filters_by_candidate_and_validation_run() {
        assert!(
            LOAD_LATEST_SHADOW_EVALUATION_BY_CANDIDATE_AND_RUN_SQL
                .contains("lower(trim(candidate_id)) = lower(trim($1))")
        );
        assert!(
            LOAD_LATEST_SHADOW_EVALUATION_BY_CANDIDATE_AND_RUN_SQL
                .contains("lower(trim(validation_run_id)) = lower(trim($2))")
        );
        assert!(
            LOAD_LATEST_SHADOW_EVALUATION_BY_CANDIDATE_AND_RUN_SQL
                .contains("ORDER BY started_at_utc DESC, evaluation_id ASC")
        );
    }
}
