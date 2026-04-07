use domain::risk::{
    MarketBucketProfile, MarketBucketReasonCode, MarketBucketValidationIssue,
    MarketClusterOverride, MarketPolicyProfile, MarketPolicyReasonCode, MarketPolicyValidationIssue,
    canonical_market_bucket_profile_id, canonicalize_market_bucket_profile,
    validate_market_cluster_override, validate_market_policy_profile,
};
use persistence::postgres::market_bucket_profiles::{
    MarketBucketPersistenceError, load_active_market_bucket_profile as pg_load_active_bucket_profile,
    upsert_market_bucket_profile as pg_upsert_market_bucket_profile,
};
use persistence::postgres::market_policy::{
    MarketPolicyPersistenceError, load_active_market_policy_profile as pg_load_active_profile,
    load_market_cluster_override as pg_load_cluster_override,
    upsert_market_cluster_override as pg_upsert_cluster_override,
    upsert_market_policy_profile as pg_upsert_profile,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MarketPolicyServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<MarketPolicyValidationIssue>,
}

impl MarketPolicyServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<MarketPolicyValidationIssue>,
    ) -> Self {
        Self {
            code: MarketPolicyReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: "market_policy_unauthorized_role",
            message: "actor role is not authorized for market policy workflows".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: MarketPolicyReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for MarketPolicyServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for MarketPolicyServiceError {}

fn map_market_policy_persistence_error(error: MarketPolicyPersistenceError) -> MarketPolicyServiceError {
    match error.code {
        "market_policy_query_failed" | "market_policy_row_decode_failed" => {
            MarketPolicyServiceError::persistence_unavailable(error.message)
        }
        _ => MarketPolicyServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

fn map_market_bucket_persistence_error(error: MarketBucketPersistenceError) -> MarketPolicyServiceError {
    match error.code {
        "market_bucket_query_failed" | "market_bucket_row_decode_failed" => {
            MarketPolicyServiceError::persistence_unavailable(error.message)
        }
        _ => MarketPolicyServiceError {
            code: error.code,
            message: error.message,
            field_errors: map_bucket_validation_issues(error.field_errors),
        },
    }
}

fn map_bucket_validation_issues(
    field_errors: Vec<MarketBucketValidationIssue>,
) -> Vec<MarketPolicyValidationIssue> {
    field_errors
        .into_iter()
        .map(|issue| MarketPolicyValidationIssue {
            field: issue.field,
            code: issue.code,
            message: issue.message,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct UpsertMarketPolicyProfileInput {
    pub actor_id: String,
    pub actor_role: String,
    pub cluster_id: String,
    pub min_liquidity_usd: f64,
    pub max_spread_bps: f64,
    pub min_reward_score: f64,
    pub max_exposure_pct_nav: f64,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ToggleMarketClusterInput {
    pub actor_id: String,
    pub actor_role: String,
    pub cluster_id: String,
    pub is_enabled: bool,
    pub reason_code: Option<String>,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct UpsertMarketBucketProfileInput {
    pub actor_id: String,
    pub actor_role: String,
    pub market_id: String,
    pub cluster_id: String,
    pub bucket_type: String,
    pub risk_policy_key: String,
    pub allocation_policy_key: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadMarketBucketProfileInput {
    pub actor_id: String,
    pub actor_role: String,
    pub market_id: String,
    pub cluster_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MarketPolicyProfileEvidence {
    pub profile_id: String,
    pub cluster_id: String,
    pub min_liquidity_usd: f64,
    pub max_spread_bps: f64,
    pub min_reward_score: f64,
    pub max_exposure_pct_nav: f64,
    pub is_active: bool,
    pub actor_id: String,
    pub correlation_id: String,
    pub reason_code: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MarketClusterToggleEvidence {
    pub cluster_id: String,
    pub is_enabled: bool,
    pub actor_id: String,
    pub correlation_id: String,
    pub reason_code: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MarketBucketProfileEvidence {
    pub profile_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub bucket_type: String,
    pub risk_policy_key: String,
    pub allocation_policy_key: String,
    pub is_active: bool,
    pub actor_id: String,
    pub correlation_id: String,
    pub reason_code: String,
    pub updated_at_utc: String,
}

pub trait MarketPolicyRepositoryPort: Send + Sync {
    fn upsert_profile(&self, profile: MarketPolicyProfile) -> Result<(), MarketPolicyServiceError>;
    fn upsert_bucket_profile(
        &self,
        profile: MarketBucketProfile,
    ) -> Result<(), MarketPolicyServiceError>;
    fn upsert_cluster_override(
        &self,
        cluster_override: MarketClusterOverride,
    ) -> Result<(), MarketPolicyServiceError>;
    fn load_active_profile(
        &self,
        cluster_id: &str,
    ) -> Result<Option<MarketPolicyProfile>, MarketPolicyServiceError>;
    fn load_active_bucket_profile(
        &self,
        market_id: &str,
        cluster_id: &str,
    ) -> Result<Option<MarketBucketProfile>, MarketPolicyServiceError>;
    fn load_cluster_override(
        &self,
        cluster_id: &str,
    ) -> Result<Option<MarketClusterOverride>, MarketPolicyServiceError>;
}

pub trait MarketPolicyOrchestrator: Send + Sync {
    fn upsert_market_policy_profile(
        &self,
        input: UpsertMarketPolicyProfileInput,
    ) -> Result<MarketPolicyProfileEvidence, MarketPolicyServiceError>;
    fn toggle_market_cluster(
        &self,
        input: ToggleMarketClusterInput,
    ) -> Result<MarketClusterToggleEvidence, MarketPolicyServiceError>;
    fn upsert_market_bucket_profile(
        &self,
        input: UpsertMarketBucketProfileInput,
    ) -> Result<MarketBucketProfileEvidence, MarketPolicyServiceError>;
    fn read_market_bucket_profile(
        &self,
        input: ReadMarketBucketProfileInput,
    ) -> Result<MarketBucketProfileEvidence, MarketPolicyServiceError>;
}

#[derive(Clone)]
pub struct MarketPolicyService {
    repository: Arc<dyn MarketPolicyRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl MarketPolicyService {
    pub fn new(repository: Arc<dyn MarketPolicyRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryMarketPolicyRepository::default()))
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresMarketPolicyRepository::new(pool)))
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, MarketPolicyServiceError> {
        self.operation_lock.lock().map_err(|_| {
            MarketPolicyServiceError::persistence_unavailable(
                "market policy operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for MarketPolicyService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl MarketPolicyOrchestrator for MarketPolicyService {
    fn upsert_market_policy_profile(
        &self,
        input: UpsertMarketPolicyProfileInput,
    ) -> Result<MarketPolicyProfileEvidence, MarketPolicyServiceError> {
        let normalized_cluster_id = normalize_cluster_id(&input.cluster_id);
        if let Err(error) = validate_market_policy_role(&input.actor_role) {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("actor_id", &input.actor_id) {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("cluster_id", &input.cluster_id) {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("correlation_id", &input.correlation_id) {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("updated_at_utc", &input.updated_at_utc) {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let profile = MarketPolicyProfile {
            profile_id: format!("policy::{normalized_cluster_id}"),
            cluster_id: normalized_cluster_id.clone(),
            min_liquidity_usd: input.min_liquidity_usd,
            max_spread_bps: input.max_spread_bps,
            min_reward_score: input.min_reward_score,
            max_exposure_pct_nav: input.max_exposure_pct_nav,
            is_active: true,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            updated_at_utc: input.updated_at_utc.clone(),
        };
        if let Err(error) = validate_market_policy_profile(&profile).map_err(|error| {
            MarketPolicyServiceError::invalid_payload(error.message, error.field_errors.clone())
        }) {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_market_policy_telemetry(
                "market_policy_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
        })?;
        self.repository
            .upsert_profile(profile.clone())
            .inspect_err(|error| {
                emit_market_policy_telemetry(
                    "market_policy_profile_update_v1",
                    "deny",
                    &input.actor_id,
                    &input.cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        emit_market_policy_telemetry(
            "market_policy_profile_update_v1",
            "allow",
            &input.actor_id,
            &input.cluster_id,
            MarketPolicyReasonCode::ProfileUpdated.code(),
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(MarketPolicyProfileEvidence {
            profile_id: profile.profile_id,
            cluster_id: profile.cluster_id,
            min_liquidity_usd: profile.min_liquidity_usd,
            max_spread_bps: profile.max_spread_bps,
            min_reward_score: profile.min_reward_score,
            max_exposure_pct_nav: profile.max_exposure_pct_nav,
            is_active: profile.is_active,
            actor_id: profile.actor_id,
            correlation_id: profile.correlation_id,
            reason_code: MarketPolicyReasonCode::ProfileUpdated.code().to_string(),
            updated_at_utc: profile.updated_at_utc,
        })
    }

    fn toggle_market_cluster(
        &self,
        input: ToggleMarketClusterInput,
    ) -> Result<MarketClusterToggleEvidence, MarketPolicyServiceError> {
        let normalized_cluster_id = normalize_cluster_id(&input.cluster_id);
        if let Err(error) = validate_market_policy_role(&input.actor_role) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("actor_id", &input.actor_id) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("cluster_id", &input.cluster_id) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("correlation_id", &input.correlation_id) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if let Err(error) = validate_non_empty("updated_at_utc", &input.updated_at_utc) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let fallback_reason = if input.is_enabled {
            MarketPolicyReasonCode::ClusterEnabled.code()
        } else {
            MarketPolicyReasonCode::ClusterDisabledByOperator.code()
        };
        let reason_code = input
            .reason_code
            .as_deref()
            .unwrap_or(fallback_reason)
            .trim();
        if let Err(error) = validate_non_empty("reason_code", reason_code) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        let parsed_reason = match MarketPolicyReasonCode::parse(reason_code).map_err(|error| {
            MarketPolicyServiceError::invalid_payload(error.message, error.field_errors)
        }) {
            Ok(parsed_reason) => parsed_reason,
            Err(error) => {
                emit_market_policy_telemetry(
                    "market_policy_cluster_toggle_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
                return Err(error);
            }
        };
        if input.is_enabled && parsed_reason != MarketPolicyReasonCode::ClusterEnabled {
            let error = MarketPolicyServiceError::invalid_payload(
                "enabled cluster toggles must use `market_policy_cluster_enabled` reason code",
                vec![MarketPolicyValidationIssue {
                    field: "reason_code",
                    code: MarketPolicyReasonCode::InvalidPayload.code(),
                    message:
                        "enabled cluster toggles must use `market_policy_cluster_enabled` reason code"
                            .to_string(),
                }],
            );
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        if !input.is_enabled && parsed_reason != MarketPolicyReasonCode::ClusterDisabledByOperator {
            let error = MarketPolicyServiceError::invalid_payload(
                "disabled cluster toggles must use `market_policy_cluster_disabled_by_operator` reason code",
                vec![MarketPolicyValidationIssue {
                    field: "reason_code",
                    code: MarketPolicyReasonCode::InvalidPayload.code(),
                    message: "disabled cluster toggles must use `market_policy_cluster_disabled_by_operator` reason code".to_string(),
                }],
            );
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let cluster_override = MarketClusterOverride {
            cluster_id: normalized_cluster_id.clone(),
            is_enabled: input.is_enabled,
            reason_code: reason_code.to_string(),
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            updated_at_utc: input.updated_at_utc.clone(),
        };
        if let Err(error) = validate_market_cluster_override(&cluster_override).map_err(|error| {
            MarketPolicyServiceError::invalid_payload(error.message, error.field_errors.clone())
        }) {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_market_policy_telemetry(
                "market_policy_cluster_toggle_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
        })?;
        self.repository
            .upsert_cluster_override(cluster_override.clone())
            .inspect_err(|error| {
                emit_market_policy_telemetry(
                    "market_policy_cluster_toggle_v1",
                    "deny",
                    &input.actor_id,
                    &input.cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        emit_market_policy_telemetry(
            "market_policy_cluster_toggle_v1",
            "allow",
            &input.actor_id,
            &input.cluster_id,
            &cluster_override.reason_code,
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(MarketClusterToggleEvidence {
            cluster_id: cluster_override.cluster_id,
            is_enabled: cluster_override.is_enabled,
            actor_id: cluster_override.actor_id,
            correlation_id: cluster_override.correlation_id,
            reason_code: cluster_override.reason_code,
            updated_at_utc: cluster_override.updated_at_utc,
        })
    }

    fn upsert_market_bucket_profile(
        &self,
        input: UpsertMarketBucketProfileInput,
    ) -> Result<MarketBucketProfileEvidence, MarketPolicyServiceError> {
        let normalized_market_id = normalize_market_bucket_identifier(&input.market_id);
        let normalized_cluster_id = normalize_market_bucket_identifier(&input.cluster_id);
        if let Err(error) = validate_market_policy_role(&input.actor_role) {
            emit_market_policy_telemetry(
                "market_bucket_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        for (field, value) in [
            ("actor_id", input.actor_id.as_str()),
            ("market_id", input.market_id.as_str()),
            ("cluster_id", input.cluster_id.as_str()),
            ("bucket_type", input.bucket_type.as_str()),
            ("risk_policy_key", input.risk_policy_key.as_str()),
            ("allocation_policy_key", input.allocation_policy_key.as_str()),
            ("correlation_id", input.correlation_id.as_str()),
            ("updated_at_utc", input.updated_at_utc.as_str()),
        ] {
            if let Err(error) = validate_non_empty(field, value) {
                emit_market_policy_telemetry(
                    "market_bucket_profile_update_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
                return Err(error);
            }
        }

        let profile = MarketBucketProfile {
            profile_id: canonical_market_bucket_profile_id(&input.market_id, &input.cluster_id),
            market_id: normalized_market_id,
            cluster_id: normalized_cluster_id.clone(),
            bucket_type: input.bucket_type.clone(),
            risk_policy_key: input.risk_policy_key.clone(),
            allocation_policy_key: input.allocation_policy_key.clone(),
            is_active: true,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            updated_at_utc: input.updated_at_utc.clone(),
        };
        let canonical_profile = match canonicalize_market_bucket_profile(&profile).map_err(|error| {
            MarketPolicyServiceError {
                code: error.code,
                message: error.message,
                field_errors: map_bucket_validation_issues(error.field_errors),
            }
        }) {
            Ok(profile) => profile,
            Err(error) => {
                emit_market_policy_telemetry(
                    "market_bucket_profile_update_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
                return Err(error);
            }
        };

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_market_policy_telemetry(
                "market_bucket_profile_update_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
        })?;
        self.repository
            .upsert_bucket_profile(canonical_profile.clone())
            .inspect_err(|error| {
                emit_market_policy_telemetry(
                    "market_bucket_profile_update_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        emit_market_policy_telemetry(
            "market_bucket_profile_update_v1",
            "allow",
            &input.actor_id,
            &canonical_profile.cluster_id,
            MarketBucketReasonCode::ProfileUpdated.code(),
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(MarketBucketProfileEvidence {
            profile_id: canonical_profile.profile_id,
            market_id: canonical_profile.market_id,
            cluster_id: canonical_profile.cluster_id,
            bucket_type: canonical_profile.bucket_type,
            risk_policy_key: canonical_profile.risk_policy_key,
            allocation_policy_key: canonical_profile.allocation_policy_key,
            is_active: canonical_profile.is_active,
            actor_id: canonical_profile.actor_id,
            correlation_id: canonical_profile.correlation_id,
            reason_code: MarketBucketReasonCode::ProfileUpdated.code().to_string(),
            updated_at_utc: canonical_profile.updated_at_utc,
        })
    }

    fn read_market_bucket_profile(
        &self,
        input: ReadMarketBucketProfileInput,
    ) -> Result<MarketBucketProfileEvidence, MarketPolicyServiceError> {
        let normalized_cluster_id = normalize_market_bucket_identifier(&input.cluster_id);
        if let Err(error) = validate_market_policy_role(&input.actor_role) {
            emit_market_policy_telemetry(
                "market_bucket_profile_read_v1",
                "deny",
                &input.actor_id,
                &normalized_cluster_id,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        for (field, value) in [
            ("actor_id", input.actor_id.as_str()),
            ("market_id", input.market_id.as_str()),
            ("cluster_id", input.cluster_id.as_str()),
            ("correlation_id", input.correlation_id.as_str()),
            ("queried_at_utc", input.queried_at_utc.as_str()),
        ] {
            if let Err(error) = validate_non_empty(field, value) {
                emit_market_policy_telemetry(
                    "market_bucket_profile_read_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
                return Err(error);
            }
        }

        let maybe_profile = self
            .repository
            .load_active_bucket_profile(&input.market_id, &input.cluster_id)
            .inspect_err(|error| {
                emit_market_policy_telemetry(
                    "market_bucket_profile_read_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;
        let profile = match maybe_profile {
            Some(profile) => profile,
            None => {
                emit_market_policy_telemetry(
                    "market_bucket_profile_read_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_cluster_id,
                    MarketBucketReasonCode::MappingUnavailable.code(),
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
                return Err(MarketPolicyServiceError {
                    code: MarketBucketReasonCode::MappingUnavailable.code(),
                    message: "no active market bucket profile found for market/cluster".to_string(),
                    field_errors: vec![MarketPolicyValidationIssue {
                        field: "market_id",
                        code: MarketBucketReasonCode::MappingUnavailable.code(),
                        message:
                            "no active market bucket mapping exists for this market/cluster pair"
                                .to_string(),
                    }],
                });
            }
        };

        let canonical_profile = canonicalize_market_bucket_profile(&profile).map_err(|error| {
            MarketPolicyServiceError {
                code: error.code,
                message: error.message,
                field_errors: map_bucket_validation_issues(error.field_errors),
            }
        })?;

        emit_market_policy_telemetry(
            "market_bucket_profile_read_v1",
            "allow",
            &input.actor_id,
            &canonical_profile.cluster_id,
            MarketBucketReasonCode::ProfileRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(MarketBucketProfileEvidence {
            profile_id: canonical_profile.profile_id,
            market_id: canonical_profile.market_id,
            cluster_id: canonical_profile.cluster_id,
            bucket_type: canonical_profile.bucket_type,
            risk_policy_key: canonical_profile.risk_policy_key,
            allocation_policy_key: canonical_profile.allocation_policy_key,
            is_active: canonical_profile.is_active,
            actor_id: input.actor_id,
            correlation_id: input.correlation_id,
            reason_code: MarketBucketReasonCode::ProfileRead.code().to_string(),
            updated_at_utc: input.queried_at_utc,
        })
    }
}

fn validate_market_policy_role(role: &str) -> Result<(), MarketPolicyServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(MarketPolicyServiceError::unauthorized_role()),
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), MarketPolicyServiceError> {
    if value.trim().is_empty() {
        return Err(MarketPolicyServiceError::invalid_payload(
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

fn normalize_market_bucket_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn emit_market_policy_telemetry(
    event_name: &'static str,
    outcome: &'static str,
    actor_id: &str,
    cluster_id: &str,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let action = match event_name {
        "market_policy_profile_update_v1" => "market_policy_profile_update",
        "market_policy_cluster_toggle_v1" => "market_policy_cluster_toggle",
        _ => "market_policy_decision",
    };
    let event = MarketPolicyTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        cluster_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(MarketPolicySecuritySignal {
                name: "market_policy_denied_v1",
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
        serde_json::to_string(&event).expect("market policy telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct MarketPolicyTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    cluster_id: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<MarketPolicySecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct MarketPolicySecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresMarketPolicyRepository {
    pool: PgPool,
}

impl PostgresMarketPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, MarketPolicyServiceError>
    where
        F: Future<Output = Result<T, MarketPolicyPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_market_policy_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    MarketPolicyServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_market_policy_persistence_error),
        }
    }

    fn run_market_bucket_with_runtime<F, T>(&self, future: F) -> Result<T, MarketPolicyServiceError>
    where
        F: Future<Output = Result<T, MarketBucketPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_market_bucket_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    MarketPolicyServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_market_bucket_persistence_error),
        }
    }
}

impl MarketPolicyRepositoryPort for PostgresMarketPolicyRepository {
    fn upsert_profile(&self, profile: MarketPolicyProfile) -> Result<(), MarketPolicyServiceError> {
        self.run_with_runtime(pg_upsert_profile(&self.pool, &profile))
    }

    fn upsert_bucket_profile(
        &self,
        profile: MarketBucketProfile,
    ) -> Result<(), MarketPolicyServiceError> {
        self.run_market_bucket_with_runtime(pg_upsert_market_bucket_profile(&self.pool, &profile))
    }

    fn upsert_cluster_override(
        &self,
        cluster_override: MarketClusterOverride,
    ) -> Result<(), MarketPolicyServiceError> {
        self.run_with_runtime(pg_upsert_cluster_override(&self.pool, &cluster_override))
    }

    fn load_active_profile(
        &self,
        cluster_id: &str,
    ) -> Result<Option<MarketPolicyProfile>, MarketPolicyServiceError> {
        self.run_with_runtime(pg_load_active_profile(&self.pool, cluster_id))
    }

    fn load_active_bucket_profile(
        &self,
        market_id: &str,
        cluster_id: &str,
    ) -> Result<Option<MarketBucketProfile>, MarketPolicyServiceError> {
        self.run_market_bucket_with_runtime(pg_load_active_bucket_profile(
            &self.pool,
            market_id,
            cluster_id,
        ))
    }

    fn load_cluster_override(
        &self,
        cluster_id: &str,
    ) -> Result<Option<MarketClusterOverride>, MarketPolicyServiceError> {
        self.run_with_runtime(pg_load_cluster_override(&self.pool, cluster_id))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryMarketPolicyRepository {
    profiles: Mutex<BTreeMap<String, MarketPolicyProfile>>,
    bucket_profiles: Mutex<BTreeMap<String, MarketBucketProfile>>,
    overrides: Mutex<BTreeMap<String, MarketClusterOverride>>,
}

impl MarketPolicyRepositoryPort for InMemoryMarketPolicyRepository {
    fn upsert_profile(&self, profile: MarketPolicyProfile) -> Result<(), MarketPolicyServiceError> {
        let mut profiles = self
            .profiles
            .lock()
            .expect("in-memory market policy profiles lock should not be poisoned");

        let normalized_cluster = profile.cluster_id.trim().to_ascii_lowercase();
        if profile.is_active
            && profiles.values().any(|existing| {
                existing.profile_id != profile.profile_id
                    && existing.is_active
                    && existing
                        .cluster_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_cluster)
            })
        {
            return Err(MarketPolicyServiceError {
                code: "market_policy_constraint_violation",
                message: format!(
                    "active market policy profile already exists for cluster `{}`",
                    profile.cluster_id
                ),
                field_errors: vec![MarketPolicyValidationIssue {
                    field: "cluster_id",
                    code: "market_policy_constraint_violation",
                    message: "only one active profile is allowed per cluster at a time".to_string(),
                }],
            });
        }

        profiles.insert(profile.profile_id.clone(), profile);
        Ok(())
    }

    fn upsert_bucket_profile(
        &self,
        profile: MarketBucketProfile,
    ) -> Result<(), MarketPolicyServiceError> {
        let mut profiles = self
            .bucket_profiles
            .lock()
            .expect("in-memory market bucket profiles lock should not be poisoned");
        let normalized_market = profile.market_id.trim().to_ascii_lowercase();
        let normalized_cluster = profile.cluster_id.trim().to_ascii_lowercase();

        if profile.is_active
            && profiles.values().any(|existing| {
                existing.profile_id != profile.profile_id
                    && existing.is_active
                    && existing
                        .market_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_market)
                    && existing
                        .cluster_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_cluster)
            })
        {
            return Err(MarketPolicyServiceError {
                code: "market_bucket_constraint_violation",
                message: format!(
                    "active market bucket profile already exists for market `{}` cluster `{}`",
                    profile.market_id, profile.cluster_id
                ),
                field_errors: vec![MarketPolicyValidationIssue {
                    field: "market_id",
                    code: "market_bucket_constraint_violation",
                    message:
                        "only one active bucket profile is allowed per market and cluster at a time"
                            .to_string(),
                }],
            });
        }

        profiles.insert(profile.profile_id.clone(), profile);
        Ok(())
    }

    fn upsert_cluster_override(
        &self,
        cluster_override: MarketClusterOverride,
    ) -> Result<(), MarketPolicyServiceError> {
        let mut overrides = self
            .overrides
            .lock()
            .expect("in-memory market cluster overrides lock should not be poisoned");
        overrides.insert(
            cluster_override.cluster_id.trim().to_ascii_lowercase(),
            cluster_override,
        );
        Ok(())
    }

    fn load_active_profile(
        &self,
        cluster_id: &str,
    ) -> Result<Option<MarketPolicyProfile>, MarketPolicyServiceError> {
        let profiles = self
            .profiles
            .lock()
            .expect("in-memory market policy profiles lock should not be poisoned");
        let normalized_cluster = cluster_id.trim().to_ascii_lowercase();
        let selected = profiles
            .values()
            .filter(|profile| {
                profile.is_active
                    && profile
                        .cluster_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_cluster)
            })
            .max_by(|left, right| {
                left.updated_at_utc
                    .cmp(&right.updated_at_utc)
                    .then(left.profile_id.cmp(&right.profile_id))
            })
            .cloned();
        Ok(selected)
    }

    fn load_active_bucket_profile(
        &self,
        market_id: &str,
        cluster_id: &str,
    ) -> Result<Option<MarketBucketProfile>, MarketPolicyServiceError> {
        let profiles = self
            .bucket_profiles
            .lock()
            .expect("in-memory market bucket profiles lock should not be poisoned");
        let normalized_market = market_id.trim().to_ascii_lowercase();
        let normalized_cluster = cluster_id.trim().to_ascii_lowercase();
        let selected = profiles
            .values()
            .filter(|profile| {
                profile.is_active
                    && profile
                        .market_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_market)
                    && profile
                        .cluster_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_cluster)
            })
            .max_by(|left, right| {
                left.updated_at_utc
                    .cmp(&right.updated_at_utc)
                    .then(left.profile_id.cmp(&right.profile_id))
            })
            .cloned();
        Ok(selected)
    }

    fn load_cluster_override(
        &self,
        cluster_id: &str,
    ) -> Result<Option<MarketClusterOverride>, MarketPolicyServiceError> {
        let overrides = self
            .overrides
            .lock()
            .expect("in-memory market cluster overrides lock should not be poisoned");
        Ok(overrides
            .get(&cluster_id.trim().to_ascii_lowercase())
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_input() -> UpsertMarketPolicyProfileInput {
        UpsertMarketPolicyProfileInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            min_liquidity_usd: 500.0,
            max_spread_bps: 2.5,
            min_reward_score: 0.2,
            max_exposure_pct_nav: 20.0,
            correlation_id: "corr-profile-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn toggle_input() -> ToggleMarketClusterInput {
        ToggleMarketClusterInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            is_enabled: false,
            reason_code: Some(
                MarketPolicyReasonCode::ClusterDisabledByOperator
                    .code()
                    .to_string(),
            ),
            correlation_id: "corr-toggle-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn bucket_profile_input() -> UpsertMarketBucketProfileInput {
        UpsertMarketBucketProfileInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            bucket_type: "core".to_string(),
            risk_policy_key: "core-risk-default".to_string(),
            allocation_policy_key: "core-allocation-default".to_string(),
            correlation_id: "corr-bucket-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    fn read_bucket_profile_input() -> ReadMarketBucketProfileInput {
        ReadMarketBucketProfileInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            correlation_id: "corr-bucket-read-001".to_string(),
            queried_at_utc: "2026-04-07T00:05:00Z".to_string(),
        }
    }

    #[test]
    fn profile_update_rejects_invalid_thresholds_with_field_errors() {
        let service = MarketPolicyService::default();
        let mut input = profile_input();
        input.max_exposure_pct_nav = 100.1;

        let error = service
            .upsert_market_policy_profile(input)
            .expect_err("invalid thresholds should fail");
        assert_eq!(error.code, MarketPolicyReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "max_exposure_pct_nav")
        );
    }

    #[test]
    fn cluster_toggle_blocks_unauthorized_roles() {
        let service = MarketPolicyService::default();
        let mut input = toggle_input();
        input.actor_role = "read_only_analytics".to_string();

        let error = service
            .toggle_market_cluster(input)
            .expect_err("unauthorized role should fail closed");
        assert_eq!(error.code, "market_policy_unauthorized_role");
    }

    #[test]
    fn cluster_toggle_returns_success_evidence() {
        let service = MarketPolicyService::default();
        let evidence = service
            .toggle_market_cluster(toggle_input())
            .expect("cluster toggle should succeed");

        assert_eq!(evidence.cluster_id, "cluster_alpha");
        assert!(!evidence.is_enabled);
        assert_eq!(
            evidence.reason_code,
            MarketPolicyReasonCode::ClusterDisabledByOperator.code()
        );
    }

    #[test]
    fn profile_update_returns_machine_readable_success_evidence() {
        let service = MarketPolicyService::default();
        let evidence = service
            .upsert_market_policy_profile(profile_input())
            .expect("valid profile should upsert successfully");

        assert_eq!(evidence.cluster_id, "cluster_alpha");
        assert_eq!(evidence.profile_id, "policy::cluster_alpha");
        assert_eq!(
            evidence.reason_code,
            MarketPolicyReasonCode::ProfileUpdated.code()
        );
    }

    #[test]
    fn profile_update_uses_non_lossy_profile_identity_for_distinct_clusters() {
        let service = MarketPolicyService::default();

        let mut slash_cluster = profile_input();
        slash_cluster.cluster_id = " Cluster/A ".to_string();
        slash_cluster.correlation_id = "corr-profile-slash".to_string();
        let slash_evidence = service
            .upsert_market_policy_profile(slash_cluster)
            .expect("slash cluster policy should upsert");

        let mut underscore_cluster = profile_input();
        underscore_cluster.cluster_id = "cluster_a".to_string();
        underscore_cluster.correlation_id = "corr-profile-underscore".to_string();
        let underscore_evidence = service
            .upsert_market_policy_profile(underscore_cluster)
            .expect("underscore cluster policy should upsert");

        assert_eq!(slash_evidence.cluster_id, "cluster/a");
        assert_eq!(slash_evidence.profile_id, "policy::cluster/a");
        assert_eq!(underscore_evidence.cluster_id, "cluster_a");
        assert_eq!(underscore_evidence.profile_id, "policy::cluster_a");
        assert_ne!(slash_evidence.profile_id, underscore_evidence.profile_id);
    }

    #[test]
    fn cluster_toggle_rejects_unknown_reason_code() {
        let service = MarketPolicyService::default();
        let mut input = toggle_input();
        input.reason_code = Some("cluster_reason_unknown".to_string());

        let error = service
            .toggle_market_cluster(input)
            .expect_err("unknown reason codes should fail");
        assert_eq!(error.code, MarketPolicyReasonCode::InvalidPayload.code());
    }

    #[test]
    fn cluster_toggle_normalizes_cluster_id_before_persistence() {
        let service = MarketPolicyService::default();
        let mut input = toggle_input();
        input.cluster_id = " Cluster_Alpha ".to_string();
        let evidence = service
            .toggle_market_cluster(input)
            .expect("toggle should normalize cluster identifier");

        assert_eq!(evidence.cluster_id, "cluster_alpha");
    }

    #[test]
    fn invalid_profile_update_does_not_partially_overwrite_previous_profile_state() {
        let repository = Arc::new(InMemoryMarketPolicyRepository::default());
        let service = MarketPolicyService::new(repository.clone());

        let baseline = service
            .upsert_market_policy_profile(profile_input())
            .expect("baseline policy profile should persist");
        assert_eq!(baseline.max_exposure_pct_nav, 20.0);

        let mut invalid_update = profile_input();
        invalid_update.max_exposure_pct_nav = 100.1;
        invalid_update.updated_at_utc = "2026-04-06T00:10:00Z".to_string();
        service
            .upsert_market_policy_profile(invalid_update)
            .expect_err("invalid update should fail validation and avoid partial persistence");

        let persisted = repository
            .load_active_profile("cluster_alpha")
            .expect("profile lookup should succeed")
            .expect("cluster profile should remain present");
        assert_eq!(persisted.max_exposure_pct_nav, 20.0);
    }

    #[test]
    fn bucket_profile_update_rejects_unsupported_bucket_type() {
        let service = MarketPolicyService::default();
        let mut input = bucket_profile_input();
        input.bucket_type = "growth".to_string();

        let error = service
            .upsert_market_bucket_profile(input)
            .expect_err("unsupported bucket type should fail validation");
        assert_eq!(error.code, MarketBucketReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "bucket_type")
        );
    }

    #[test]
    fn bucket_profile_update_returns_machine_readable_success_evidence() {
        let service = MarketPolicyService::default();
        let evidence = service
            .upsert_market_bucket_profile(bucket_profile_input())
            .expect("bucket profile update should succeed");

        assert_eq!(evidence.profile_id, "bucket::market_yes_no_1::cluster_alpha");
        assert_eq!(evidence.bucket_type, "core");
        assert_eq!(evidence.risk_policy_key, "core-risk-default");
        assert_eq!(evidence.allocation_policy_key, "core-allocation-default");
        assert_eq!(
            evidence.reason_code,
            MarketBucketReasonCode::ProfileUpdated.code()
        );
    }

    #[test]
    fn bucket_profile_read_returns_profile_for_market_cluster_mapping() {
        let service = MarketPolicyService::default();
        service
            .upsert_market_bucket_profile(bucket_profile_input())
            .expect("bucket profile should seed read path");

        let evidence = service
            .read_market_bucket_profile(read_bucket_profile_input())
            .expect("bucket profile read should succeed");

        assert_eq!(evidence.profile_id, "bucket::market_yes_no_1::cluster_alpha");
        assert_eq!(evidence.market_id, "market_yes_no_1");
        assert_eq!(evidence.cluster_id, "cluster_alpha");
        assert_eq!(
            evidence.reason_code,
            MarketBucketReasonCode::ProfileRead.code()
        );
    }

    #[test]
    fn bucket_profile_read_fails_closed_when_mapping_missing() {
        let service = MarketPolicyService::default();

        let error = service
            .read_market_bucket_profile(read_bucket_profile_input())
            .expect_err("missing mapping should fail closed");
        assert_eq!(error.code, MarketBucketReasonCode::MappingUnavailable.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.code == MarketBucketReasonCode::MappingUnavailable.code())
        );
    }
}
