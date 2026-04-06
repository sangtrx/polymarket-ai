use domain::risk::{
    InventoryLimitRule, RiskLimitProfileStatus, RiskLimitProfileVersion, RiskLimitReasonCode,
    RiskLimitScope, RiskLimitValidationIssue, RiskScopeLimit, normalize_risk_limit_identifier,
    requires_risk_limit_increase_approval, validate_inventory_limit_rule,
    validate_risk_limit_profile_version,
};
use persistence::postgres::risk_limits::{
    RiskLimitPersistenceError, RiskLimitProfileBundle,
    load_active_risk_limit_profile_bundle as pg_load_active_profile_bundle,
    load_pending_risk_limit_profile_bundles as pg_load_pending_profile_bundles,
    upsert_risk_limit_profile_bundle as pg_upsert_profile_bundle,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskLimitServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<RiskLimitValidationIssue>,
}

impl RiskLimitServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RiskLimitValidationIssue>,
    ) -> Self {
        Self {
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: "risk_limit_unauthorized_role",
            message: "actor role is not authorized for risk limit workflows".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RiskLimitReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RiskLimitServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RiskLimitServiceError {}

fn map_persistence_error(error: RiskLimitPersistenceError) -> RiskLimitServiceError {
    match error.code {
        "risk_limit_query_failed" | "risk_limit_row_decode_failed" => {
            RiskLimitServiceError::persistence_unavailable(error.message)
        }
        _ => RiskLimitServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct RiskLimitRuleInput {
    pub scope: RiskLimitScope,
    pub scope_id: String,
    pub max_position_units: f64,
    pub max_order_size_units: f64,
    pub max_concentration_pct_nav: f64,
}

#[derive(Debug, Clone)]
pub struct UpsertRiskLimitProfileInput {
    pub actor_id: String,
    pub actor_role: String,
    pub profile_key: String,
    pub version: i64,
    pub portfolio_scope_id: String,
    pub market_scope_id: String,
    pub strategy_scope_id: String,
    pub portfolio_max_notional_usd: f64,
    pub market_max_notional_usd: f64,
    pub strategy_max_notional_usd: f64,
    pub portfolio_max_inventory_units: f64,
    pub market_max_inventory_units: f64,
    pub strategy_max_inventory_units: f64,
    pub portfolio_max_concentration_pct_nav: f64,
    pub market_max_concentration_pct_nav: f64,
    pub strategy_max_concentration_pct_nav: f64,
    pub inventory_rules: Vec<RiskLimitRuleInput>,
    pub correlation_id: String,
    pub updated_at_utc: String,
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingRiskLimitProfilesInput {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
    pub profile_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RiskLimitProfileMutationEvidence {
    pub profile_key: String,
    pub version: i64,
    pub status: String,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    pub inventory_rule_count: usize,
}

pub trait RiskLimitRepositoryPort: Send + Sync {
    fn upsert_profile_bundle(
        &self,
        bundle: RiskLimitProfileBundle,
    ) -> Result<(), RiskLimitServiceError>;
    fn load_active_profile_bundle(
        &self,
        profile_key: &str,
    ) -> Result<Option<RiskLimitProfileBundle>, RiskLimitServiceError>;
    fn load_pending_profile_bundles(
        &self,
        profile_key: Option<&str>,
    ) -> Result<Vec<RiskLimitProfileBundle>, RiskLimitServiceError>;
}

pub trait RiskLimitOrchestrator: Send + Sync {
    fn upsert_risk_limit_profile(
        &self,
        input: UpsertRiskLimitProfileInput,
    ) -> Result<RiskLimitProfileMutationEvidence, RiskLimitServiceError>;
    fn list_pending_risk_limit_profiles(
        &self,
        input: PendingRiskLimitProfilesInput,
    ) -> Result<Vec<RiskLimitProfileMutationEvidence>, RiskLimitServiceError>;
}

#[derive(Clone)]
pub struct RiskLimitService {
    repository: Arc<dyn RiskLimitRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl RiskLimitService {
    pub fn new(repository: Arc<dyn RiskLimitRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryRiskLimitRepository::default()))
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresRiskLimitRepository::new(pool)))
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, RiskLimitServiceError> {
        self.operation_lock.lock().map_err(|_| {
            RiskLimitServiceError::persistence_unavailable(
                "risk limit operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for RiskLimitService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl RiskLimitOrchestrator for RiskLimitService {
    fn upsert_risk_limit_profile(
        &self,
        input: UpsertRiskLimitProfileInput,
    ) -> Result<RiskLimitProfileMutationEvidence, RiskLimitServiceError> {
        if let Err(error) = validate_risk_limit_role(&input.actor_role) {
            emit_risk_limit_telemetry(
                "risk_limit_profile_upsert_v1",
                "deny",
                &input.actor_id,
                &input.profile_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
                None,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("profile_key", &input.profile_key)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("updated_at_utc", &input.updated_at_utc)?;

        let _lock = self.lock_operations()?;
        let normalized_profile_key = normalize_risk_limit_identifier(&input.profile_key);
        let active_bundle = self
            .repository
            .load_active_profile_bundle(&normalized_profile_key)?;

        let mut profile = build_profile_version(
            &input,
            normalized_profile_key.clone(),
            RiskLimitProfileStatus::Active,
            RiskLimitReasonCode::ProfileApplied.code().to_string(),
            input.approval_reference.clone(),
        );
        let mut inventory_rules = build_inventory_rules(&input, &normalized_profile_key);

        let critical_increase = requires_risk_limit_increase_approval(
            active_bundle.as_ref().map(|bundle| &bundle.profile),
            &profile,
        );
        if critical_increase && profile.approval_reference.is_none() {
            profile.status = RiskLimitProfileStatus::Pending;
            profile.reason_code = RiskLimitReasonCode::ApprovalRequired.code().to_string();
            emit_risk_limit_telemetry(
                "risk_limit_profile_upsert_v1",
                "pending",
                &input.actor_id,
                &normalized_profile_key,
                &profile.reason_code,
                &input.correlation_id,
                &input.updated_at_utc,
                None,
            );
        } else {
            emit_risk_limit_telemetry(
                "risk_limit_profile_upsert_v1",
                "allow",
                &input.actor_id,
                &normalized_profile_key,
                &profile.reason_code,
                &input.correlation_id,
                &input.updated_at_utc,
                profile.approval_reference.as_deref(),
            );
        }

        validate_risk_limit_profile_version(&profile).map_err(|error| {
            RiskLimitServiceError::invalid_payload(error.message, error.field_errors.clone())
        })?;
        for rule in &inventory_rules {
            validate_inventory_limit_rule(rule).map_err(|error| {
                RiskLimitServiceError::invalid_payload(error.message, error.field_errors.clone())
            })?;
        }

        self.repository
            .upsert_profile_bundle(RiskLimitProfileBundle {
                profile: profile.clone(),
                inventory_rules: std::mem::take(&mut inventory_rules),
            })
            .inspect_err(|error| {
                emit_risk_limit_telemetry(
                    "risk_limit_profile_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_profile_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                    None,
                );
            })?;

        Ok(RiskLimitProfileMutationEvidence {
            profile_key: profile.profile_key,
            version: profile.version,
            status: profile.status.as_str().to_string(),
            actor_id: profile.actor_id,
            reason_code: profile.reason_code,
            correlation_id: profile.correlation_id,
            updated_at_utc: profile.updated_at_utc,
            approval_reference: profile.approval_reference,
            inventory_rule_count: input.inventory_rules.len(),
        })
    }

    fn list_pending_risk_limit_profiles(
        &self,
        input: PendingRiskLimitProfilesInput,
    ) -> Result<Vec<RiskLimitProfileMutationEvidence>, RiskLimitServiceError> {
        validate_risk_limit_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;

        let profile_key = input
            .profile_key
            .as_deref()
            .map(normalize_risk_limit_identifier);
        let pending = self
            .repository
            .load_pending_profile_bundles(profile_key.as_deref())?;

        emit_risk_limit_telemetry(
            "risk_limit_pending_query_v1",
            "allow",
            &input.actor_id,
            profile_key.as_deref().unwrap_or("all_profiles"),
            RiskLimitReasonCode::ProfilePendingApproval.code(),
            &input.correlation_id,
            &input.queried_at_utc,
            None,
        );

        Ok(pending
            .into_iter()
            .map(|bundle| RiskLimitProfileMutationEvidence {
                profile_key: bundle.profile.profile_key,
                version: bundle.profile.version,
                status: bundle.profile.status.as_str().to_string(),
                actor_id: bundle.profile.actor_id,
                reason_code: bundle.profile.reason_code,
                correlation_id: bundle.profile.correlation_id,
                updated_at_utc: bundle.profile.updated_at_utc,
                approval_reference: bundle.profile.approval_reference,
                inventory_rule_count: bundle.inventory_rules.len(),
            })
            .collect())
    }
}

fn build_profile_version(
    input: &UpsertRiskLimitProfileInput,
    profile_key: String,
    status: RiskLimitProfileStatus,
    reason_code: String,
    approval_reference: Option<String>,
) -> RiskLimitProfileVersion {
    RiskLimitProfileVersion {
        profile_key,
        version: input.version,
        portfolio: RiskScopeLimit {
            scope: RiskLimitScope::Portfolio,
            scope_id: normalize_risk_limit_identifier(&input.portfolio_scope_id),
            max_notional_usd: input.portfolio_max_notional_usd,
            max_inventory_units: input.portfolio_max_inventory_units,
            max_concentration_pct_nav: input.portfolio_max_concentration_pct_nav,
        },
        market: RiskScopeLimit {
            scope: RiskLimitScope::Market,
            scope_id: normalize_risk_limit_identifier(&input.market_scope_id),
            max_notional_usd: input.market_max_notional_usd,
            max_inventory_units: input.market_max_inventory_units,
            max_concentration_pct_nav: input.market_max_concentration_pct_nav,
        },
        strategy: RiskScopeLimit {
            scope: RiskLimitScope::Strategy,
            scope_id: normalize_risk_limit_identifier(&input.strategy_scope_id),
            max_notional_usd: input.strategy_max_notional_usd,
            max_inventory_units: input.strategy_max_inventory_units,
            max_concentration_pct_nav: input.strategy_max_concentration_pct_nav,
        },
        status,
        approval_reference,
        actor_id: input.actor_id.clone(),
        reason_code,
        correlation_id: input.correlation_id.clone(),
        updated_at_utc: input.updated_at_utc.clone(),
    }
}

fn build_inventory_rules(
    input: &UpsertRiskLimitProfileInput,
    normalized_profile_key: &str,
) -> Vec<InventoryLimitRule> {
    input
        .inventory_rules
        .iter()
        .enumerate()
        .map(|(index, rule)| InventoryLimitRule {
            rule_id: format!(
                "rule::{}::{}::{}",
                normalized_profile_key,
                input.version,
                index + 1
            ),
            profile_key: normalized_profile_key.to_string(),
            profile_version: input.version,
            scope: rule.scope,
            scope_id: normalize_risk_limit_identifier(&rule.scope_id),
            max_position_units: rule.max_position_units,
            max_order_size_units: rule.max_order_size_units,
            max_concentration_pct_nav: rule.max_concentration_pct_nav,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            updated_at_utc: input.updated_at_utc.clone(),
        })
        .collect()
}

fn validate_risk_limit_role(role: &str) -> Result<(), RiskLimitServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(RiskLimitServiceError::unauthorized_role()),
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), RiskLimitServiceError> {
    if value.trim().is_empty() {
        return Err(RiskLimitServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_risk_limit_telemetry(
    event_name: &'static str,
    outcome: &'static str,
    actor_id: &str,
    profile_key: &str,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
    approval_reference: Option<&str>,
) {
    let event = RiskLimitTelemetryEvent {
        event_name,
        action: match event_name {
            "risk_limit_pending_query_v1" => "risk_limit_pending_query",
            _ => "risk_limit_profile_update",
        },
        outcome,
        actor_id,
        profile_key,
        reason_code,
        correlation_id,
        timestamp_utc,
        approval_reference,
        security_signal: if outcome == "deny" {
            Some(RiskLimitSecuritySignal {
                name: "risk_limit_policy_denied_v1",
                severity: "high",
                alert_compatible: true,
                alert_target_seconds: 30,
            })
        } else {
            None
        },
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("risk limit telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct RiskLimitTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    profile_key: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_reference: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<RiskLimitSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct RiskLimitSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresRiskLimitRepository {
    pool: PgPool,
}

impl PostgresRiskLimitRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, RiskLimitServiceError>
    where
        F: Future<Output = Result<T, RiskLimitPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    RiskLimitServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl RiskLimitRepositoryPort for PostgresRiskLimitRepository {
    fn upsert_profile_bundle(
        &self,
        bundle: RiskLimitProfileBundle,
    ) -> Result<(), RiskLimitServiceError> {
        self.run_with_runtime(pg_upsert_profile_bundle(
            &self.pool,
            &bundle.profile,
            &bundle.inventory_rules,
        ))
    }

    fn load_active_profile_bundle(
        &self,
        profile_key: &str,
    ) -> Result<Option<RiskLimitProfileBundle>, RiskLimitServiceError> {
        self.run_with_runtime(pg_load_active_profile_bundle(&self.pool, profile_key))
    }

    fn load_pending_profile_bundles(
        &self,
        profile_key: Option<&str>,
    ) -> Result<Vec<RiskLimitProfileBundle>, RiskLimitServiceError> {
        self.run_with_runtime(pg_load_pending_profile_bundles(&self.pool, profile_key))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryRiskLimitRepository {
    bundles: Mutex<BTreeMap<(String, i64), RiskLimitProfileBundle>>,
}

impl RiskLimitRepositoryPort for InMemoryRiskLimitRepository {
    fn upsert_profile_bundle(
        &self,
        bundle: RiskLimitProfileBundle,
    ) -> Result<(), RiskLimitServiceError> {
        let mut bundles = self
            .bundles
            .lock()
            .expect("in-memory risk limit bundle lock should not be poisoned");
        if bundle.profile.status == RiskLimitProfileStatus::Active {
            let key_prefix = bundle.profile.profile_key.clone();
            for existing in bundles.values_mut() {
                if existing.profile.profile_key == key_prefix
                    && existing.profile.status == RiskLimitProfileStatus::Active
                {
                    existing.profile.status = RiskLimitProfileStatus::Denied;
                    existing.profile.reason_code =
                        RiskLimitReasonCode::ProfileDenied.code().to_string();
                    existing.profile.approval_reference = None;
                }
            }
        }
        bundles.insert(
            (bundle.profile.profile_key.clone(), bundle.profile.version),
            bundle,
        );
        Ok(())
    }

    fn load_active_profile_bundle(
        &self,
        profile_key: &str,
    ) -> Result<Option<RiskLimitProfileBundle>, RiskLimitServiceError> {
        let bundles = self
            .bundles
            .lock()
            .expect("in-memory risk limit bundle lock should not be poisoned");
        let normalized = normalize_risk_limit_identifier(profile_key);
        let selected = bundles
            .iter()
            .filter(|((key, _), bundle)| {
                normalize_risk_limit_identifier(key) == normalized
                    && bundle.profile.status == RiskLimitProfileStatus::Active
            })
            .max_by_key(|((_, version), _)| *version)
            .map(|(_, bundle)| bundle.clone());
        Ok(selected)
    }

    fn load_pending_profile_bundles(
        &self,
        profile_key: Option<&str>,
    ) -> Result<Vec<RiskLimitProfileBundle>, RiskLimitServiceError> {
        let bundles = self
            .bundles
            .lock()
            .expect("in-memory risk limit bundle lock should not be poisoned");
        let filter_key = profile_key.map(normalize_risk_limit_identifier);
        let mut pending: Vec<_> = bundles
            .values()
            .filter(|bundle| bundle.profile.status == RiskLimitProfileStatus::Pending)
            .filter(|bundle| {
                filter_key.as_ref().is_none_or(|expected| {
                    normalize_risk_limit_identifier(&bundle.profile.profile_key) == *expected
                })
            })
            .cloned()
            .collect();
        pending.sort_by(|left, right| {
            left.profile
                .profile_key
                .cmp(&right.profile.profile_key)
                .then(right.profile.version.cmp(&left.profile.version))
        });
        Ok(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> UpsertRiskLimitProfileInput {
        UpsertRiskLimitProfileInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            profile_key: "default".to_string(),
            version: 1,
            portfolio_scope_id: "portfolio::default".to_string(),
            market_scope_id: "market::sports".to_string(),
            strategy_scope_id: "strategy::maker".to_string(),
            portfolio_max_notional_usd: 1000.0,
            market_max_notional_usd: 600.0,
            strategy_max_notional_usd: 300.0,
            portfolio_max_inventory_units: 800.0,
            market_max_inventory_units: 400.0,
            strategy_max_inventory_units: 200.0,
            portfolio_max_concentration_pct_nav: 45.0,
            market_max_concentration_pct_nav: 30.0,
            strategy_max_concentration_pct_nav: 20.0,
            inventory_rules: vec![
                RiskLimitRuleInput {
                    scope: RiskLimitScope::Market,
                    scope_id: "market::sports".to_string(),
                    max_position_units: 300.0,
                    max_order_size_units: 40.0,
                    max_concentration_pct_nav: 25.0,
                },
                RiskLimitRuleInput {
                    scope: RiskLimitScope::Strategy,
                    scope_id: "strategy::maker".to_string(),
                    max_position_units: 150.0,
                    max_order_size_units: 20.0,
                    max_concentration_pct_nav: 15.0,
                },
            ],
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
            approval_reference: None,
        }
    }

    #[test]
    fn upsert_rejects_unauthorized_role() {
        let service = RiskLimitService::default();
        let mut input = sample_input();
        input.actor_role = "read_only_analytics".to_string();

        let error = service
            .upsert_risk_limit_profile(input)
            .expect_err("unauthorized role should fail closed");
        assert_eq!(error.code, "risk_limit_unauthorized_role");
    }

    #[test]
    fn critical_increase_without_approval_reference_returns_pending() {
        let service = RiskLimitService::default();
        service
            .upsert_risk_limit_profile(sample_input())
            .expect("initial profile should be applied");

        let mut critical_increase = sample_input();
        critical_increase.version = 2;
        critical_increase.market_max_notional_usd = 650.0;
        critical_increase.updated_at_utc = "2026-04-06T00:05:00Z".to_string();

        let evidence = service
            .upsert_risk_limit_profile(critical_increase)
            .expect("critical increase should return pending evidence");
        assert_eq!(evidence.status, "pending");
        assert_eq!(
            evidence.reason_code,
            RiskLimitReasonCode::ApprovalRequired.code()
        );
        assert!(evidence.approval_reference.is_none());
    }

    #[test]
    fn approved_critical_increase_becomes_active() {
        let service = RiskLimitService::default();
        service
            .upsert_risk_limit_profile(sample_input())
            .expect("initial profile should be applied");

        let mut approved_increase = sample_input();
        approved_increase.version = 2;
        approved_increase.market_max_notional_usd = 650.0;
        approved_increase.updated_at_utc = "2026-04-06T00:06:00Z".to_string();
        approved_increase.approval_reference = Some("apr_risk_limit_increase_req_2".to_string());

        let evidence = service
            .upsert_risk_limit_profile(approved_increase)
            .expect("approved critical increase should be active");
        assert_eq!(evidence.status, "active");
        assert_eq!(
            evidence.reason_code,
            RiskLimitReasonCode::ProfileApplied.code()
        );
        assert_eq!(
            evidence.approval_reference.as_deref(),
            Some("apr_risk_limit_increase_req_2")
        );
    }

    #[test]
    fn pending_query_returns_pending_profiles_with_actor_and_reason_fields() {
        let service = RiskLimitService::default();
        service
            .upsert_risk_limit_profile(sample_input())
            .expect("initial profile should be applied");
        let mut pending = sample_input();
        pending.version = 2;
        pending.market_max_notional_usd = 700.0;
        pending.updated_at_utc = "2026-04-06T00:08:00Z".to_string();
        service
            .upsert_risk_limit_profile(pending)
            .expect("critical increase should be pending");

        let pending_profiles = service
            .list_pending_risk_limit_profiles(PendingRiskLimitProfilesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-risk-limit-query-001".to_string(),
                queried_at_utc: "2026-04-06T00:09:00Z".to_string(),
                profile_key: Some("default".to_string()),
            })
            .expect("pending query should succeed");
        assert_eq!(pending_profiles.len(), 1);
        assert_eq!(pending_profiles[0].status, "pending");
        assert_eq!(
            pending_profiles[0].reason_code,
            RiskLimitReasonCode::ApprovalRequired.code()
        );
        assert_eq!(pending_profiles[0].actor_id, "ops-1");
    }

    #[test]
    fn upsert_rejects_invalid_cross_scope_payload_with_field_errors() {
        let service = RiskLimitService::default();
        let mut input = sample_input();
        input.market_max_notional_usd = input.portfolio_max_notional_usd + 1.0;

        let error = service
            .upsert_risk_limit_profile(input)
            .expect_err("market > portfolio must fail");
        assert_eq!(error.code, RiskLimitReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market.max_notional_usd")
        );
    }
}
