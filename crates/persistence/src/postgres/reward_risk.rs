use domain::risk::{
    RewardRiskContractError, RewardRiskPolicy, RewardRiskReasonCode, RewardRiskValidationIssue,
    validate_reward_risk_policy,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const UPSERT_REWARD_RISK_POLICY_SQL: &str = r#"
    INSERT INTO reward_risk_policies (
        policy_key,
        strategy_key,
        min_reward_per_risk,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6::timestamptz
    )
    ON CONFLICT (policy_key)
    DO UPDATE SET
        strategy_key = EXCLUDED.strategy_key,
        min_reward_per_risk = EXCLUDED.min_reward_per_risk,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_REWARD_RISK_POLICY_SQL: &str = r#"
    SELECT
        policy_key,
        strategy_key,
        min_reward_per_risk,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM reward_risk_policies
    WHERE lower(trim(policy_key)) = lower(trim($1))
    ORDER BY updated_at_utc DESC, policy_key ASC
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardRiskPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RewardRiskValidationIssue>,
}

impl RewardRiskPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RewardRiskValidationIssue>,
    ) -> Self {
        Self {
            code: RewardRiskReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "reward_risk_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "reward_risk_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "reward_risk_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RewardRiskPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RewardRiskPersistenceError {}

pub async fn upsert_reward_risk_policy<'e, E>(
    executor: E,
    policy: &RewardRiskPolicy,
) -> Result<(), RewardRiskPersistenceError>
where
    E: PgExecutor<'e>,
{
    let normalized_policy = RewardRiskPolicy {
        policy_key: normalize_identifier(&policy.policy_key),
        strategy_key: normalize_identifier(&policy.strategy_key),
        min_reward_per_risk: policy.min_reward_per_risk,
        actor_id: policy.actor_id.clone(),
        correlation_id: policy.correlation_id.clone(),
        updated_at_utc: policy.updated_at_utc.clone(),
    };
    validate_policy_for_persistence(&normalized_policy)?;

    let result = sqlx::query(UPSERT_REWARD_RISK_POLICY_SQL)
        .bind(&normalized_policy.policy_key)
        .bind(&normalized_policy.strategy_key)
        .bind(normalized_policy.min_reward_per_risk)
        .bind(&normalized_policy.actor_id)
        .bind(&normalized_policy.correlation_id)
        .bind(&normalized_policy.updated_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_reward_risk_policy", error))?;

    if result.rows_affected() != 1 {
        return Err(RewardRiskPersistenceError::invalid_payload(
            format!(
                "upsert_reward_risk_policy expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_reward_risk_policy<'e, E>(
    executor: E,
    policy_key: &str,
) -> Result<Option<RewardRiskPolicy>, RewardRiskPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("policy_key", policy_key)?;
    let normalized_policy_key = normalize_identifier(policy_key);

    let row = sqlx::query(LOAD_REWARD_RISK_POLICY_SQL)
        .bind(&normalized_policy_key)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            RewardRiskPersistenceError::query_failure("load_reward_risk_policy", error)
        })?;

    row.map(decode_reward_risk_policy_row).transpose()
}

fn decode_reward_risk_policy_row(
    row: sqlx::postgres::PgRow,
) -> Result<RewardRiskPolicy, RewardRiskPersistenceError> {
    let policy = RewardRiskPolicy {
        policy_key: row
            .try_get("policy_key")
            .map_err(|error| RewardRiskPersistenceError::row_decode_failure("policy_key", error))?,
        strategy_key: row.try_get("strategy_key").map_err(|error| {
            RewardRiskPersistenceError::row_decode_failure("strategy_key", error)
        })?,
        min_reward_per_risk: row.try_get("min_reward_per_risk").map_err(|error| {
            RewardRiskPersistenceError::row_decode_failure("min_reward_per_risk", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| RewardRiskPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            RewardRiskPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            RewardRiskPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_policy_for_persistence(&policy)?;
    Ok(policy)
}

fn validate_policy_for_persistence(
    policy: &RewardRiskPolicy,
) -> Result<(), RewardRiskPersistenceError> {
    validate_reward_risk_policy(policy).map_err(map_contract_error)?;
    parse_utc_timestamp(&policy.updated_at_utc)?;
    Ok(())
}

fn map_contract_error(error: RewardRiskContractError) -> RewardRiskPersistenceError {
    RewardRiskPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), RewardRiskPersistenceError> {
    if value.trim().is_empty() {
        return Err(RewardRiskPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![RewardRiskValidationIssue {
                field,
                code: RewardRiskReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> RewardRiskPersistenceError {
    if is_constraint_error(&error) {
        return RewardRiskPersistenceError::constraint_violation(operation, error);
    }
    RewardRiskPersistenceError::query_failure(operation, error)
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

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, RewardRiskPersistenceError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        RewardRiskPersistenceError::invalid_payload(
            format!("timestamp `{value}` must be RFC3339 UTC"),
            Vec::new(),
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(RewardRiskPersistenceError::invalid_payload(
            "timestamps must use UTC `Z` offset",
            Vec::new(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REWARD_RISK_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407090000_reward_risk_policies.sql");

    fn sample_policy() -> RewardRiskPolicy {
        RewardRiskPolicy {
            policy_key: "strategy::maker-alpha".to_string(),
            strategy_key: "strategy::maker-alpha".to_string(),
            min_reward_per_risk: 1.2,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-reward-risk-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_reward_risk_schema_scope() {
        assert!(
            REWARD_RISK_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS reward_risk_policies")
        );
        assert!(
            !REWARD_RISK_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS risk_limit_profiles")
        );
    }

    #[test]
    fn migration_enforces_constraints_and_indexes() {
        assert!(REWARD_RISK_MIGRATION_SQL.contains("min_reward_per_risk >= 0"));
        assert!(REWARD_RISK_MIGRATION_SQL.contains("policy_key = lower(trim(policy_key))"));
        assert!(
            REWARD_RISK_MIGRATION_SQL
                .contains("idx_reward_risk_policies_policy_key_canonical_unique")
        );
        assert!(REWARD_RISK_MIGRATION_SQL.contains("idx_reward_risk_policies_strategy_lookup"));
    }

    #[test]
    fn policy_validation_rejects_negative_threshold() {
        let mut policy = sample_policy();
        policy.min_reward_per_risk = -0.1;
        let error = validate_policy_for_persistence(&policy)
            .expect_err("negative threshold should fail validation");

        assert_eq!(error.code, RewardRiskReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|field| field.field == "min_reward_per_risk")
        );
    }

    #[test]
    fn parse_utc_timestamp_rejects_non_utc_offset() {
        let error = parse_utc_timestamp("2026-04-07T01:00:00+01:00")
            .expect_err("non-UTC offset should fail");
        assert_eq!(error.code, RewardRiskReasonCode::InvalidPayload.code());
    }

    #[test]
    fn normalize_identifier_trims_and_lowercases() {
        assert_eq!(
            normalize_identifier(" Strategy::Maker-Alpha "),
            "strategy::maker-alpha"
        );
    }

    #[test]
    fn policy_lookup_query_prefers_latest_update_deterministically() {
        assert!(
            LOAD_REWARD_RISK_POLICY_SQL.contains("ORDER BY updated_at_utc DESC, policy_key ASC")
        );
    }
}
