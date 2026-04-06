use domain::allocation::{
    AllocationApprovalStatus, AllocationContractError, AllocationPolicyVersion,
    AllocationValidationIssue, RebalanceReasonCode, RebalanceRecommendation,
    RebalanceRecommendationStatus, normalize_allocation_identifier,
    validate_allocation_policy_version, validate_rebalance_recommendation,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const DEACTIVATE_ACTIVE_POLICY_SQL: &str = r#"
    UPDATE allocation_policies
    SET
        approval_status = 'denied',
        is_active = FALSE,
        reason_code = $2,
        updated_at_utc = $3::timestamptz
    WHERE policy_key = $1
      AND is_active = TRUE
"#;

const UPSERT_ALLOCATION_POLICY_SQL: &str = r#"
    INSERT INTO allocation_policies (
        policy_key,
        version,
        portfolio_scope_id,
        target_exposure_pct_nav,
        target_relative_alpha_weight,
        exposure_drift_threshold_pct,
        relative_alpha_drift_threshold_pct,
        approval_status,
        is_active,
        approval_reference,
        actor_id,
        reason_code,
        correlation_id,
        updated_at_utc,
        advanced_parameters
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14::timestamptz, $15::jsonb
    )
    ON CONFLICT (policy_key, version)
    DO UPDATE SET
        portfolio_scope_id = EXCLUDED.portfolio_scope_id,
        target_exposure_pct_nav = EXCLUDED.target_exposure_pct_nav,
        target_relative_alpha_weight = EXCLUDED.target_relative_alpha_weight,
        exposure_drift_threshold_pct = EXCLUDED.exposure_drift_threshold_pct,
        relative_alpha_drift_threshold_pct = EXCLUDED.relative_alpha_drift_threshold_pct,
        approval_status = EXCLUDED.approval_status,
        is_active = EXCLUDED.is_active,
        approval_reference = EXCLUDED.approval_reference,
        actor_id = EXCLUDED.actor_id,
        reason_code = EXCLUDED.reason_code,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc,
        advanced_parameters = EXCLUDED.advanced_parameters
"#;

const LOAD_ACTIVE_ALLOCATION_POLICY_SQL: &str = r#"
    SELECT
        policy_key,
        version,
        portfolio_scope_id,
        target_exposure_pct_nav,
        target_relative_alpha_weight,
        exposure_drift_threshold_pct,
        relative_alpha_drift_threshold_pct,
        approval_status,
        approval_reference,
        actor_id,
        reason_code,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc,
        advanced_parameters
    FROM allocation_policies
    WHERE policy_key = $1
      AND is_active = TRUE
    ORDER BY version DESC, updated_at_utc DESC
    LIMIT 1
"#;

const UPSERT_REBALANCE_RECOMMENDATION_SQL: &str = r#"
    INSERT INTO rebalance_recommendations (
        recommendation_id,
        policy_key,
        policy_version,
        status,
        approval_status,
        approval_reference,
        action_type,
        rationale,
        reason_code,
        actor_id,
        correlation_id,
        parameters,
        evidence,
        created_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13::jsonb, $14::timestamptz, $15::timestamptz
    )
    ON CONFLICT (recommendation_id)
    DO UPDATE SET
        policy_key = EXCLUDED.policy_key,
        policy_version = EXCLUDED.policy_version,
        status = EXCLUDED.status,
        approval_status = EXCLUDED.approval_status,
        approval_reference = EXCLUDED.approval_reference,
        action_type = EXCLUDED.action_type,
        rationale = EXCLUDED.rationale,
        reason_code = EXCLUDED.reason_code,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        parameters = EXCLUDED.parameters,
        evidence = EXCLUDED.evidence,
        created_at_utc = EXCLUDED.created_at_utc,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_PENDING_REBALANCE_RECOMMENDATIONS_SQL: &str = r#"
    SELECT
        recommendation_id,
        policy_key,
        policy_version,
        status,
        approval_status,
        approval_reference,
        action_type,
        rationale,
        reason_code,
        actor_id,
        correlation_id,
        parameters,
        evidence,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM rebalance_recommendations
    WHERE approval_status = 'pending'
    ORDER BY policy_key ASC, created_at_utc DESC, recommendation_id DESC
"#;

const LOAD_PENDING_REBALANCE_RECOMMENDATIONS_FOR_KEY_SQL: &str = r#"
    SELECT
        recommendation_id,
        policy_key,
        policy_version,
        status,
        approval_status,
        approval_reference,
        action_type,
        rationale,
        reason_code,
        actor_id,
        correlation_id,
        parameters,
        evidence,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM rebalance_recommendations
    WHERE approval_status = 'pending'
      AND policy_key = $1
    ORDER BY created_at_utc DESC, recommendation_id DESC
"#;

const LOAD_LATEST_REBALANCE_RECOMMENDATION_FOR_POLICY_SQL: &str = r#"
    SELECT
        recommendation_id,
        policy_key,
        policy_version,
        status,
        approval_status,
        approval_reference,
        action_type,
        rationale,
        reason_code,
        actor_id,
        correlation_id,
        parameters,
        evidence,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM rebalance_recommendations
    WHERE policy_key = $1
    ORDER BY created_at_utc DESC, recommendation_id DESC
    LIMIT 1
"#;

const LOAD_REBALANCE_RECOMMENDATION_BY_ID_SQL: &str = r#"
    SELECT
        recommendation_id,
        policy_key,
        policy_version,
        status,
        approval_status,
        approval_reference,
        action_type,
        rationale,
        reason_code,
        actor_id,
        correlation_id,
        parameters,
        evidence,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM rebalance_recommendations
    WHERE recommendation_id = $1
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AllocationValidationIssue>,
}

impl AllocationPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AllocationValidationIssue>,
    ) -> Self {
        Self {
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "allocation_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "allocation_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "allocation_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for AllocationPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AllocationPersistenceError {}

pub async fn upsert_allocation_policy_version(
    pool: &PgPool,
    policy: &AllocationPolicyVersion,
) -> Result<(), AllocationPersistenceError> {
    validate_allocation_policy_version(policy).map_err(map_contract_error)?;
    let normalized_policy_key = normalize_allocation_identifier(&policy.policy_key);

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_allocation_policy_tx", error))?;

    if policy.approval_status == AllocationApprovalStatus::Approved {
        sqlx::query(DEACTIVATE_ACTIVE_POLICY_SQL)
            .bind(&normalized_policy_key)
            .bind(RebalanceReasonCode::AllocationPolicyDenied.code())
            .bind(&policy.updated_at_utc)
            .execute(&mut *tx)
            .await
            .map_err(|error| classify_query_error("deactivate_active_allocation_policy", error))?;
    }

    let is_active = policy.approval_status == AllocationApprovalStatus::Approved;
    let result = sqlx::query(UPSERT_ALLOCATION_POLICY_SQL)
        .bind(&normalized_policy_key)
        .bind(policy.version)
        .bind(normalize_allocation_identifier(&policy.portfolio_scope_id))
        .bind(policy.target_exposure_pct_nav)
        .bind(policy.target_relative_alpha_weight)
        .bind(policy.exposure_drift_threshold_pct)
        .bind(policy.relative_alpha_drift_threshold_pct)
        .bind(policy.approval_status.as_str())
        .bind(is_active)
        .bind(policy.approval_reference.as_deref())
        .bind(&policy.actor_id)
        .bind(&policy.reason_code)
        .bind(&policy.correlation_id)
        .bind(&policy.updated_at_utc)
        .bind(&policy.advanced_parameters)
        .execute(&mut *tx)
        .await
        .map_err(|error| classify_query_error("upsert_allocation_policy_version", error))?;

    if result.rows_affected() != 1 {
        return Err(AllocationPersistenceError::invalid_payload(
            format!(
                "upsert_allocation_policy_version expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    tx.commit()
        .await
        .map_err(|error| classify_query_error("commit_allocation_policy_tx", error))?;
    Ok(())
}

pub async fn load_active_allocation_policy_version(
    pool: &PgPool,
    policy_key: &str,
) -> Result<Option<AllocationPolicyVersion>, AllocationPersistenceError> {
    validate_non_empty("policy_key", policy_key)?;
    let normalized_policy_key = normalize_allocation_identifier(policy_key);

    let row = sqlx::query(LOAD_ACTIVE_ALLOCATION_POLICY_SQL)
        .bind(&normalized_policy_key)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_active_allocation_policy_version", error))?;

    row.map(decode_allocation_policy_row).transpose()
}

pub async fn upsert_rebalance_recommendation(
    pool: &PgPool,
    recommendation: &RebalanceRecommendation,
) -> Result<(), AllocationPersistenceError> {
    validate_rebalance_recommendation(recommendation).map_err(map_contract_error)?;
    let result = sqlx::query(UPSERT_REBALANCE_RECOMMENDATION_SQL)
        .bind(normalize_allocation_identifier(
            &recommendation.recommendation_id,
        ))
        .bind(normalize_allocation_identifier(&recommendation.policy_key))
        .bind(recommendation.policy_version)
        .bind(recommendation.status.as_str())
        .bind(recommendation.approval_status.as_str())
        .bind(recommendation.approval_reference.as_deref())
        .bind(&recommendation.action_type)
        .bind(&recommendation.rationale)
        .bind(&recommendation.reason_code)
        .bind(&recommendation.actor_id)
        .bind(&recommendation.correlation_id)
        .bind(&recommendation.parameters)
        .bind(&recommendation.evidence)
        .bind(&recommendation.created_at_utc)
        .bind(&recommendation.updated_at_utc)
        .execute(pool)
        .await
        .map_err(|error| classify_query_error("upsert_rebalance_recommendation", error))?;

    if result.rows_affected() != 1 {
        return Err(AllocationPersistenceError::invalid_payload(
            format!(
                "upsert_rebalance_recommendation expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_pending_rebalance_recommendations(
    pool: &PgPool,
    policy_key: Option<&str>,
) -> Result<Vec<RebalanceRecommendation>, AllocationPersistenceError> {
    let rows = match policy_key {
        Some(policy_key) => {
            validate_non_empty("policy_key", policy_key)?;
            sqlx::query(LOAD_PENDING_REBALANCE_RECOMMENDATIONS_FOR_KEY_SQL)
                .bind(normalize_allocation_identifier(policy_key))
                .fetch_all(pool)
                .await
                .map_err(|error| {
                    classify_query_error("load_pending_rebalance_recommendations_for_key", error)
                })?
        }
        None => sqlx::query(LOAD_PENDING_REBALANCE_RECOMMENDATIONS_SQL)
            .fetch_all(pool)
            .await
            .map_err(|error| {
                classify_query_error("load_pending_rebalance_recommendations", error)
            })?,
    };

    rows.into_iter()
        .map(decode_rebalance_recommendation_row)
        .collect()
}

pub async fn load_latest_rebalance_recommendation_for_policy(
    pool: &PgPool,
    policy_key: &str,
) -> Result<Option<RebalanceRecommendation>, AllocationPersistenceError> {
    validate_non_empty("policy_key", policy_key)?;
    let row = sqlx::query(LOAD_LATEST_REBALANCE_RECOMMENDATION_FOR_POLICY_SQL)
        .bind(normalize_allocation_identifier(policy_key))
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_rebalance_recommendation_for_policy", error)
        })?;

    row.map(decode_rebalance_recommendation_row).transpose()
}

pub async fn load_rebalance_recommendation_by_id(
    pool: &PgPool,
    recommendation_id: &str,
) -> Result<Option<RebalanceRecommendation>, AllocationPersistenceError> {
    validate_non_empty("recommendation_id", recommendation_id)?;
    let row = sqlx::query(LOAD_REBALANCE_RECOMMENDATION_BY_ID_SQL)
        .bind(normalize_allocation_identifier(recommendation_id))
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_rebalance_recommendation_by_id", error))?;

    row.map(decode_rebalance_recommendation_row).transpose()
}

fn decode_allocation_policy_row(
    row: sqlx::postgres::PgRow,
) -> Result<AllocationPolicyVersion, AllocationPersistenceError> {
    let approval_status: String = row.try_get("approval_status").map_err(|error| {
        AllocationPersistenceError::row_decode_failure("approval_status", error)
    })?;
    let approval_status =
        AllocationApprovalStatus::parse(&approval_status).map_err(map_contract_error)?;
    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| AllocationPersistenceError::row_decode_failure("reason_code", error))?;
    RebalanceReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let policy = AllocationPolicyVersion {
        policy_key: row
            .try_get("policy_key")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("policy_key", error))?,
        version: row
            .try_get("version")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("version", error))?,
        portfolio_scope_id: row.try_get("portfolio_scope_id").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("portfolio_scope_id", error)
        })?,
        target_exposure_pct_nav: row.try_get("target_exposure_pct_nav").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("target_exposure_pct_nav", error)
        })?,
        target_relative_alpha_weight: row.try_get("target_relative_alpha_weight").map_err(
            |error| {
                AllocationPersistenceError::row_decode_failure(
                    "target_relative_alpha_weight",
                    error,
                )
            },
        )?,
        exposure_drift_threshold_pct: row.try_get("exposure_drift_threshold_pct").map_err(
            |error| {
                AllocationPersistenceError::row_decode_failure(
                    "exposure_drift_threshold_pct",
                    error,
                )
            },
        )?,
        relative_alpha_drift_threshold_pct: row
            .try_get("relative_alpha_drift_threshold_pct")
            .map_err(|error| {
                AllocationPersistenceError::row_decode_failure(
                    "relative_alpha_drift_threshold_pct",
                    error,
                )
            })?,
        approval_status,
        approval_reference: row.try_get("approval_reference").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("approval_reference", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("actor_id", error))?,
        reason_code,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
        advanced_parameters: row.try_get("advanced_parameters").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("advanced_parameters", error)
        })?,
    };
    validate_allocation_policy_version(&policy).map_err(map_contract_error)?;
    Ok(policy)
}

fn decode_rebalance_recommendation_row(
    row: sqlx::postgres::PgRow,
) -> Result<RebalanceRecommendation, AllocationPersistenceError> {
    let status: String = row
        .try_get("status")
        .map_err(|error| AllocationPersistenceError::row_decode_failure("status", error))?;
    let status = RebalanceRecommendationStatus::parse(&status).map_err(map_contract_error)?;
    let approval_status: String = row.try_get("approval_status").map_err(|error| {
        AllocationPersistenceError::row_decode_failure("approval_status", error)
    })?;
    let approval_status =
        AllocationApprovalStatus::parse(&approval_status).map_err(map_contract_error)?;
    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| AllocationPersistenceError::row_decode_failure("reason_code", error))?;
    RebalanceReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let recommendation = RebalanceRecommendation {
        recommendation_id: row.try_get("recommendation_id").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("recommendation_id", error)
        })?,
        policy_key: row
            .try_get("policy_key")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("policy_key", error))?,
        policy_version: row.try_get("policy_version").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("policy_version", error)
        })?,
        status,
        approval_status,
        approval_reference: row.try_get("approval_reference").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("approval_reference", error)
        })?,
        action_type: row.try_get("action_type").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("action_type", error)
        })?,
        rationale: row
            .try_get("rationale")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("rationale", error))?,
        reason_code,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        parameters: row
            .try_get("parameters")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("parameters", error))?,
        evidence: row
            .try_get("evidence")
            .map_err(|error| AllocationPersistenceError::row_decode_failure("evidence", error))?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            AllocationPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };
    validate_rebalance_recommendation(&recommendation).map_err(map_contract_error)?;
    Ok(recommendation)
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), AllocationPersistenceError> {
    if value.trim().is_empty() {
        return Err(AllocationPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AllocationValidationIssue {
                field,
                code: RebalanceReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn map_contract_error(error: AllocationContractError) -> AllocationPersistenceError {
    AllocationPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> AllocationPersistenceError {
    if is_constraint_error(&error) {
        return AllocationPersistenceError::constraint_violation(operation, error);
    }
    AllocationPersistenceError::query_failure(operation, error)
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

    const ALLOCATION_MIGRATION_SQL: &str = include_str!(
        "../../migrations/20260406113000_allocation_policies_rebalance_recommendations.sql"
    );

    fn sample_policy() -> AllocationPolicyVersion {
        AllocationPolicyVersion {
            policy_key: "portfolio-default".to_string(),
            version: 2,
            portfolio_scope_id: "portfolio::default".to_string(),
            target_exposure_pct_nav: 40.0,
            target_relative_alpha_weight: 1.2,
            exposure_drift_threshold_pct: 10.0,
            relative_alpha_drift_threshold_pct: 15.0,
            approval_status: AllocationApprovalStatus::Approved,
            approval_reference: Some("apr-allocation-policy-001".to_string()),
            actor_id: "ops-1".to_string(),
            reason_code: RebalanceReasonCode::AllocationPolicyUpdated
                .code()
                .to_string(),
            correlation_id: "corr-allocation-001".to_string(),
            updated_at_utc: "2026-04-06T07:00:00Z".to_string(),
            advanced_parameters: json!({
                "execution_window_minutes": 15
            }),
        }
    }

    fn sample_recommendation() -> RebalanceRecommendation {
        RebalanceRecommendation {
            recommendation_id: "reco::portfolio-default::2::corr-allocation-001".to_string(),
            policy_key: "portfolio-default".to_string(),
            policy_version: 2,
            status: RebalanceRecommendationStatus::Proposed,
            approval_status: AllocationApprovalStatus::NotRequired,
            approval_reference: None,
            action_type: "recommend".to_string(),
            rationale: "Exposure drift exceeded configured threshold.".to_string(),
            reason_code: RebalanceReasonCode::RecommendationProposed
                .code()
                .to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-allocation-001".to_string(),
            parameters: json!({
                "target_exposure_pct_nav": 40.0
            }),
            evidence: json!({
                "exposure_drift_pct": 12.5
            }),
            created_at_utc: "2026-04-06T07:00:00Z".to_string(),
            updated_at_utc: "2026-04-06T07:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_story_3_3_schema_scope() {
        assert!(
            ALLOCATION_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS allocation_policies")
        );
        assert!(
            ALLOCATION_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS rebalance_recommendations")
        );
        assert!(!ALLOCATION_MIGRATION_SQL.contains("risk_limit_profiles"));
        assert!(!ALLOCATION_MIGRATION_SQL.contains("market_policy_profiles"));
    }

    #[test]
    fn migration_enforces_threshold_defaults_and_operability_indexes() {
        assert!(
            ALLOCATION_MIGRATION_SQL
                .contains("exposure_drift_threshold_pct DOUBLE PRECISION NOT NULL DEFAULT 10")
        );
        assert!(
            ALLOCATION_MIGRATION_SQL.contains(
                "relative_alpha_drift_threshold_pct DOUBLE PRECISION NOT NULL DEFAULT 15"
            )
        );
        assert!(ALLOCATION_MIGRATION_SQL.contains("idx_allocation_policies_active_lookup"));
        assert!(ALLOCATION_MIGRATION_SQL.contains("idx_allocation_policies_actor_correlation"));
        assert!(ALLOCATION_MIGRATION_SQL.contains("idx_rebalance_recommendations_pending"));
        assert!(ALLOCATION_MIGRATION_SQL.contains("idx_rebalance_recommendations_policy_latest"));
    }

    #[test]
    fn recommendation_queries_remain_deterministic_for_pending_and_latest_lookups() {
        assert!(
            LOAD_PENDING_REBALANCE_RECOMMENDATIONS_SQL
                .contains("ORDER BY policy_key ASC, created_at_utc DESC, recommendation_id DESC")
        );
        assert!(
            LOAD_LATEST_REBALANCE_RECOMMENDATION_FOR_POLICY_SQL
                .contains("ORDER BY created_at_utc DESC, recommendation_id DESC")
        );
    }

    #[test]
    fn policy_validation_rejects_exposure_threshold_above_hundred() {
        let mut policy = sample_policy();
        policy.exposure_drift_threshold_pct = 100.1;

        let error = validate_allocation_policy_version(&policy)
            .expect_err("threshold > 100 should fail validation");
        assert_eq!(error.code, RebalanceReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "exposure_drift_threshold_pct")
        );
    }

    #[test]
    fn recommendation_validation_rejects_pending_approval_with_reference() {
        let mut recommendation = sample_recommendation();
        recommendation.status = RebalanceRecommendationStatus::PendingApproval;
        recommendation.approval_status = AllocationApprovalStatus::Pending;
        recommendation.approval_reference = Some("apr-should-not-exist".to_string());

        let error = validate_rebalance_recommendation(&recommendation)
            .expect_err("pending recommendations cannot carry approval references");
        assert_eq!(error.code, RebalanceReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "approval_reference")
        );
    }

    #[test]
    fn normalize_identifier_trims_and_lowercases() {
        assert_eq!(
            normalize_allocation_identifier(" Portfolio-Default "),
            "portfolio-default"
        );
    }
}
