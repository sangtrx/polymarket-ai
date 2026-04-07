use domain::research::{
    AlphaHypothesisContractError, AlphaHypothesisReasonCode, AlphaHypothesisRegistration,
    AlphaHypothesisValidationIssue, canonicalize_alpha_hypothesis_registration,
    normalize_research_identifier, parse_utc_timestamp,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_ALPHA_HYPOTHESIS_SQL: &str = r#"
    INSERT INTO alpha_hypotheses (
        hypothesis_id,
        feature_set_version,
        target_regime,
        expected_edge_source,
        training_window_start_utc,
        training_window_end_utc,
        risk_assumptions_json,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5::timestamptz, $6::timestamptz, $7, $8, $9, $10::timestamptz
    )
    ON CONFLICT (hypothesis_id)
    DO UPDATE SET
        feature_set_version = EXCLUDED.feature_set_version,
        target_regime = EXCLUDED.target_regime,
        expected_edge_source = EXCLUDED.expected_edge_source,
        training_window_start_utc = EXCLUDED.training_window_start_utc,
        training_window_end_utc = EXCLUDED.training_window_end_utc,
        risk_assumptions_json = EXCLUDED.risk_assumptions_json,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_ALPHA_HYPOTHESIS_SQL: &str = r#"
    SELECT
        hypothesis_id,
        feature_set_version,
        target_regime,
        expected_edge_source,
        to_char(training_window_start_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS training_window_start_utc,
        to_char(training_window_end_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS training_window_end_utc,
        risk_assumptions_json,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM alpha_hypotheses
    WHERE lower(trim(hypothesis_id)) = lower(trim($1))
    ORDER BY updated_at_utc DESC, hypothesis_id ASC
    LIMIT 1
"#;

const LIST_ALPHA_HYPOTHESES_BY_FEATURE_SET_SQL: &str = r#"
    SELECT
        hypothesis_id,
        feature_set_version,
        target_regime,
        expected_edge_source,
        to_char(training_window_start_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS training_window_start_utc,
        to_char(training_window_end_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS training_window_end_utc,
        risk_assumptions_json,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM alpha_hypotheses
    WHERE lower(trim(feature_set_version)) = lower(trim($1))
    ORDER BY updated_at_utc DESC, hypothesis_id ASC
    LIMIT $2
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphaHypothesisPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlphaHypothesisValidationIssue>,
}

impl AlphaHypothesisPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlphaHypothesisValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_hypothesis_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_hypothesis_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "alpha_hypothesis_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for AlphaHypothesisPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AlphaHypothesisPersistenceError {}

pub async fn upsert_alpha_hypothesis<'e, E>(
    executor: E,
    registration: &AlphaHypothesisRegistration,
) -> Result<(), AlphaHypothesisPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_registration_for_persistence(registration)?;

    let result = sqlx::query(UPSERT_ALPHA_HYPOTHESIS_SQL)
        .bind(&canonical.hypothesis_id)
        .bind(&canonical.feature_set_version)
        .bind(&canonical.target_regime)
        .bind(&canonical.expected_edge_source)
        .bind(&canonical.training_window_start_utc)
        .bind(&canonical.training_window_end_utc)
        .bind(&canonical.risk_assumptions)
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.updated_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_alpha_hypothesis", error))?;

    if result.rows_affected() != 1 {
        return Err(AlphaHypothesisPersistenceError::invalid_payload(
            format!(
                "upsert_alpha_hypothesis expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_alpha_hypothesis<'e, E>(
    executor: E,
    hypothesis_id: &str,
) -> Result<Option<AlphaHypothesisRegistration>, AlphaHypothesisPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("hypothesis_id", hypothesis_id)?;
    let normalized_hypothesis_id = normalize_research_identifier(hypothesis_id);

    let row = sqlx::query(LOAD_ALPHA_HYPOTHESIS_SQL)
        .bind(&normalized_hypothesis_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            AlphaHypothesisPersistenceError::query_failure("load_alpha_hypothesis", error)
        })?;

    row.map(decode_alpha_hypothesis_row).transpose()
}

pub async fn list_alpha_hypotheses_by_feature_set<'e, E>(
    executor: E,
    feature_set_version: &str,
    limit: i64,
) -> Result<Vec<AlphaHypothesisRegistration>, AlphaHypothesisPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("feature_set_version", feature_set_version)?;
    if limit <= 0 {
        return Err(AlphaHypothesisPersistenceError::invalid_payload(
            "limit must be greater than 0",
            vec![AlphaHypothesisValidationIssue {
                field: "limit".to_string(),
                code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let normalized_feature_set = normalize_research_identifier(feature_set_version);

    let rows = sqlx::query(LIST_ALPHA_HYPOTHESES_BY_FEATURE_SET_SQL)
        .bind(&normalized_feature_set)
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            AlphaHypothesisPersistenceError::query_failure(
                "list_alpha_hypotheses_by_feature_set",
                error,
            )
        })?;

    rows.into_iter().map(decode_alpha_hypothesis_row).collect()
}

fn decode_alpha_hypothesis_row(
    row: sqlx::postgres::PgRow,
) -> Result<AlphaHypothesisRegistration, AlphaHypothesisPersistenceError> {
    let registration = AlphaHypothesisRegistration {
        hypothesis_id: row.try_get("hypothesis_id").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("hypothesis_id", error)
        })?,
        feature_set_version: row.try_get("feature_set_version").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("feature_set_version", error)
        })?,
        target_regime: row.try_get("target_regime").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("target_regime", error)
        })?,
        expected_edge_source: row.try_get("expected_edge_source").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("expected_edge_source", error)
        })?,
        training_window_start_utc: row.try_get("training_window_start_utc").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("training_window_start_utc", error)
        })?,
        training_window_end_utc: row.try_get("training_window_end_utc").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("training_window_end_utc", error)
        })?,
        risk_assumptions: row.try_get("risk_assumptions_json").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("risk_assumptions_json", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            AlphaHypothesisPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_registration_for_persistence(&registration)
}

fn validate_registration_for_persistence(
    registration: &AlphaHypothesisRegistration,
) -> Result<AlphaHypothesisRegistration, AlphaHypothesisPersistenceError> {
    let canonical =
        canonicalize_alpha_hypothesis_registration(registration).map_err(map_contract_error)?;
    parse_utc_timestamp(&canonical.training_window_start_utc).map_err(map_contract_error)?;
    parse_utc_timestamp(&canonical.training_window_end_utc).map_err(map_contract_error)?;
    parse_utc_timestamp(&canonical.updated_at_utc).map_err(map_contract_error)?;
    Ok(canonical)
}

fn map_contract_error(error: AlphaHypothesisContractError) -> AlphaHypothesisPersistenceError {
    AlphaHypothesisPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AlphaHypothesisPersistenceError> {
    if value.trim().is_empty() {
        return Err(AlphaHypothesisPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaHypothesisValidationIssue {
                field: field.to_string(),
                code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> AlphaHypothesisPersistenceError {
    if is_constraint_error(&error) {
        return AlphaHypothesisPersistenceError::constraint_violation(operation, error);
    }
    AlphaHypothesisPersistenceError::query_failure(operation, error)
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

    const ALPHA_HYPOTHESIS_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407160000_alpha_hypotheses.sql");

    fn sample_registration() -> AlphaHypothesisRegistration {
        AlphaHypothesisRegistration {
            hypothesis_id: "alpha::mean-reversion".to_string(),
            feature_set_version: "dataset::v1".to_string(),
            target_regime: "overnight".to_string(),
            expected_edge_source: "liquidity_dislocation".to_string(),
            training_window_start_utc: "2026-04-01T00:00:00Z".to_string(),
            training_window_end_utc: "2026-04-02T00:00:00Z".to_string(),
            risk_assumptions: json!({
                "max_drawdown_pct": 2.5,
                "max_inventory_units": 1000
            }),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_alpha_hypothesis_schema_scope() {
        assert!(
            ALPHA_HYPOTHESIS_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS alpha_hypotheses")
        );
        assert!(
            !ALPHA_HYPOTHESIS_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS reward_risk_policies")
        );
    }

    #[test]
    fn migration_enforces_constraints_and_indexes() {
        assert!(
            ALPHA_HYPOTHESIS_MIGRATION_SQL
                .contains("training_window_start_utc < training_window_end_utc")
        );
        assert!(
            ALPHA_HYPOTHESIS_MIGRATION_SQL
                .contains("jsonb_typeof(risk_assumptions_json) = 'object'")
        );
        assert!(
            ALPHA_HYPOTHESIS_MIGRATION_SQL
                .contains("idx_alpha_hypotheses_hypothesis_id_canonical_unique")
        );
        assert!(ALPHA_HYPOTHESIS_MIGRATION_SQL.contains("idx_alpha_hypotheses_feature_set_lookup"));
    }

    #[test]
    fn registration_validation_canonicalizes_identifiers() {
        let mut registration = sample_registration();
        registration.hypothesis_id = " Alpha::Mean-Reversion ".to_string();
        registration.feature_set_version = " Dataset::V1 ".to_string();

        let canonical = validate_registration_for_persistence(&registration)
            .expect("canonical registration should pass");
        assert_eq!(canonical.hypothesis_id, "alpha::mean-reversion");
        assert_eq!(canonical.feature_set_version, "dataset::v1");
    }

    #[test]
    fn registration_validation_rejects_equal_training_boundaries() {
        let mut registration = sample_registration();
        registration.training_window_end_utc = registration.training_window_start_utc.clone();

        let error = validate_registration_for_persistence(&registration)
            .expect_err("equal boundaries should fail");
        assert_eq!(error.code, AlphaHypothesisReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "training_window")
        );
    }

    #[test]
    fn list_query_orders_results_deterministically() {
        assert!(
            LIST_ALPHA_HYPOTHESES_BY_FEATURE_SET_SQL
                .contains("ORDER BY updated_at_utc DESC, hypothesis_id ASC")
        );
    }

    #[test]
    fn normalize_identifier_trims_and_lowercases() {
        assert_eq!(
            normalize_research_identifier(" Alpha::Mean-Reversion "),
            "alpha::mean-reversion"
        );
    }
}
