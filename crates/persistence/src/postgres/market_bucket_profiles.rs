use domain::risk::{
    MarketBucketContractError, MarketBucketProfile, MarketBucketReasonCode,
    MarketBucketValidationIssue, canonicalize_market_bucket_profile,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const UPSERT_MARKET_BUCKET_PROFILE_SQL: &str = r#"
    INSERT INTO market_bucket_profiles (
        profile_id,
        market_id,
        cluster_id,
        bucket_type,
        risk_policy_key,
        allocation_policy_key,
        is_active,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10::timestamptz
    )
    ON CONFLICT (profile_id)
    DO UPDATE SET
        market_id = EXCLUDED.market_id,
        cluster_id = EXCLUDED.cluster_id,
        bucket_type = EXCLUDED.bucket_type,
        risk_policy_key = EXCLUDED.risk_policy_key,
        allocation_policy_key = EXCLUDED.allocation_policy_key,
        is_active = EXCLUDED.is_active,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_ACTIVE_MARKET_BUCKET_PROFILE_SQL: &str = r#"
    SELECT
        profile_id,
        market_id,
        cluster_id,
        bucket_type,
        risk_policy_key,
        allocation_policy_key,
        is_active,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM market_bucket_profiles
    WHERE lower(trim(market_id)) = lower(trim($1))
      AND lower(trim(cluster_id)) = lower(trim($2))
      AND is_active = TRUE
    ORDER BY updated_at_utc DESC, profile_id ASC
    LIMIT 1
"#;

const LIST_ACTIVE_MARKET_BUCKET_PROFILES_SQL: &str = r#"
    SELECT
        profile_id,
        market_id,
        cluster_id,
        bucket_type,
        risk_policy_key,
        allocation_policy_key,
        is_active,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM market_bucket_profiles
    WHERE is_active = TRUE
    ORDER BY market_id ASC, cluster_id ASC, updated_at_utc DESC, profile_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketBucketPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<MarketBucketValidationIssue>,
}

impl MarketBucketPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<MarketBucketValidationIssue>,
    ) -> Self {
        Self {
            code: MarketBucketReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_bucket_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_bucket_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "market_bucket_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for MarketBucketPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for MarketBucketPersistenceError {}

pub async fn upsert_market_bucket_profile<'e, E>(
    executor: E,
    profile: &MarketBucketProfile,
) -> Result<(), MarketBucketPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical_profile = validate_profile_for_persistence(profile)?;

    let result = sqlx::query(UPSERT_MARKET_BUCKET_PROFILE_SQL)
        .bind(&canonical_profile.profile_id)
        .bind(&canonical_profile.market_id)
        .bind(&canonical_profile.cluster_id)
        .bind(&canonical_profile.bucket_type)
        .bind(&canonical_profile.risk_policy_key)
        .bind(&canonical_profile.allocation_policy_key)
        .bind(canonical_profile.is_active)
        .bind(&canonical_profile.actor_id)
        .bind(&canonical_profile.correlation_id)
        .bind(&canonical_profile.updated_at_utc)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_market_bucket_profile", error))?;

    if result.rows_affected() != 1 {
        return Err(MarketBucketPersistenceError::invalid_payload(
            format!(
                "upsert_market_bucket_profile expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_active_market_bucket_profile<'e, E>(
    executor: E,
    market_id: &str,
    cluster_id: &str,
) -> Result<Option<MarketBucketProfile>, MarketBucketPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("market_id", market_id)?;
    validate_non_empty("cluster_id", cluster_id)?;
    let normalized_market_id = normalize_identifier(market_id);
    let normalized_cluster_id = normalize_identifier(cluster_id);

    let row = sqlx::query(LOAD_ACTIVE_MARKET_BUCKET_PROFILE_SQL)
        .bind(&normalized_market_id)
        .bind(&normalized_cluster_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            MarketBucketPersistenceError::query_failure("load_active_market_bucket_profile", error)
        })?;

    row.map(decode_market_bucket_profile_row).transpose()
}

pub async fn list_active_market_bucket_profiles<'e, E>(
    executor: E,
) -> Result<Vec<MarketBucketProfile>, MarketBucketPersistenceError>
where
    E: PgExecutor<'e>,
{
    let rows = sqlx::query(LIST_ACTIVE_MARKET_BUCKET_PROFILES_SQL)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            MarketBucketPersistenceError::query_failure("list_active_market_bucket_profiles", error)
        })?;

    rows.into_iter()
        .map(decode_market_bucket_profile_row)
        .collect()
}

fn decode_market_bucket_profile_row(
    row: sqlx::postgres::PgRow,
) -> Result<MarketBucketProfile, MarketBucketPersistenceError> {
    let profile = MarketBucketProfile {
        profile_id: row.try_get("profile_id").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("profile_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("market_id", error)
        })?,
        cluster_id: row.try_get("cluster_id").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("cluster_id", error)
        })?,
        bucket_type: row.try_get("bucket_type").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("bucket_type", error)
        })?,
        risk_policy_key: row.try_get("risk_policy_key").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("risk_policy_key", error)
        })?,
        allocation_policy_key: row.try_get("allocation_policy_key").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("allocation_policy_key", error)
        })?,
        is_active: row.try_get("is_active").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("is_active", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| MarketBucketPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            MarketBucketPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_profile_for_persistence(&profile)
}

fn validate_profile_for_persistence(
    profile: &MarketBucketProfile,
) -> Result<MarketBucketProfile, MarketBucketPersistenceError> {
    let canonical_profile =
        canonicalize_market_bucket_profile(profile).map_err(map_contract_error)?;
    parse_utc_timestamp(&canonical_profile.updated_at_utc)?;
    Ok(canonical_profile)
}

fn map_contract_error(error: MarketBucketContractError) -> MarketBucketPersistenceError {
    MarketBucketPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), MarketBucketPersistenceError> {
    if value.trim().is_empty() {
        return Err(MarketBucketPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![MarketBucketValidationIssue {
                field,
                code: MarketBucketReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> MarketBucketPersistenceError {
    if is_constraint_error(&error) {
        return MarketBucketPersistenceError::constraint_violation(operation, error);
    }
    MarketBucketPersistenceError::query_failure(operation, error)
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

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, MarketBucketPersistenceError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        MarketBucketPersistenceError::invalid_payload(
            format!("timestamp `{value}` must be RFC3339 UTC"),
            Vec::new(),
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(MarketBucketPersistenceError::invalid_payload(
            "timestamps must use UTC `Z` offset",
            Vec::new(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKET_BUCKET_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407133000_market_bucket_profiles.sql");

    fn sample_profile() -> MarketBucketProfile {
        MarketBucketProfile {
            profile_id: "bucket::market_yes_no_1::cluster_alpha".to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            bucket_type: "core".to_string(),
            risk_policy_key: "core-risk-default".to_string(),
            allocation_policy_key: "core-allocation-default".to_string(),
            is_active: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-bucket-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_market_bucket_schema_scope() {
        assert!(
            MARKET_BUCKET_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS market_bucket_profiles")
        );
        assert!(
            !MARKET_BUCKET_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS market_policy_profiles")
        );
    }

    #[test]
    fn migration_enforces_bucket_constraints_and_active_unique_index() {
        assert!(MARKET_BUCKET_MIGRATION_SQL.contains("bucket_type IN ('core', 'satellite')"));
        assert!(MARKET_BUCKET_MIGRATION_SQL.contains("market_id = lower(trim(market_id))"));
        assert!(MARKET_BUCKET_MIGRATION_SQL.contains("cluster_id = lower(trim(cluster_id))"));
        assert!(
            MARKET_BUCKET_MIGRATION_SQL
                .contains("idx_market_bucket_profiles_active_market_cluster_unique")
        );
    }

    #[test]
    fn profile_validation_rejects_unsupported_bucket_type() {
        let mut profile = sample_profile();
        profile.bucket_type = "growth".to_string();
        let error = validate_profile_for_persistence(&profile)
            .expect_err("unsupported bucket type should fail validation");

        assert_eq!(error.code, MarketBucketReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|field| field.field == "bucket_type")
        );
    }

    #[test]
    fn profile_validation_canonicalizes_identifiers() {
        let mut profile = sample_profile();
        profile.market_id = " Market_Yes_No_1 ".to_string();
        profile.cluster_id = " Cluster_Alpha ".to_string();
        profile.profile_id = " Bucket::Market_Yes_No_1::Cluster_Alpha ".to_string();
        profile.risk_policy_key = " Core-Risk-Default ".to_string();
        profile.allocation_policy_key = " Core-Allocation-Default ".to_string();
        let canonical = validate_profile_for_persistence(&profile)
            .expect("canonical formatting should be produced for persisted profiles");

        assert_eq!(
            canonical.profile_id,
            "bucket::market_yes_no_1::cluster_alpha"
        );
        assert_eq!(canonical.market_id, "market_yes_no_1");
        assert_eq!(canonical.cluster_id, "cluster_alpha");
        assert_eq!(canonical.risk_policy_key, "core-risk-default");
        assert_eq!(canonical.allocation_policy_key, "core-allocation-default");
    }

    #[test]
    fn parse_utc_timestamp_rejects_non_utc_offset() {
        let error = parse_utc_timestamp("2026-04-07T01:00:00+01:00")
            .expect_err("non-UTC offset should fail");
        assert_eq!(error.code, MarketBucketReasonCode::InvalidPayload.code());
    }

    #[test]
    fn load_active_lookup_query_prefers_latest_update_deterministically() {
        assert!(
            LOAD_ACTIVE_MARKET_BUCKET_PROFILE_SQL
                .contains("ORDER BY updated_at_utc DESC, profile_id ASC")
        );
    }

    #[test]
    fn normalize_identifier_trims_and_lowercases() {
        assert_eq!(normalize_identifier(" Market_Yes_No_1 "), "market_yes_no_1");
    }
}
