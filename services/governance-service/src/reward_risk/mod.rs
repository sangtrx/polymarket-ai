use domain::risk::{
    REWARD_RISK_DEFAULT_THRESHOLD, RewardRiskPolicy, RewardRiskReasonCode,
    RewardRiskValidationIssue, normalize_reward_risk_identifier, validate_reward_risk_policy,
};
use persistence::postgres::reward_risk::{
    RewardRiskPersistenceError, load_reward_risk_policy as pg_load_reward_risk_policy,
    upsert_reward_risk_policy as pg_upsert_reward_risk_policy,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RewardRiskServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<RewardRiskValidationIssue>,
}

impl RewardRiskServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RewardRiskValidationIssue>,
    ) -> Self {
        Self {
            code: RewardRiskReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: "reward_risk_unauthorized_role",
            message: "actor role is not authorized for reward-risk workflows".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RewardRiskReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RewardRiskServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RewardRiskServiceError {}

fn map_persistence_error(error: RewardRiskPersistenceError) -> RewardRiskServiceError {
    match error.code {
        "reward_risk_query_failed" | "reward_risk_row_decode_failed" => {
            RewardRiskServiceError::persistence_unavailable(error.message)
        }
        _ => RewardRiskServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct UpsertRewardRiskPolicyInput {
    pub actor_id: String,
    pub actor_role: String,
    pub policy_key: String,
    pub strategy_key: String,
    pub min_reward_per_risk: f64,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadRewardRiskPolicyInput {
    pub actor_id: String,
    pub actor_role: String,
    pub policy_key: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RewardRiskPolicyEvidence {
    pub policy_key: String,
    pub strategy_key: String,
    pub min_reward_per_risk: f64,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
    pub default_threshold_applied: bool,
}

pub trait RewardRiskRepositoryPort: Send + Sync {
    fn upsert_policy(&self, policy: RewardRiskPolicy) -> Result<(), RewardRiskServiceError>;
    fn load_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<RewardRiskPolicy>, RewardRiskServiceError>;
}

pub trait RewardRiskOrchestrator: Send + Sync {
    fn upsert_reward_risk_policy(
        &self,
        input: UpsertRewardRiskPolicyInput,
    ) -> Result<RewardRiskPolicyEvidence, RewardRiskServiceError>;
    fn read_reward_risk_policy(
        &self,
        input: ReadRewardRiskPolicyInput,
    ) -> Result<RewardRiskPolicyEvidence, RewardRiskServiceError>;
}

#[derive(Clone)]
pub struct RewardRiskService {
    repository: Arc<dyn RewardRiskRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl RewardRiskService {
    pub fn new(repository: Arc<dyn RewardRiskRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryRewardRiskRepository::default()))
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresRewardRiskRepository::new(pool)))
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, RewardRiskServiceError> {
        self.operation_lock.lock().map_err(|_| {
            RewardRiskServiceError::persistence_unavailable(
                "reward-risk operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for RewardRiskService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl RewardRiskOrchestrator for RewardRiskService {
    fn upsert_reward_risk_policy(
        &self,
        input: UpsertRewardRiskPolicyInput,
    ) -> Result<RewardRiskPolicyEvidence, RewardRiskServiceError> {
        let normalized_policy_key = normalize_reward_risk_identifier(&input.policy_key);
        let normalized_strategy_key = normalize_reward_risk_identifier(&input.strategy_key);
        if let Err(error) = validate_reward_risk_role(&input.actor_role) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("actor_id", &input.actor_id) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("policy_key", &input.policy_key) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("strategy_key", &input.strategy_key) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("correlation_id", &input.correlation_id) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("updated_at_utc", &input.updated_at_utc) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let policy = RewardRiskPolicy {
            policy_key: normalized_policy_key.clone(),
            strategy_key: normalized_strategy_key,
            min_reward_per_risk: input.min_reward_per_risk,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            updated_at_utc: input.updated_at_utc.clone(),
        };
        if let Err(error) = validate_reward_risk_policy(&policy).map_err(|error| {
            RewardRiskServiceError::invalid_payload(error.message, error.field_errors)
        }) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_reward_risk_telemetry(
                "reward_risk_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
        })?;
        self.repository
            .upsert_policy(policy.clone())
            .inspect_err(|error| {
                emit_reward_risk_telemetry(
                    "reward_risk_policy_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        emit_reward_risk_telemetry(
            "reward_risk_policy_upsert_v1",
            "allow",
            &input.actor_id,
            &normalized_policy_key,
            RewardRiskReasonCode::PolicyUpdated.code(),
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(RewardRiskPolicyEvidence {
            policy_key: policy.policy_key,
            strategy_key: policy.strategy_key,
            min_reward_per_risk: policy.min_reward_per_risk,
            actor_id: policy.actor_id,
            reason_code: RewardRiskReasonCode::PolicyUpdated.code().to_string(),
            correlation_id: policy.correlation_id,
            updated_at_utc: policy.updated_at_utc,
            default_threshold_applied: false,
        })
    }

    fn read_reward_risk_policy(
        &self,
        input: ReadRewardRiskPolicyInput,
    ) -> Result<RewardRiskPolicyEvidence, RewardRiskServiceError> {
        if let Err(error) = validate_reward_risk_role(&input.actor_role) {
            emit_reward_risk_telemetry(
                "reward_risk_policy_read_v1",
                "deny",
                &input.actor_id,
                &input.policy_key,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("policy_key", &input.policy_key)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;

        let normalized_policy_key = normalize_reward_risk_identifier(&input.policy_key);
        let loaded = self.repository.load_policy(&normalized_policy_key)?;
        let evidence = if let Some(policy) = loaded {
            RewardRiskPolicyEvidence {
                policy_key: policy.policy_key,
                strategy_key: policy.strategy_key,
                min_reward_per_risk: policy.min_reward_per_risk,
                actor_id: input.actor_id.clone(),
                reason_code: RewardRiskReasonCode::PolicyRead.code().to_string(),
                correlation_id: input.correlation_id.clone(),
                updated_at_utc: input.queried_at_utc.clone(),
                default_threshold_applied: false,
            }
        } else {
            RewardRiskPolicyEvidence {
                policy_key: normalized_policy_key.clone(),
                strategy_key: normalized_policy_key.clone(),
                min_reward_per_risk: REWARD_RISK_DEFAULT_THRESHOLD,
                actor_id: input.actor_id.clone(),
                reason_code: RewardRiskReasonCode::DefaultThresholdApplied
                    .code()
                    .to_string(),
                correlation_id: input.correlation_id.clone(),
                updated_at_utc: input.queried_at_utc.clone(),
                default_threshold_applied: true,
            }
        };

        emit_reward_risk_telemetry(
            "reward_risk_policy_read_v1",
            "allow",
            &input.actor_id,
            &evidence.policy_key,
            &evidence.reason_code,
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(evidence)
    }
}

fn validate_reward_risk_role(role: &str) -> Result<(), RewardRiskServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(RewardRiskServiceError::unauthorized_role()),
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), RewardRiskServiceError> {
    if value.trim().is_empty() {
        return Err(RewardRiskServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_reward_risk_telemetry(
    event_name: &'static str,
    outcome: &'static str,
    actor_id: &str,
    policy_key: &str,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = RewardRiskTelemetryEvent {
        event_name,
        action: match event_name {
            "reward_risk_policy_read_v1" => "reward_risk_policy_read",
            _ => "reward_risk_policy_upsert",
        },
        outcome,
        actor_id,
        policy_key,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(RewardRiskSecuritySignal {
                name: "reward_risk_policy_denied_v1",
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
        serde_json::to_string(&event).expect("reward-risk telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct RewardRiskTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    policy_key: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<RewardRiskSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct RewardRiskSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresRewardRiskRepository {
    pool: PgPool,
}

impl PostgresRewardRiskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, RewardRiskServiceError>
    where
        F: Future<Output = Result<T, RewardRiskPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    RewardRiskServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl RewardRiskRepositoryPort for PostgresRewardRiskRepository {
    fn upsert_policy(&self, policy: RewardRiskPolicy) -> Result<(), RewardRiskServiceError> {
        self.run_with_runtime(pg_upsert_reward_risk_policy(&self.pool, &policy))
    }

    fn load_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<RewardRiskPolicy>, RewardRiskServiceError> {
        self.run_with_runtime(pg_load_reward_risk_policy(&self.pool, policy_key))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryRewardRiskRepository {
    policies: Mutex<BTreeMap<String, RewardRiskPolicy>>,
}

impl RewardRiskRepositoryPort for InMemoryRewardRiskRepository {
    fn upsert_policy(&self, policy: RewardRiskPolicy) -> Result<(), RewardRiskServiceError> {
        self.policies
            .lock()
            .expect("in-memory reward-risk policy lock should not be poisoned")
            .insert(policy.policy_key.clone(), policy);
        Ok(())
    }

    fn load_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<RewardRiskPolicy>, RewardRiskServiceError> {
        let normalized = normalize_reward_risk_identifier(policy_key);
        Ok(self
            .policies
            .lock()
            .expect("in-memory reward-risk policy lock should not be poisoned")
            .get(&normalized)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_upsert_input() -> UpsertRewardRiskPolicyInput {
        UpsertRewardRiskPolicyInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            policy_key: "strategy::maker-alpha".to_string(),
            strategy_key: "strategy::maker-alpha".to_string(),
            min_reward_per_risk: 1.4,
            correlation_id: "corr-reward-risk-upsert-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn upsert_rejects_unauthorized_role() {
        let service = RewardRiskService::default();
        let mut input = sample_upsert_input();
        input.actor_role = "read_only_analytics".to_string();

        let error = service
            .upsert_reward_risk_policy(input)
            .expect_err("unauthorized role should fail closed");
        assert_eq!(error.code, "reward_risk_unauthorized_role");
    }

    #[test]
    fn read_returns_default_threshold_when_policy_is_missing() {
        let service = RewardRiskService::default();
        let evidence = service
            .read_reward_risk_policy(ReadRewardRiskPolicyInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                policy_key: "strategy::maker-missing".to_string(),
                correlation_id: "corr-reward-risk-read-001".to_string(),
                queried_at_utc: "2026-04-07T00:10:00Z".to_string(),
            })
            .expect("read should return deterministic default");

        assert_eq!(
            evidence.reason_code,
            RewardRiskReasonCode::DefaultThresholdApplied.code()
        );
        assert_eq!(evidence.min_reward_per_risk, REWARD_RISK_DEFAULT_THRESHOLD);
        assert!(evidence.default_threshold_applied);
    }

    #[test]
    fn upsert_normalizes_policy_identifiers_and_returns_evidence() {
        let service = RewardRiskService::default();
        let mut input = sample_upsert_input();
        input.policy_key = " Strategy::Maker-Alpha ".to_string();
        input.strategy_key = " Strategy::Maker-Alpha ".to_string();

        let evidence = service
            .upsert_reward_risk_policy(input)
            .expect("upsert should normalize identifiers");
        assert_eq!(evidence.policy_key, "strategy::maker-alpha");
        assert_eq!(evidence.strategy_key, "strategy::maker-alpha");
        assert_eq!(
            evidence.reason_code,
            RewardRiskReasonCode::PolicyUpdated.code()
        );
        assert!(!evidence.default_threshold_applied);
    }

    #[test]
    fn upsert_rejects_invalid_threshold_with_field_error() {
        let service = RewardRiskService::default();
        let mut input = sample_upsert_input();
        input.min_reward_per_risk = -0.2;

        let error = service
            .upsert_reward_risk_policy(input)
            .expect_err("invalid threshold should fail");
        assert_eq!(error.code, RewardRiskReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "min_reward_per_risk")
        );
    }

    #[test]
    fn read_returns_policy_override_after_upsert() {
        let service = RewardRiskService::default();
        service
            .upsert_reward_risk_policy(sample_upsert_input())
            .expect("upsert should succeed");

        let evidence = service
            .read_reward_risk_policy(ReadRewardRiskPolicyInput {
                actor_id: "ops-2".to_string(),
                actor_role: "administrative_actions".to_string(),
                policy_key: "strategy::maker-alpha".to_string(),
                correlation_id: "corr-reward-risk-read-002".to_string(),
                queried_at_utc: "2026-04-07T00:11:00Z".to_string(),
            })
            .expect("read should load persisted override");
        assert_eq!(
            evidence.reason_code,
            RewardRiskReasonCode::PolicyRead.code()
        );
        assert_eq!(evidence.min_reward_per_risk, 1.4);
        assert!(!evidence.default_threshold_applied);
    }
}
