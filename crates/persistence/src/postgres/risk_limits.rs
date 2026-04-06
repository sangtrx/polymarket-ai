use domain::risk::{
    InventoryLimitRule, RiskLimitContractError, RiskLimitProfileStatus, RiskLimitProfileVersion,
    RiskLimitReasonCode, RiskLimitScope, RiskLimitValidationIssue, RiskScopeLimit,
    normalize_risk_limit_identifier, validate_inventory_limit_rule,
    validate_risk_limit_profile_version,
};
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

const DEACTIVATE_ACTIVE_PROFILE_SQL: &str = r#"
    UPDATE risk_limit_profiles
    SET
        approval_status = 'denied',
        is_active = FALSE,
        reason_code = $2,
        updated_at_utc = $3::timestamptz
    WHERE profile_key = $1
      AND is_active = TRUE
"#;

const UPSERT_RISK_LIMIT_SCOPE_SQL: &str = r#"
    INSERT INTO risk_limit_profiles (
        profile_key,
        version,
        scope_type,
        scope_id,
        max_notional_usd,
        max_inventory_units,
        max_concentration_pct_nav,
        approval_status,
        is_active,
        approval_reference,
        actor_id,
        reason_code,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14::timestamptz
    )
    ON CONFLICT (profile_key, version, scope_type)
    DO UPDATE SET
        scope_id = EXCLUDED.scope_id,
        max_notional_usd = EXCLUDED.max_notional_usd,
        max_inventory_units = EXCLUDED.max_inventory_units,
        max_concentration_pct_nav = EXCLUDED.max_concentration_pct_nav,
        approval_status = EXCLUDED.approval_status,
        is_active = EXCLUDED.is_active,
        approval_reference = EXCLUDED.approval_reference,
        actor_id = EXCLUDED.actor_id,
        reason_code = EXCLUDED.reason_code,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const DELETE_INVENTORY_RULES_SQL: &str = r#"
    DELETE FROM inventory_limit_rules
    WHERE profile_key = $1
      AND profile_version = $2
"#;

const UPSERT_INVENTORY_RULE_SQL: &str = r#"
    INSERT INTO inventory_limit_rules (
        rule_id,
        profile_key,
        profile_version,
        scope_type,
        scope_id,
        max_position_units,
        max_order_size_units,
        max_concentration_pct_nav,
        actor_id,
        correlation_id,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz
    )
    ON CONFLICT (rule_id)
    DO UPDATE SET
        profile_key = EXCLUDED.profile_key,
        profile_version = EXCLUDED.profile_version,
        scope_type = EXCLUDED.scope_type,
        scope_id = EXCLUDED.scope_id,
        max_position_units = EXCLUDED.max_position_units,
        max_order_size_units = EXCLUDED.max_order_size_units,
        max_concentration_pct_nav = EXCLUDED.max_concentration_pct_nav,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        updated_at_utc = EXCLUDED.updated_at_utc
"#;

const LOAD_ACTIVE_PROFILE_ROWS_SQL: &str = r#"
    WITH latest_active AS (
        SELECT version
        FROM risk_limit_profiles
        WHERE profile_key = $1
          AND is_active = TRUE
        ORDER BY version DESC, updated_at_utc DESC
        LIMIT 1
    )
    SELECT
        profile_key,
        version,
        scope_type,
        scope_id,
        max_notional_usd,
        max_inventory_units,
        max_concentration_pct_nav,
        approval_status,
        approval_reference,
        actor_id,
        reason_code,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM risk_limit_profiles
    WHERE profile_key = $1
      AND version = (SELECT version FROM latest_active)
    ORDER BY scope_type ASC
"#;

const LOAD_PENDING_PROFILE_ROWS_SQL: &str = r#"
    SELECT
        profile_key,
        version,
        scope_type,
        scope_id,
        max_notional_usd,
        max_inventory_units,
        max_concentration_pct_nav,
        approval_status,
        approval_reference,
        actor_id,
        reason_code,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM risk_limit_profiles
    WHERE approval_status = 'pending'
    ORDER BY profile_key ASC, version DESC, scope_type ASC
"#;

const LOAD_PENDING_PROFILE_ROWS_FOR_KEY_SQL: &str = r#"
    SELECT
        profile_key,
        version,
        scope_type,
        scope_id,
        max_notional_usd,
        max_inventory_units,
        max_concentration_pct_nav,
        approval_status,
        approval_reference,
        actor_id,
        reason_code,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM risk_limit_profiles
    WHERE approval_status = 'pending'
      AND profile_key = $1
    ORDER BY version DESC, scope_type ASC
"#;

const LOAD_INVENTORY_RULES_SQL: &str = r#"
    SELECT
        rule_id,
        profile_key,
        profile_version,
        scope_type,
        scope_id,
        max_position_units,
        max_order_size_units,
        max_concentration_pct_nav,
        actor_id,
        correlation_id,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM inventory_limit_rules
    WHERE profile_key = $1
      AND profile_version = $2
    ORDER BY scope_type ASC, scope_id ASC, rule_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskLimitPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RiskLimitValidationIssue>,
}

impl RiskLimitPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RiskLimitValidationIssue>,
    ) -> Self {
        Self {
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "risk_limit_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "risk_limit_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "risk_limit_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RiskLimitPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RiskLimitPersistenceError {}

#[derive(Debug, Clone, PartialEq)]
pub struct RiskLimitProfileBundle {
    pub profile: RiskLimitProfileVersion,
    pub inventory_rules: Vec<InventoryLimitRule>,
}

pub async fn upsert_risk_limit_profile_bundle(
    pool: &PgPool,
    profile: &RiskLimitProfileVersion,
    inventory_rules: &[InventoryLimitRule],
) -> Result<(), RiskLimitPersistenceError> {
    validate_bundle_for_persistence(profile, inventory_rules)?;

    let profile_key = normalize_risk_limit_identifier(&profile.profile_key);
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_risk_limit_bundle_tx", error))?;

    if profile.status == RiskLimitProfileStatus::Active {
        sqlx::query(DEACTIVATE_ACTIVE_PROFILE_SQL)
            .bind(&profile_key)
            .bind(RiskLimitReasonCode::ProfileDenied.code())
            .bind(&profile.updated_at_utc)
            .execute(&mut *tx)
            .await
            .map_err(|error| classify_query_error("deactivate_prior_active_scope_rows", error))?;
    }

    let is_active = profile.status == RiskLimitProfileStatus::Active;
    for scope_limit in [&profile.portfolio, &profile.market, &profile.strategy] {
        let result = sqlx::query(UPSERT_RISK_LIMIT_SCOPE_SQL)
            .bind(&profile_key)
            .bind(profile.version)
            .bind(scope_limit.scope.as_str())
            .bind(normalize_risk_limit_identifier(&scope_limit.scope_id))
            .bind(scope_limit.max_notional_usd)
            .bind(scope_limit.max_inventory_units)
            .bind(scope_limit.max_concentration_pct_nav)
            .bind(profile.status.as_str())
            .bind(is_active)
            .bind(profile.approval_reference.as_deref())
            .bind(&profile.actor_id)
            .bind(&profile.reason_code)
            .bind(&profile.correlation_id)
            .bind(&profile.updated_at_utc)
            .execute(&mut *tx)
            .await
            .map_err(|error| classify_query_error("upsert_risk_limit_scope_row", error))?;
        if result.rows_affected() != 1 {
            return Err(RiskLimitPersistenceError::invalid_payload(
                format!(
                    "upsert_risk_limit_scope_row expected 1 affected row, got {}",
                    result.rows_affected()
                ),
                Vec::new(),
            ));
        }
    }

    sqlx::query(DELETE_INVENTORY_RULES_SQL)
        .bind(&profile_key)
        .bind(profile.version)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            classify_query_error("delete_inventory_rules_for_profile_version", error)
        })?;

    for rule in inventory_rules {
        let result = sqlx::query(UPSERT_INVENTORY_RULE_SQL)
            .bind(normalize_risk_limit_identifier(&rule.rule_id))
            .bind(&profile_key)
            .bind(profile.version)
            .bind(rule.scope.as_str())
            .bind(normalize_risk_limit_identifier(&rule.scope_id))
            .bind(rule.max_position_units)
            .bind(rule.max_order_size_units)
            .bind(rule.max_concentration_pct_nav)
            .bind(&rule.actor_id)
            .bind(&rule.correlation_id)
            .bind(&rule.updated_at_utc)
            .execute(&mut *tx)
            .await
            .map_err(|error| classify_query_error("upsert_inventory_limit_rule", error))?;
        if result.rows_affected() != 1 {
            return Err(RiskLimitPersistenceError::invalid_payload(
                format!(
                    "upsert_inventory_limit_rule expected 1 affected row, got {}",
                    result.rows_affected()
                ),
                Vec::new(),
            ));
        }
    }

    tx.commit()
        .await
        .map_err(|error| classify_query_error("commit_risk_limit_bundle_tx", error))?;
    Ok(())
}

pub async fn load_active_risk_limit_profile_bundle(
    pool: &PgPool,
    profile_key: &str,
) -> Result<Option<RiskLimitProfileBundle>, RiskLimitPersistenceError> {
    validate_non_empty("profile_key", profile_key)?;
    let normalized_key = normalize_risk_limit_identifier(profile_key);

    let rows = sqlx::query(LOAD_ACTIVE_PROFILE_ROWS_SQL)
        .bind(&normalized_key)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_active_risk_limit_profile_rows", error))?;
    if rows.is_empty() {
        return Ok(None);
    }

    let profile = decode_profile_rows(rows)?;
    let inventory_rules =
        load_inventory_rules_for_profile(pool, &profile.profile_key, profile.version).await?;
    Ok(Some(RiskLimitProfileBundle {
        profile,
        inventory_rules,
    }))
}

pub async fn load_pending_risk_limit_profile_bundles(
    pool: &PgPool,
    profile_key: Option<&str>,
) -> Result<Vec<RiskLimitProfileBundle>, RiskLimitPersistenceError> {
    let rows = match profile_key {
        Some(value) => {
            validate_non_empty("profile_key", value)?;
            sqlx::query(LOAD_PENDING_PROFILE_ROWS_FOR_KEY_SQL)
                .bind(normalize_risk_limit_identifier(value))
                .fetch_all(pool)
                .await
                .map_err(|error| {
                    classify_query_error("load_pending_risk_limit_profile_rows_by_key", error)
                })?
        }
        None => sqlx::query(LOAD_PENDING_PROFILE_ROWS_SQL)
            .fetch_all(pool)
            .await
            .map_err(|error| classify_query_error("load_pending_risk_limit_profile_rows", error))?,
    };

    let mut grouped = BTreeMap::<(String, i64), Vec<DecodedScopeRow>>::new();
    for row in rows {
        let decoded = decode_scope_row(row)?;
        grouped
            .entry((decoded.profile_key.clone(), decoded.version))
            .or_default()
            .push(decoded);
    }

    let mut bundles = Vec::new();
    for ((group_profile_key, group_version), scoped_rows) in grouped {
        let profile = decode_profile_from_scope_rows(scoped_rows)?;
        if profile.profile_key != group_profile_key || profile.version != group_version {
            return Err(RiskLimitPersistenceError::invalid_payload(
                "decoded pending profile grouping drifted unexpectedly",
                Vec::new(),
            ));
        }
        let inventory_rules =
            load_inventory_rules_for_profile(pool, &profile.profile_key, profile.version).await?;
        bundles.push(RiskLimitProfileBundle {
            profile,
            inventory_rules,
        });
    }

    Ok(bundles)
}

async fn load_inventory_rules_for_profile(
    pool: &PgPool,
    profile_key: &str,
    profile_version: i64,
) -> Result<Vec<InventoryLimitRule>, RiskLimitPersistenceError> {
    let rows = sqlx::query(LOAD_INVENTORY_RULES_SQL)
        .bind(profile_key)
        .bind(profile_version)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_inventory_rules_for_profile", error))?;

    rows.into_iter().map(decode_inventory_rule_row).collect()
}

fn decode_profile_rows(
    rows: Vec<sqlx::postgres::PgRow>,
) -> Result<RiskLimitProfileVersion, RiskLimitPersistenceError> {
    let decoded_rows = rows
        .into_iter()
        .map(decode_scope_row)
        .collect::<Result<Vec<_>, _>>()?;
    decode_profile_from_scope_rows(decoded_rows)
}

fn decode_profile_from_scope_rows(
    scoped_rows: Vec<DecodedScopeRow>,
) -> Result<RiskLimitProfileVersion, RiskLimitPersistenceError> {
    if scoped_rows.len() != 3 {
        return Err(RiskLimitPersistenceError::invalid_payload(
            "risk limit profile must contain exactly portfolio, market, and strategy scope rows",
            Vec::new(),
        ));
    }

    let mut portfolio: Option<RiskScopeLimit> = None;
    let mut market: Option<RiskScopeLimit> = None;
    let mut strategy: Option<RiskScopeLimit> = None;

    for row in &scoped_rows {
        let scope_limit = RiskScopeLimit {
            scope: row.scope,
            scope_id: row.scope_id.clone(),
            max_notional_usd: row.max_notional_usd,
            max_inventory_units: row.max_inventory_units,
            max_concentration_pct_nav: row.max_concentration_pct_nav,
        };
        match row.scope {
            RiskLimitScope::Portfolio => portfolio = Some(scope_limit),
            RiskLimitScope::Market => market = Some(scope_limit),
            RiskLimitScope::Strategy => strategy = Some(scope_limit),
        }
    }

    let first = &scoped_rows[0];
    let profile = RiskLimitProfileVersion {
        profile_key: first.profile_key.clone(),
        version: first.version,
        portfolio: portfolio.ok_or_else(|| {
            RiskLimitPersistenceError::invalid_payload("missing portfolio scope row", Vec::new())
        })?,
        market: market.ok_or_else(|| {
            RiskLimitPersistenceError::invalid_payload("missing market scope row", Vec::new())
        })?,
        strategy: strategy.ok_or_else(|| {
            RiskLimitPersistenceError::invalid_payload("missing strategy scope row", Vec::new())
        })?,
        status: first.status,
        approval_reference: first.approval_reference.clone(),
        actor_id: first.actor_id.clone(),
        reason_code: first.reason_code.clone(),
        correlation_id: first.correlation_id.clone(),
        updated_at_utc: first.updated_at_utc.clone(),
    };

    validate_risk_limit_profile_version(&profile).map_err(map_contract_error)?;
    Ok(profile)
}

fn decode_scope_row(
    row: sqlx::postgres::PgRow,
) -> Result<DecodedScopeRow, RiskLimitPersistenceError> {
    let scope_type: String = row
        .try_get("scope_type")
        .map_err(|error| RiskLimitPersistenceError::row_decode_failure("scope_type", error))?;
    let scope = RiskLimitScope::parse(&scope_type).map_err(map_contract_error)?;
    let status: String = row
        .try_get("approval_status")
        .map_err(|error| RiskLimitPersistenceError::row_decode_failure("approval_status", error))?;
    let status = RiskLimitProfileStatus::parse(&status).map_err(map_contract_error)?;
    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| RiskLimitPersistenceError::row_decode_failure("reason_code", error))?;
    RiskLimitReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    Ok(DecodedScopeRow {
        profile_key: row
            .try_get("profile_key")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("profile_key", error))?,
        version: row
            .try_get("version")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("version", error))?,
        scope,
        scope_id: row
            .try_get("scope_id")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("scope_id", error))?,
        max_notional_usd: row.try_get("max_notional_usd").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("max_notional_usd", error)
        })?,
        max_inventory_units: row.try_get("max_inventory_units").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("max_inventory_units", error)
        })?,
        max_concentration_pct_nav: row.try_get("max_concentration_pct_nav").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("max_concentration_pct_nav", error)
        })?,
        status,
        approval_reference: row.try_get("approval_reference").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("approval_reference", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("actor_id", error))?,
        reason_code,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    })
}

fn decode_inventory_rule_row(
    row: sqlx::postgres::PgRow,
) -> Result<InventoryLimitRule, RiskLimitPersistenceError> {
    let scope_type: String = row
        .try_get("scope_type")
        .map_err(|error| RiskLimitPersistenceError::row_decode_failure("scope_type", error))?;
    let scope = RiskLimitScope::parse(&scope_type).map_err(map_contract_error)?;
    let rule = InventoryLimitRule {
        rule_id: row
            .try_get("rule_id")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("rule_id", error))?,
        profile_key: row
            .try_get("profile_key")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("profile_key", error))?,
        profile_version: row.try_get("profile_version").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("profile_version", error)
        })?,
        scope,
        scope_id: row
            .try_get("scope_id")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("scope_id", error))?,
        max_position_units: row.try_get("max_position_units").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("max_position_units", error)
        })?,
        max_order_size_units: row.try_get("max_order_size_units").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("max_order_size_units", error)
        })?,
        max_concentration_pct_nav: row.try_get("max_concentration_pct_nav").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("max_concentration_pct_nav", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| RiskLimitPersistenceError::row_decode_failure("actor_id", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            RiskLimitPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };

    validate_inventory_limit_rule(&rule).map_err(map_contract_error)?;
    Ok(rule)
}

fn validate_bundle_for_persistence(
    profile: &RiskLimitProfileVersion,
    inventory_rules: &[InventoryLimitRule],
) -> Result<(), RiskLimitPersistenceError> {
    validate_risk_limit_profile_version(profile).map_err(map_contract_error)?;

    for rule in inventory_rules {
        validate_inventory_limit_rule(rule).map_err(map_contract_error)?;
        if normalize_risk_limit_identifier(&rule.profile_key)
            != normalize_risk_limit_identifier(&profile.profile_key)
        {
            return Err(RiskLimitPersistenceError::invalid_payload(
                "inventory rule profile_key must match persisted profile_key",
                Vec::new(),
            ));
        }
        if rule.profile_version != profile.version {
            return Err(RiskLimitPersistenceError::invalid_payload(
                "inventory rule profile_version must match persisted profile version",
                Vec::new(),
            ));
        }
    }

    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), RiskLimitPersistenceError> {
    if value.trim().is_empty() {
        return Err(RiskLimitPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

fn map_contract_error(error: RiskLimitContractError) -> RiskLimitPersistenceError {
    RiskLimitPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> RiskLimitPersistenceError {
    if is_constraint_error(&error) {
        return RiskLimitPersistenceError::constraint_violation(operation, error);
    }
    RiskLimitPersistenceError::query_failure(operation, error)
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

#[derive(Debug, Clone)]
struct DecodedScopeRow {
    profile_key: String,
    version: i64,
    scope: RiskLimitScope,
    scope_id: String,
    max_notional_usd: f64,
    max_inventory_units: f64,
    max_concentration_pct_nav: f64,
    status: RiskLimitProfileStatus,
    approval_reference: Option<String>,
    actor_id: String,
    reason_code: String,
    correlation_id: String,
    updated_at_utc: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const RISK_LIMIT_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406072000_risk_limit_profiles.sql");

    fn sample_scope_limit(scope: RiskLimitScope, scope_id: &str, limit: f64) -> RiskScopeLimit {
        RiskScopeLimit {
            scope,
            scope_id: scope_id.to_string(),
            max_notional_usd: limit,
            max_inventory_units: limit,
            max_concentration_pct_nav: limit.min(100.0),
        }
    }

    fn sample_profile() -> RiskLimitProfileVersion {
        RiskLimitProfileVersion {
            profile_key: "default".to_string(),
            version: 2,
            portfolio: sample_scope_limit(RiskLimitScope::Portfolio, "portfolio::default", 1000.0),
            market: sample_scope_limit(RiskLimitScope::Market, "market::sports", 700.0),
            strategy: sample_scope_limit(RiskLimitScope::Strategy, "strategy::maker", 300.0),
            status: RiskLimitProfileStatus::Active,
            approval_reference: Some("apr_risk_limit_increase_req_1".to_string()),
            actor_id: "ops-1".to_string(),
            reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_inventory_rule() -> InventoryLimitRule {
        InventoryLimitRule {
            rule_id: "rule::sports::maker".to_string(),
            profile_key: "default".to_string(),
            profile_version: 2,
            scope: RiskLimitScope::Market,
            scope_id: "market::sports".to_string(),
            max_position_units: 250.0,
            max_order_size_units: 40.0,
            max_concentration_pct_nav: 30.0,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_risk_limit_schema_scope() {
        assert!(
            RISK_LIMIT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS risk_limit_profiles")
        );
        assert!(
            RISK_LIMIT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS inventory_limit_rules")
        );
        assert!(!RISK_LIMIT_MIGRATION_SQL.contains("pretrade_gate_decisions"));
        assert!(!RISK_LIMIT_MIGRATION_SQL.contains("safety_control_actions"));
    }

    #[test]
    fn migration_enforces_constraints_and_operability_indexes() {
        assert!(
            RISK_LIMIT_MIGRATION_SQL.contains("scope_type IN ('portfolio', 'market', 'strategy')")
        );
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("scope_type IN ('market', 'strategy')"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("isfinite(max_notional_usd)"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("max_concentration_pct_nav <= 100"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("idx_risk_limit_profiles_active_lookup"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("idx_risk_limit_profiles_scope_version"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("idx_risk_limit_profiles_pending_approval"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("idx_risk_limit_profiles_actor_correlation"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("idx_risk_limit_profiles_latest_effective"));
        assert!(RISK_LIMIT_MIGRATION_SQL.contains("idx_inventory_limit_rules_profile_lookup"));
    }

    #[test]
    fn active_and_pending_queries_remain_deterministic() {
        assert!(LOAD_ACTIVE_PROFILE_ROWS_SQL.contains("WHERE profile_key = $1"));
        assert!(LOAD_ACTIVE_PROFILE_ROWS_SQL.contains("is_active = TRUE"));
        assert!(LOAD_ACTIVE_PROFILE_ROWS_SQL.contains("ORDER BY version DESC"));
        assert!(LOAD_PENDING_PROFILE_ROWS_SQL.contains("approval_status = 'pending'"));
        assert!(LOAD_PENDING_PROFILE_ROWS_SQL.contains("ORDER BY profile_key ASC, version DESC"));
    }

    #[test]
    fn bundle_validation_rejects_inventory_profile_mismatch() {
        let profile = sample_profile();
        let mut rule = sample_inventory_rule();
        rule.profile_version = profile.version + 1;

        let error = validate_bundle_for_persistence(&profile, &[rule])
            .expect_err("mismatched profile_version should fail validation");
        assert_eq!(error.code, RiskLimitReasonCode::InvalidPayload.code());
    }

    #[test]
    fn bundle_validation_accepts_canonical_payload() {
        let profile = sample_profile();
        let rule = sample_inventory_rule();
        assert!(validate_bundle_for_persistence(&profile, &[rule]).is_ok());
    }
}
