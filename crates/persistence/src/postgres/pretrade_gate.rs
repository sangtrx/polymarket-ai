use domain::risk::{
    PreTradeDecisionOutcome, PreTradeGateContractError, PreTradeGateDecision,
    PreTradeGateValidationIssue, PreTradeReasonCode, normalize_pretrade_identifier,
    validate_pretrade_gate_decision,
};
use serde_json::json;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_PRETRADE_GATE_DECISION_SQL: &str = r#"
    INSERT INTO pretrade_gate_decisions (
        decision_id,
        intent_id,
        market_id,
        cluster_id,
        profile_key,
        outcome,
        reason_code,
        protective_mode_active,
        gate_results,
        correlation_id,
        evaluated_at_utc,
        evidence
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12
    )
"#;

const LOAD_LATEST_PRETRADE_GATE_DECISION_BY_INTENT_SQL: &str = r#"
    SELECT
        decision_id,
        intent_id,
        market_id,
        cluster_id,
        profile_key,
        outcome,
        reason_code,
        protective_mode_active,
        gate_results,
        correlation_id,
        to_char(evaluated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS evaluated_at_utc
    FROM pretrade_gate_decisions
    WHERE intent_id = $1
    ORDER BY evaluated_at_utc DESC, decision_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_PRETRADE_GATE_DECISION_BY_CORRELATION_SQL: &str = r#"
    SELECT
        decision_id,
        intent_id,
        market_id,
        cluster_id,
        profile_key,
        outcome,
        reason_code,
        protective_mode_active,
        gate_results,
        correlation_id,
        to_char(evaluated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS evaluated_at_utc
    FROM pretrade_gate_decisions
    WHERE correlation_id = $1
    ORDER BY evaluated_at_utc DESC, decision_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_PRETRADE_GATE_DECISION_FOR_MARKET_SQL: &str = r#"
    SELECT
        decision_id,
        intent_id,
        market_id,
        cluster_id,
        profile_key,
        outcome,
        reason_code,
        protective_mode_active,
        gate_results,
        correlation_id,
        to_char(evaluated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS evaluated_at_utc
    FROM pretrade_gate_decisions
    WHERE market_id = $1
    ORDER BY evaluated_at_utc DESC, decision_id DESC
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTradeGatePersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<PreTradeGateValidationIssue>,
}

impl PreTradeGatePersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<PreTradeGateValidationIssue>,
    ) -> Self {
        Self {
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn persistence_unavailable(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: PreTradeReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: PreTradeReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: PreTradeReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for PreTradeGatePersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for PreTradeGatePersistenceError {}

pub async fn insert_pretrade_gate_decision<'e, E>(
    executor: E,
    decision: &PreTradeGateDecision,
) -> Result<(), PreTradeGatePersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_pretrade_gate_decision(decision).map_err(map_contract_error)?;
    let gate_results = serde_json::to_value(&decision.gate_results).map_err(|error| {
        PreTradeGatePersistenceError::invalid_payload(
            format!("unable to serialize gate_results to json: {error}"),
            Vec::new(),
        )
    })?;
    let failed_gate_count = decision
        .gate_results
        .iter()
        .filter(|gate| !gate.passed)
        .count();
    let evidence = json!({
        "decision_id": decision.decision_id,
        "outcome": decision.outcome.as_str(),
        "reason_code": decision.reason_code,
        "protective_mode_active": decision.protective_mode_active,
        "evaluated_gate_count": decision.gate_results.len(),
        "failed_gate_count": failed_gate_count,
    });

    let result = sqlx::query(INSERT_PRETRADE_GATE_DECISION_SQL)
        .bind(&decision.decision_id)
        .bind(&decision.intent_id)
        .bind(&decision.market_id)
        .bind(&decision.cluster_id)
        .bind(&decision.profile_key)
        .bind(decision.outcome.as_str())
        .bind(&decision.reason_code)
        .bind(decision.protective_mode_active)
        .bind(gate_results)
        .bind(&decision.correlation_id)
        .bind(&decision.evaluated_at_utc)
        .bind(evidence)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_pretrade_gate_decision", error))?;

    if result.rows_affected() != 1 {
        return Err(PreTradeGatePersistenceError::invalid_payload(
            format!(
                "insert_pretrade_gate_decision expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_latest_pretrade_gate_decision_by_intent<'e, E>(
    executor: E,
    intent_id: &str,
) -> Result<Option<PreTradeGateDecision>, PreTradeGatePersistenceError>
where
    E: PgExecutor<'e>,
{
    let intent_id = normalize_lookup_identifier("intent_id", intent_id)?;
    let row = sqlx::query(LOAD_LATEST_PRETRADE_GATE_DECISION_BY_INTENT_SQL)
        .bind(&intent_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_pretrade_gate_decision_by_intent", error)
        })?;
    row.map(decode_pretrade_gate_decision_row).transpose()
}

pub async fn load_latest_pretrade_gate_decision_by_correlation<'e, E>(
    executor: E,
    correlation_id: &str,
) -> Result<Option<PreTradeGateDecision>, PreTradeGatePersistenceError>
where
    E: PgExecutor<'e>,
{
    let correlation_id = normalize_lookup_identifier("correlation_id", correlation_id)?;
    let row = sqlx::query(LOAD_LATEST_PRETRADE_GATE_DECISION_BY_CORRELATION_SQL)
        .bind(&correlation_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_pretrade_gate_decision_by_correlation", error)
        })?;
    row.map(decode_pretrade_gate_decision_row).transpose()
}

pub async fn load_latest_pretrade_gate_decision_for_market<'e, E>(
    executor: E,
    market_id: &str,
) -> Result<Option<PreTradeGateDecision>, PreTradeGatePersistenceError>
where
    E: PgExecutor<'e>,
{
    let market_id = normalize_lookup_identifier("market_id", market_id)?;
    let row = sqlx::query(LOAD_LATEST_PRETRADE_GATE_DECISION_FOR_MARKET_SQL)
        .bind(&market_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_pretrade_gate_decision_for_market", error)
        })?;
    row.map(decode_pretrade_gate_decision_row).transpose()
}

fn decode_pretrade_gate_decision_row(
    row: sqlx::postgres::PgRow,
) -> Result<PreTradeGateDecision, PreTradeGatePersistenceError> {
    let outcome: String = row
        .try_get("outcome")
        .map_err(|error| PreTradeGatePersistenceError::row_decode_failure("outcome", error))?;
    let outcome = PreTradeDecisionOutcome::parse(&outcome).map_err(map_contract_error)?;

    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| PreTradeGatePersistenceError::row_decode_failure("reason_code", error))?;
    let reason_code = PreTradeReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let gate_results_raw: serde_json::Value = row
        .try_get("gate_results")
        .map_err(|error| PreTradeGatePersistenceError::row_decode_failure("gate_results", error))?;
    let gate_results = serde_json::from_value(gate_results_raw).map_err(|error| {
        PreTradeGatePersistenceError::invalid_payload(
            format!("unable to parse gate_results json payload: {error}"),
            Vec::new(),
        )
    })?;

    let decision = PreTradeGateDecision {
        decision_id: row.try_get("decision_id").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("decision_id", error)
        })?,
        intent_id: row.try_get("intent_id").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("intent_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("market_id", error)
        })?,
        cluster_id: row.try_get("cluster_id").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("cluster_id", error)
        })?,
        profile_key: row.try_get("profile_key").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("profile_key", error)
        })?,
        outcome,
        reason_code: reason_code.code().to_string(),
        protective_mode_active: row.try_get("protective_mode_active").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("protective_mode_active", error)
        })?,
        gate_results,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("correlation_id", error)
        })?,
        evaluated_at_utc: row.try_get("evaluated_at_utc").map_err(|error| {
            PreTradeGatePersistenceError::row_decode_failure("evaluated_at_utc", error)
        })?,
    };

    validate_pretrade_gate_decision(&decision).map_err(map_contract_error)?;
    Ok(decision)
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, PreTradeGatePersistenceError> {
    let normalized = normalize_pretrade_identifier(value);
    if normalized.is_empty() {
        return Err(PreTradeGatePersistenceError::invalid_payload(
            format!("{field} must not be empty"),
            vec![PreTradeGateValidationIssue {
                field,
                code: PreTradeReasonCode::InvalidPayload.code(),
                message: format!("{field} must not be empty"),
            }],
        ));
    }
    Ok(normalized)
}

fn map_contract_error(error: PreTradeGateContractError) -> PreTradeGatePersistenceError {
    PreTradeGatePersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> PreTradeGatePersistenceError {
    if is_constraint_error(&error) {
        return PreTradeGatePersistenceError::constraint_violation(operation, error);
    }
    PreTradeGatePersistenceError::persistence_unavailable(operation, error)
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
    use domain::risk::{PreTradeGateDimension, PreTradeGateResult};

    const PRETRADE_GATE_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406081500_pretrade_gate_decisions.sql");

    fn sample_gate_result(
        dimension: PreTradeGateDimension,
        passed: bool,
        reason_code: PreTradeReasonCode,
    ) -> PreTradeGateResult {
        PreTradeGateResult {
            gate: dimension,
            passed,
            reason_code: reason_code.code().to_string(),
            evaluated_at_utc: "2026-04-06T08:16:00Z".to_string(),
        }
    }

    fn sample_decision() -> PreTradeGateDecision {
        PreTradeGateDecision {
            decision_id: "pretrade::decision_0001".to_string(),
            intent_id: "intent_0001".to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            profile_key: "profile_default".to_string(),
            outcome: PreTradeDecisionOutcome::Deny,
            reason_code: PreTradeReasonCode::FreshnessStateUnavailable
                .code()
                .to_string(),
            protective_mode_active: false,
            gate_results: vec![sample_gate_result(
                PreTradeGateDimension::Freshness,
                false,
                PreTradeReasonCode::FreshnessStateUnavailable,
            )],
            correlation_id: "corr_pretrade_0001".to_string(),
            evaluated_at_utc: "2026-04-06T08:16:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_pretrade_schema_scope() {
        assert!(
            PRETRADE_GATE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS pretrade_gate_decisions")
        );
        assert!(!PRETRADE_GATE_MIGRATION_SQL.contains("risk_limit_profiles"));
        assert!(!PRETRADE_GATE_MIGRATION_SQL.contains("inventory_limit_rules"));
        assert!(!PRETRADE_GATE_MIGRATION_SQL.contains("safety_control_actions"));
    }

    #[test]
    fn migration_enforces_constraints_and_indexes_for_traceability() {
        assert!(PRETRADE_GATE_MIGRATION_SQL.contains("outcome IN ('allow', 'deny')"));
        assert!(PRETRADE_GATE_MIGRATION_SQL.contains("jsonb_typeof(gate_results) = 'array'"));
        assert!(PRETRADE_GATE_MIGRATION_SQL.contains("jsonb_array_length(gate_results) > 0"));
        assert!(PRETRADE_GATE_MIGRATION_SQL.contains("idx_pretrade_gate_decisions_intent_unique"));
        assert!(PRETRADE_GATE_MIGRATION_SQL.contains("idx_pretrade_gate_decisions_market_time"));
        assert!(PRETRADE_GATE_MIGRATION_SQL.contains("idx_pretrade_gate_decisions_reason_time"));
        assert!(
            PRETRADE_GATE_MIGRATION_SQL.contains("idx_pretrade_gate_decisions_correlation_time")
        );
    }

    #[test]
    fn decision_validation_accepts_canonical_payload() {
        assert!(validate_pretrade_gate_decision(&sample_decision()).is_ok());
    }

    #[test]
    fn lookup_identifier_normalization_rejects_empty_values() {
        let error = normalize_lookup_identifier("intent_id", "   ")
            .expect_err("empty lookup values must be rejected");
        assert_eq!(error.code, PreTradeReasonCode::InvalidPayload.code());
        assert_eq!(error.field_errors[0].field, "intent_id");
    }

    #[test]
    fn latest_lookup_queries_remain_deterministic() {
        assert!(LOAD_LATEST_PRETRADE_GATE_DECISION_BY_INTENT_SQL.contains("WHERE intent_id = $1"));
        assert!(
            LOAD_LATEST_PRETRADE_GATE_DECISION_BY_CORRELATION_SQL
                .contains("WHERE correlation_id = $1")
        );
        assert!(LOAD_LATEST_PRETRADE_GATE_DECISION_FOR_MARKET_SQL.contains("WHERE market_id = $1"));
        assert!(
            LOAD_LATEST_PRETRADE_GATE_DECISION_BY_INTENT_SQL
                .contains("ORDER BY evaluated_at_utc DESC, decision_id DESC")
        );
        assert!(
            LOAD_LATEST_PRETRADE_GATE_DECISION_BY_CORRELATION_SQL
                .contains("ORDER BY evaluated_at_utc DESC, decision_id DESC")
        );
        assert!(
            LOAD_LATEST_PRETRADE_GATE_DECISION_FOR_MARKET_SQL
                .contains("ORDER BY evaluated_at_utc DESC, decision_id DESC")
        );
    }
}
