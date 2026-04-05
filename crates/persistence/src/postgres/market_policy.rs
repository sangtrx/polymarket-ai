use domain::risk::{
    MarketClusterOverride, MarketPolicyContractError, MarketPolicyProfile, MarketPolicyReasonCode,
    MarketPolicyValidationIssue, validate_market_cluster_override, validate_market_policy_profile,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const UPSERT_MARKET_POLICY_PROFILE_SQL: &str = r#"
    INSERT INTO market_policy_profiles (
        profile_id,
        cluster_id,
        min_liquidity_usd,
        max_spread_bps,
        min_reward_score,
        max_exposure_pct_nav,
        is_active,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10::timestamptz
    )
    ON CONFLICT (profile_id)
    DO UPDATE SET
        cluster_id = EXCLUDED.cluster_id,
        min_liquidity_usd = EXCLUDED.min_liquidity_usd,
        max_spread_bps = EXCLUDED.max_spread_bps,
        min_reward_score = EXCLUDED.min_reward_score,
        max_exposure_pct_nav = EXCLUDED.max_exposure_pct_nav,
        is_active = EXCLUDED.is_active,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const UPSERT_MARKET_CLUSTER_OVERRIDE_SQL: &str = r#"
    INSERT INTO market_cluster_overrides (
        cluster_id,
        is_enabled,
        reason_code,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6::timestamptz
    )
    ON CONFLICT (cluster_id)
    DO UPDATE SET
        is_enabled = EXCLUDED.is_enabled,
        reason_code = EXCLUDED.reason_code,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_ACTIVE_MARKET_POLICY_PROFILE_SQL: &str = r#"
    SELECT
        profile_id,
        cluster_id,
        min_liquidity_usd,
        max_spread_bps,
        min_reward_score,
        max_exposure_pct_nav,
        is_active,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM market_policy_profiles
    WHERE lower(trim(cluster_id)) = lower(trim($1))
      AND is_active = TRUE
    ORDER BY updated_at_utc DESC, profile_id ASC
    LIMIT 1
"#;

const LOAD_MARKET_CLUSTER_OVERRIDE_SQL: &str = r#"
    SELECT
        cluster_id,
        is_enabled,
        reason_code,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM market_cluster_overrides
    WHERE lower(trim(cluster_id)) = lower(trim($1))
    ORDER BY updated_at_utc DESC, cluster_id ASC
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketPolicyPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<MarketPolicyValidationIssue>,
}

impl MarketPolicyPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<MarketPolicyValidationIssue>,
    ) -> Self {
        Self {
            code: MarketPolicyReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_policy_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_policy_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_policy_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for MarketPolicyPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for MarketPolicyPersistenceError {}

pub async fn upsert_market_policy_profile<'e, E>(
    executor: E,
    profile: &MarketPolicyProfile,
) -> Result<(), MarketPolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_profile_for_persistence(profile)?;
    let normalized_cluster_id = normalize_cluster_id(&profile.cluster_id);

    let result = sqlx::query(UPSERT_MARKET_POLICY_PROFILE_SQL)
        .bind(&profile.profile_id)
        .bind(&normalized_cluster_id)
        .bind(profile.min_liquidity_usd)
        .bind(profile.max_spread_bps)
        .bind(profile.min_reward_score)
        .bind(profile.max_exposure_pct_nav)
        .bind(profile.is_active)
        .bind(&profile.actor_id)
        .bind(&profile.correlation_id)
        .bind(&profile.updated_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_market_policy_profile", error))?;

    if result.rows_affected() != 1 {
        return Err(MarketPolicyPersistenceError::invalid_payload(
            format!(
                "upsert_market_policy_profile expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn upsert_market_cluster_override<'e, E>(
    executor: E,
    override_state: &MarketClusterOverride,
) -> Result<(), MarketPolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_override_for_persistence(override_state)?;
    let normalized_cluster_id = normalize_cluster_id(&override_state.cluster_id);

    let result = sqlx::query(UPSERT_MARKET_CLUSTER_OVERRIDE_SQL)
        .bind(&normalized_cluster_id)
        .bind(override_state.is_enabled)
        .bind(&override_state.reason_code)
        .bind(&override_state.actor_id)
        .bind(&override_state.correlation_id)
        .bind(&override_state.updated_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_market_cluster_override", error))?;

    if result.rows_affected() != 1 {
        return Err(MarketPolicyPersistenceError::invalid_payload(
            format!(
                "upsert_market_cluster_override expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_active_market_policy_profile<'e, E>(
    executor: E,
    cluster_id: &str,
) -> Result<Option<MarketPolicyProfile>, MarketPolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("cluster_id", cluster_id)?;
    let normalized_cluster_id = normalize_cluster_id(cluster_id);

    let row = sqlx::query(LOAD_ACTIVE_MARKET_POLICY_PROFILE_SQL)
        .bind(&normalized_cluster_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            MarketPolicyPersistenceError::query_failure("load_active_market_policy_profile", error)
        })?;

    row.map(decode_market_policy_profile_row).transpose()
}

pub async fn load_market_cluster_override<'e, E>(
    executor: E,
    cluster_id: &str,
) -> Result<Option<MarketClusterOverride>, MarketPolicyPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("cluster_id", cluster_id)?;
    let normalized_cluster_id = normalize_cluster_id(cluster_id);

    let row = sqlx::query(LOAD_MARKET_CLUSTER_OVERRIDE_SQL)
        .bind(&normalized_cluster_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            MarketPolicyPersistenceError::query_failure("load_market_cluster_override", error)
        })?;

    row.map(decode_market_cluster_override_row).transpose()
}

fn decode_market_policy_profile_row(
    row: sqlx::postgres::PgRow,
) -> Result<MarketPolicyProfile, MarketPolicyPersistenceError> {
    let profile = MarketPolicyProfile {
        profile_id: row.try_get("profile_id").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("profile_id", error)
        })?,
        cluster_id: row.try_get("cluster_id").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("cluster_id", error)
        })?,
        min_liquidity_usd: row.try_get("min_liquidity_usd").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("min_liquidity_usd", error)
        })?,
        max_spread_bps: row.try_get("max_spread_bps").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("max_spread_bps", error)
        })?,
        min_reward_score: row.try_get("min_reward_score").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("min_reward_score", error)
        })?,
        max_exposure_pct_nav: row.try_get("max_exposure_pct_nav").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("max_exposure_pct_nav", error)
        })?,
        is_active: row.try_get("is_active").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("is_active", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| MarketPolicyPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_profile_for_persistence(&profile)?;
    Ok(profile)
}

fn decode_market_cluster_override_row(
    row: sqlx::postgres::PgRow,
) -> Result<MarketClusterOverride, MarketPolicyPersistenceError> {
    let override_state = MarketClusterOverride {
        cluster_id: row.try_get("cluster_id").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("cluster_id", error)
        })?,
        is_enabled: row.try_get("is_enabled").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("is_enabled", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("reason_code", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| MarketPolicyPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            MarketPolicyPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_override_for_persistence(&override_state)?;
    Ok(override_state)
}

fn validate_profile_for_persistence(
    profile: &MarketPolicyProfile,
) -> Result<(), MarketPolicyPersistenceError> {
    validate_market_policy_profile(profile).map_err(map_contract_error)?;
    parse_utc_timestamp(&profile.updated_at_utc)?;
    Ok(())
}

fn validate_override_for_persistence(
    override_state: &MarketClusterOverride,
) -> Result<(), MarketPolicyPersistenceError> {
    validate_market_cluster_override(override_state).map_err(map_contract_error)?;
    MarketPolicyReasonCode::parse(&override_state.reason_code).map_err(map_contract_error)?;
    parse_utc_timestamp(&override_state.updated_at_utc)?;
    Ok(())
}

fn map_contract_error(error: MarketPolicyContractError) -> MarketPolicyPersistenceError {
    MarketPolicyPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), MarketPolicyPersistenceError> {
    if value.trim().is_empty() {
        return Err(MarketPolicyPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![MarketPolicyValidationIssue {
                field,
                code: MarketPolicyReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn normalize_cluster_id(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> MarketPolicyPersistenceError {
    if is_constraint_error(&error) {
        return MarketPolicyPersistenceError::constraint_violation(operation, error);
    }
    MarketPolicyPersistenceError::query_failure(operation, error)
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

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, MarketPolicyPersistenceError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        MarketPolicyPersistenceError::invalid_payload(
            format!("timestamp `{value}` must be RFC3339 UTC"),
            Vec::new(),
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(MarketPolicyPersistenceError::invalid_payload(
            "timestamps must use UTC `Z` offset",
            Vec::new(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKET_POLICY_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406004500_market_policy_profiles.sql");

    fn sample_profile() -> MarketPolicyProfile {
        MarketPolicyProfile {
            profile_id: "policy_cluster_alpha".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            min_liquidity_usd: 1000.0,
            max_spread_bps: 3.5,
            min_reward_score: 0.5,
            max_exposure_pct_nav: 20.0,
            is_active: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_override() -> MarketClusterOverride {
        MarketClusterOverride {
            cluster_id: "cluster_alpha".to_string(),
            is_enabled: true,
            reason_code: MarketPolicyReasonCode::ClusterEnabled.code().to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_market_policy_schema_scope() {
        assert!(
            MARKET_POLICY_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS market_policy_profiles")
        );
        assert!(
            MARKET_POLICY_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS market_cluster_overrides")
        );
        assert!(
            !MARKET_POLICY_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS credential_rotation_events")
        );
    }

    #[test]
    fn migration_enforces_threshold_constraints_and_indexes() {
        assert!(MARKET_POLICY_MIGRATION_SQL.contains("max_exposure_pct_nav >= 0"));
        assert!(MARKET_POLICY_MIGRATION_SQL.contains("max_exposure_pct_nav <= 100"));
        assert!(MARKET_POLICY_MIGRATION_SQL.contains("cluster_id = lower(trim(cluster_id))"));
        assert!(MARKET_POLICY_MIGRATION_SQL.contains("idx_market_policy_profiles_active_lookup"));
        assert!(
            MARKET_POLICY_MIGRATION_SQL
                .contains("idx_market_cluster_overrides_cluster_canonical_unique")
        );
        assert!(MARKET_POLICY_MIGRATION_SQL.contains("idx_market_cluster_overrides_runtime_state"));
    }

    #[test]
    fn profile_validation_rejects_exposure_over_100_with_field_error() {
        let mut profile = sample_profile();
        profile.max_exposure_pct_nav = 100.1;
        let error = validate_profile_for_persistence(&profile)
            .expect_err("profile exposure > 100 should fail validation");

        assert_eq!(error.code, MarketPolicyReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|field| field.field == "max_exposure_pct_nav")
        );
    }

    #[test]
    fn profile_validation_accepts_spread_zero_and_exposure_100() {
        let mut profile = sample_profile();
        profile.max_spread_bps = 0.0;
        profile.max_exposure_pct_nav = 100.0;

        assert!(validate_profile_for_persistence(&profile).is_ok());
    }

    #[test]
    fn cluster_override_validation_rejects_unknown_reason_codes() {
        let mut override_state = sample_override();
        override_state.reason_code = "cluster_runtime_foo".to_string();
        let error = validate_override_for_persistence(&override_state)
            .expect_err("unknown reason code should be rejected");

        assert_eq!(error.code, MarketPolicyReasonCode::InvalidPayload.code());
    }

    #[test]
    fn parse_utc_timestamp_rejects_non_utc_offset() {
        let error = parse_utc_timestamp("2026-04-06T01:00:00+01:00")
            .expect_err("non-UTC offset should fail");
        assert_eq!(error.code, MarketPolicyReasonCode::InvalidPayload.code());
    }

    #[test]
    fn cluster_override_lookup_query_prefers_latest_update_deterministically() {
        assert!(
            LOAD_MARKET_CLUSTER_OVERRIDE_SQL
                .contains("ORDER BY updated_at_utc DESC, cluster_id ASC")
        );
    }

    #[test]
    fn normalize_cluster_id_trims_and_lowercases() {
        assert_eq!(normalize_cluster_id(" Cluster_Alpha "), "cluster_alpha");
    }
}
