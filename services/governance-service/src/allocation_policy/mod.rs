use domain::allocation::{
    AllocationApprovalStatus, AllocationPolicyVersion, AllocationValidationIssue,
    DriftEvaluationOutcome, RebalanceReasonCode, RebalanceRecommendation,
    RebalanceRecommendationStatus, evaluate_rebalance_drift, normalize_allocation_identifier,
    requires_allocation_critical_increase_approval, validate_allocation_policy_version,
    validate_rebalance_recommendation,
};
use persistence::postgres::allocation_policies::{
    AllocationPersistenceError, load_active_allocation_policy_version as pg_load_active_policy,
    load_latest_rebalance_recommendation_for_policy as pg_load_latest_recommendation_for_policy,
    load_pending_rebalance_recommendations as pg_load_pending_recommendations,
    load_rebalance_recommendation_by_id as pg_load_recommendation_by_id,
    upsert_allocation_policy_version as pg_upsert_policy,
    upsert_rebalance_recommendation as pg_upsert_recommendation,
};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AllocationPolicyServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<AllocationValidationIssue>,
}

impl AllocationPolicyServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AllocationValidationIssue>,
    ) -> Self {
        Self {
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: "allocation_policy_unauthorized_role",
            message: "actor role is not authorized for allocation policy workflows".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RebalanceReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for AllocationPolicyServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AllocationPolicyServiceError {}

fn map_persistence_error(error: AllocationPersistenceError) -> AllocationPolicyServiceError {
    match error.code {
        "allocation_query_failed" | "allocation_row_decode_failed" => {
            AllocationPolicyServiceError::persistence_unavailable(error.message)
        }
        _ => AllocationPolicyServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct UpsertAllocationPolicyInput {
    pub actor_id: String,
    pub actor_role: String,
    pub policy_key: String,
    pub version: i64,
    pub portfolio_scope_id: String,
    pub target_exposure_pct_nav: f64,
    pub target_relative_alpha_weight: f64,
    pub exposure_drift_threshold_pct: f64,
    pub relative_alpha_drift_threshold_pct: f64,
    pub advanced_parameters: Value,
    pub correlation_id: String,
    pub updated_at_utc: String,
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EvaluateRebalanceDriftInput {
    pub actor_id: String,
    pub actor_role: String,
    pub policy_key: String,
    pub exposure_drift_pct: f64,
    pub relative_alpha_drift_pct: f64,
    pub observed_at_utc: String,
    pub stale_after_seconds: f64,
    pub correlation_id: String,
    pub require_execution: bool,
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecuteRebalanceRecommendationInput {
    pub actor_id: String,
    pub actor_role: String,
    pub recommendation_id: String,
    pub correlation_id: String,
    pub executed_at_utc: String,
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingRebalanceRecommendationsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
    pub policy_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AllocationPolicyMutationEvidence {
    pub policy_key: String,
    pub version: i64,
    pub approval_status: String,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RebalanceRecommendationEvidence {
    pub recommendation_id: String,
    pub policy_key: String,
    pub policy_version: i64,
    pub status: String,
    pub approval_status: String,
    pub action_type: String,
    pub rationale: String,
    pub recommended_next_action: String,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

pub trait AllocationPolicyRepositoryPort: Send + Sync {
    fn upsert_policy(
        &self,
        policy: AllocationPolicyVersion,
    ) -> Result<(), AllocationPolicyServiceError>;
    fn load_active_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<AllocationPolicyVersion>, AllocationPolicyServiceError>;
    fn upsert_recommendation(
        &self,
        recommendation: RebalanceRecommendation,
    ) -> Result<(), AllocationPolicyServiceError>;
    fn load_pending_recommendations(
        &self,
        policy_key: Option<&str>,
    ) -> Result<Vec<RebalanceRecommendation>, AllocationPolicyServiceError>;
    fn load_latest_recommendation_for_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<RebalanceRecommendation>, AllocationPolicyServiceError>;
    fn load_recommendation_by_id(
        &self,
        recommendation_id: &str,
    ) -> Result<Option<RebalanceRecommendation>, AllocationPolicyServiceError>;
}

pub trait AllocationPolicyOrchestrator: Send + Sync {
    fn upsert_allocation_policy(
        &self,
        input: UpsertAllocationPolicyInput,
    ) -> Result<AllocationPolicyMutationEvidence, AllocationPolicyServiceError>;
    fn evaluate_rebalance_drift(
        &self,
        input: EvaluateRebalanceDriftInput,
    ) -> Result<RebalanceRecommendationEvidence, AllocationPolicyServiceError>;
    fn execute_rebalance_recommendation(
        &self,
        input: ExecuteRebalanceRecommendationInput,
    ) -> Result<RebalanceRecommendationEvidence, AllocationPolicyServiceError>;
    fn list_pending_rebalance_recommendations(
        &self,
        input: PendingRebalanceRecommendationsInput,
    ) -> Result<Vec<RebalanceRecommendationEvidence>, AllocationPolicyServiceError>;
}

#[derive(Clone)]
pub struct AllocationPolicyService {
    repository: Arc<dyn AllocationPolicyRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl AllocationPolicyService {
    pub fn new(repository: Arc<dyn AllocationPolicyRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryAllocationPolicyRepository::default()))
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresAllocationPolicyRepository::new(pool)))
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, AllocationPolicyServiceError> {
        self.operation_lock.lock().map_err(|_| {
            AllocationPolicyServiceError::persistence_unavailable(
                "allocation policy operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for AllocationPolicyService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl AllocationPolicyOrchestrator for AllocationPolicyService {
    fn upsert_allocation_policy(
        &self,
        input: UpsertAllocationPolicyInput,
    ) -> Result<AllocationPolicyMutationEvidence, AllocationPolicyServiceError> {
        if let Err(error) = validate_allocation_role(&input.actor_role) {
            emit_allocation_telemetry(
                "allocation_policy_upsert_v1",
                "deny",
                &input.actor_id,
                &input.policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("policy_key", &input.policy_key)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("updated_at_utc", &input.updated_at_utc)?;

        let _lock = self.lock_operations()?;
        let normalized_policy_key = normalize_allocation_identifier(&input.policy_key);
        let active_policy = self.repository.load_active_policy(&normalized_policy_key)?;

        let critical_increase = requires_allocation_critical_increase_approval(
            active_policy.as_ref(),
            &AllocationPolicyVersion {
                policy_key: normalized_policy_key.clone(),
                version: input.version,
                portfolio_scope_id: normalize_allocation_identifier(&input.portfolio_scope_id),
                target_exposure_pct_nav: input.target_exposure_pct_nav,
                target_relative_alpha_weight: input.target_relative_alpha_weight,
                exposure_drift_threshold_pct: input.exposure_drift_threshold_pct,
                relative_alpha_drift_threshold_pct: input.relative_alpha_drift_threshold_pct,
                approval_status: AllocationApprovalStatus::Approved,
                approval_reference: input.approval_reference.clone(),
                actor_id: input.actor_id.clone(),
                reason_code: RebalanceReasonCode::AllocationPolicyUpdated
                    .code()
                    .to_string(),
                correlation_id: input.correlation_id.clone(),
                updated_at_utc: input.updated_at_utc.clone(),
                advanced_parameters: input.advanced_parameters.clone(),
            },
        );

        let approval_status = if critical_increase && input.approval_reference.is_none() {
            AllocationApprovalStatus::Pending
        } else {
            AllocationApprovalStatus::Approved
        };
        let reason_code = if approval_status == AllocationApprovalStatus::Pending {
            RebalanceReasonCode::AllocationPolicyPendingApproval.code()
        } else {
            RebalanceReasonCode::AllocationPolicyUpdated.code()
        };

        let policy = AllocationPolicyVersion {
            policy_key: normalized_policy_key.clone(),
            version: input.version,
            portfolio_scope_id: normalize_allocation_identifier(&input.portfolio_scope_id),
            target_exposure_pct_nav: input.target_exposure_pct_nav,
            target_relative_alpha_weight: input.target_relative_alpha_weight,
            exposure_drift_threshold_pct: input.exposure_drift_threshold_pct,
            relative_alpha_drift_threshold_pct: input.relative_alpha_drift_threshold_pct,
            approval_status,
            approval_reference: input.approval_reference.clone(),
            actor_id: input.actor_id.clone(),
            reason_code: reason_code.to_string(),
            correlation_id: input.correlation_id.clone(),
            updated_at_utc: input.updated_at_utc.clone(),
            advanced_parameters: input.advanced_parameters,
        };
        validate_allocation_policy_version(&policy).map_err(|error| {
            AllocationPolicyServiceError::invalid_payload(error.message, error.field_errors)
        })?;

        self.repository
            .upsert_policy(policy.clone())
            .inspect_err(|error| {
                emit_allocation_telemetry(
                    "allocation_policy_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        emit_allocation_telemetry(
            "allocation_policy_upsert_v1",
            if policy.approval_status == AllocationApprovalStatus::Pending {
                "pending"
            } else {
                "allow"
            },
            &input.actor_id,
            &normalized_policy_key,
            reason_code,
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(AllocationPolicyMutationEvidence {
            policy_key: policy.policy_key,
            version: policy.version,
            approval_status: policy.approval_status.as_str().to_string(),
            actor_id: policy.actor_id,
            reason_code: policy.reason_code,
            correlation_id: policy.correlation_id,
            updated_at_utc: policy.updated_at_utc,
            approval_reference: policy.approval_reference,
        })
    }

    fn evaluate_rebalance_drift(
        &self,
        input: EvaluateRebalanceDriftInput,
    ) -> Result<RebalanceRecommendationEvidence, AllocationPolicyServiceError> {
        if let Err(error) = validate_allocation_role(&input.actor_role) {
            emit_allocation_telemetry(
                "rebalance_recommendation_evaluate_v1",
                "deny",
                &input.actor_id,
                &input.policy_key,
                error.code,
                &input.correlation_id,
                &input.observed_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("policy_key", &input.policy_key)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("observed_at_utc", &input.observed_at_utc)?;

        let _lock = self.lock_operations()?;
        let normalized_policy_key = normalize_allocation_identifier(&input.policy_key);
        let policy = self.repository.load_active_policy(&normalized_policy_key)?;
        let evaluation = evaluate_rebalance_drift(
            policy.as_ref(),
            input.exposure_drift_pct,
            input.relative_alpha_drift_pct,
            &input.observed_at_utc,
            input.stale_after_seconds,
        )
        .map_err(|error| {
            AllocationPolicyServiceError::invalid_payload(error.message, error.field_errors)
        })?;

        if evaluation.outcome == DriftEvaluationOutcome::FailClosed {
            let parsed_reason = RebalanceReasonCode::parse(&evaluation.reason_code)
                .unwrap_or(RebalanceReasonCode::InvalidPayload);
            let error = AllocationPolicyServiceError {
                code: parsed_reason.code(),
                message:
                    "rebalance evaluation failed closed due to unavailable or stale policy state"
                        .to_string(),
                field_errors: Vec::new(),
            };
            emit_allocation_telemetry(
                "rebalance_recommendation_evaluate_v1",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.observed_at_utc,
            );
            return Err(error);
        }

        let Some(policy) = policy else {
            return Err(AllocationPolicyServiceError {
                code: RebalanceReasonCode::PolicyStateUnavailable.code(),
                message: "allocation policy state is unavailable".to_string(),
                field_errors: Vec::new(),
            });
        };

        if evaluation.outcome == DriftEvaluationOutcome::InBounds {
            let evidence = RebalanceRecommendationEvidence {
                recommendation_id: build_recommendation_id(
                    &policy.policy_key,
                    policy.version,
                    &input.correlation_id,
                ),
                policy_key: policy.policy_key.clone(),
                policy_version: policy.version,
                status: RebalanceRecommendationStatus::Denied.as_str().to_string(),
                approval_status: AllocationApprovalStatus::NotRequired.as_str().to_string(),
                action_type: "recommend".to_string(),
                rationale:
                    "Drift remains within configured thresholds; no rebalance recommendation generated."
                        .to_string(),
                recommended_next_action:
                    "Continue monitoring drift telemetry; no rebalance action is required."
                        .to_string(),
                actor_id: input.actor_id.clone(),
                reason_code: RebalanceReasonCode::InBounds.code().to_string(),
                correlation_id: input.correlation_id.clone(),
                created_at_utc: input.observed_at_utc.clone(),
                updated_at_utc: input.observed_at_utc.clone(),
                approval_reference: None,
            };
            emit_allocation_telemetry(
                "rebalance_recommendation_evaluate_v1",
                "allow",
                &input.actor_id,
                &policy.policy_key,
                RebalanceReasonCode::InBounds.code(),
                &input.correlation_id,
                &input.observed_at_utc,
            );
            return Ok(evidence);
        }

        let recommendation_id =
            build_recommendation_id(&policy.policy_key, policy.version, &input.correlation_id);
        let (status, approval_status, reason_code, recommended_next_action) = if input
            .require_execution
            && input.approval_reference.is_none()
        {
            (
                RebalanceRecommendationStatus::PendingApproval,
                AllocationApprovalStatus::Pending,
                RebalanceReasonCode::ApprovalRequired,
                "Complete dual approval for rebalance execution, then re-run execution with approval evidence.",
            )
        } else if input.require_execution {
            (
                RebalanceRecommendationStatus::Approved,
                AllocationApprovalStatus::Approved,
                RebalanceReasonCode::RecommendationApproved,
                "Execute the approved rebalance recommendation and verify post-trade drift contraction.",
            )
        } else {
            (
                RebalanceRecommendationStatus::Proposed,
                AllocationApprovalStatus::NotRequired,
                RebalanceReasonCode::RecommendationProposed,
                "Review rationale and execute rebalance if portfolio intent remains valid.",
            )
        };

        let recommendation = RebalanceRecommendation {
            recommendation_id: recommendation_id.clone(),
            policy_key: policy.policy_key.clone(),
            policy_version: policy.version,
            status,
            approval_status,
            approval_reference: input.approval_reference.clone(),
            action_type: if input.require_execution {
                "execute".to_string()
            } else {
                "recommend".to_string()
            },
            rationale: format!(
                "Drift exceeded threshold (exposure: {}%, alpha: {}%; thresholds: {}% / {}%).",
                input.exposure_drift_pct,
                input.relative_alpha_drift_pct,
                evaluation.exposure_threshold_pct,
                evaluation.relative_alpha_threshold_pct
            ),
            reason_code: reason_code.code().to_string(),
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            created_at_utc: input.observed_at_utc.clone(),
            updated_at_utc: input.observed_at_utc.clone(),
            parameters: json!({
                "exposure_drift_pct": input.exposure_drift_pct,
                "relative_alpha_drift_pct": input.relative_alpha_drift_pct,
                "require_execution": input.require_execution,
            }),
            evidence: json!({
                "policy_key": policy.policy_key,
                "policy_version": policy.version,
                "thresholds": {
                    "exposure_drift_threshold_pct": evaluation.exposure_threshold_pct,
                    "relative_alpha_drift_threshold_pct": evaluation.relative_alpha_threshold_pct
                },
                "exceeded_dimensions": evaluation.exceeded_dimensions,
                "actor_id": input.actor_id,
                "correlation_id": input.correlation_id,
            }),
        };
        validate_rebalance_recommendation(&recommendation).map_err(|error| {
            AllocationPolicyServiceError::invalid_payload(error.message, error.field_errors)
        })?;
        self.repository
            .upsert_recommendation(recommendation.clone())
            .inspect_err(|error| {
                emit_allocation_telemetry(
                    "rebalance_recommendation_evaluate_v1",
                    "deny",
                    &recommendation.actor_id,
                    &recommendation.policy_key,
                    error.code,
                    &recommendation.correlation_id,
                    &recommendation.updated_at_utc,
                );
            })?;

        emit_allocation_telemetry(
            "rebalance_recommendation_evaluate_v1",
            if recommendation.approval_status == AllocationApprovalStatus::Pending {
                "pending"
            } else {
                "allow"
            },
            &recommendation.actor_id,
            &recommendation.policy_key,
            &recommendation.reason_code,
            &recommendation.correlation_id,
            &recommendation.updated_at_utc,
        );

        Ok(RebalanceRecommendationEvidence {
            recommendation_id: recommendation.recommendation_id,
            policy_key: recommendation.policy_key,
            policy_version: recommendation.policy_version,
            status: recommendation.status.as_str().to_string(),
            approval_status: recommendation.approval_status.as_str().to_string(),
            action_type: recommendation.action_type,
            rationale: recommendation.rationale,
            recommended_next_action: recommended_next_action.to_string(),
            actor_id: recommendation.actor_id,
            reason_code: recommendation.reason_code,
            correlation_id: recommendation.correlation_id,
            created_at_utc: recommendation.created_at_utc,
            updated_at_utc: recommendation.updated_at_utc,
            approval_reference: recommendation.approval_reference,
        })
    }

    fn execute_rebalance_recommendation(
        &self,
        input: ExecuteRebalanceRecommendationInput,
    ) -> Result<RebalanceRecommendationEvidence, AllocationPolicyServiceError> {
        if let Err(error) = validate_allocation_role(&input.actor_role) {
            emit_allocation_telemetry(
                "rebalance_recommendation_execute_v1",
                "deny",
                &input.actor_id,
                "unknown_policy",
                error.code,
                &input.correlation_id,
                &input.executed_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("recommendation_id", &input.recommendation_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("executed_at_utc", &input.executed_at_utc)?;

        let _lock = self.lock_operations()?;
        let normalized_id = normalize_allocation_identifier(&input.recommendation_id);
        let Some(mut recommendation) = self.repository.load_recommendation_by_id(&normalized_id)?
        else {
            return Err(AllocationPolicyServiceError {
                code: RebalanceReasonCode::RecommendationNotFound.code(),
                message: "rebalance recommendation not found".to_string(),
                field_errors: Vec::new(),
            });
        };

        if recommendation.status == RebalanceRecommendationStatus::Denied {
            return Err(AllocationPolicyServiceError {
                code: RebalanceReasonCode::RecommendationDenied.code(),
                message: "denied recommendations cannot be executed".to_string(),
                field_errors: Vec::new(),
            });
        }

        if recommendation.approval_status == AllocationApprovalStatus::Pending {
            let approval_reference = input
                .approval_reference
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AllocationPolicyServiceError {
                    code: RebalanceReasonCode::ApprovalRequired.code(),
                    message:
                        "rebalance execution requires approval_reference for pending recommendation"
                            .to_string(),
                    field_errors: Vec::new(),
                })?;
            recommendation.approval_status = AllocationApprovalStatus::Approved;
            recommendation.approval_reference = Some(approval_reference.to_string());
            recommendation.status = RebalanceRecommendationStatus::Approved;
            recommendation.reason_code = RebalanceReasonCode::RecommendationApproved
                .code()
                .to_string();
        }

        recommendation.status = RebalanceRecommendationStatus::Executed;
        recommendation.approval_status =
            if recommendation.approval_status == AllocationApprovalStatus::NotRequired {
                AllocationApprovalStatus::NotRequired
            } else {
                AllocationApprovalStatus::Approved
            };
        recommendation.action_type = "execute".to_string();
        recommendation.reason_code = RebalanceReasonCode::RecommendationExecuted
            .code()
            .to_string();
        recommendation.actor_id = input.actor_id.clone();
        recommendation.correlation_id = input.correlation_id.clone();
        recommendation.updated_at_utc = input.executed_at_utc.clone();

        validate_rebalance_recommendation(&recommendation).map_err(|error| {
            AllocationPolicyServiceError::invalid_payload(error.message, error.field_errors)
        })?;
        self.repository
            .upsert_recommendation(recommendation.clone())
            .inspect_err(|error| {
                emit_allocation_telemetry(
                    "rebalance_recommendation_execute_v1",
                    "deny",
                    &input.actor_id,
                    &recommendation.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.executed_at_utc,
                );
            })?;

        emit_allocation_telemetry(
            "rebalance_recommendation_execute_v1",
            "allow",
            &input.actor_id,
            &recommendation.policy_key,
            &recommendation.reason_code,
            &input.correlation_id,
            &input.executed_at_utc,
        );

        Ok(RebalanceRecommendationEvidence {
            recommendation_id: recommendation.recommendation_id,
            policy_key: recommendation.policy_key,
            policy_version: recommendation.policy_version,
            status: recommendation.status.as_str().to_string(),
            approval_status: recommendation.approval_status.as_str().to_string(),
            action_type: recommendation.action_type,
            rationale: recommendation.rationale,
            recommended_next_action:
                "Monitor post-execution drift and confirm correlation IDs in audit records."
                    .to_string(),
            actor_id: recommendation.actor_id,
            reason_code: recommendation.reason_code,
            correlation_id: recommendation.correlation_id,
            created_at_utc: recommendation.created_at_utc,
            updated_at_utc: recommendation.updated_at_utc,
            approval_reference: recommendation.approval_reference,
        })
    }

    fn list_pending_rebalance_recommendations(
        &self,
        input: PendingRebalanceRecommendationsInput,
    ) -> Result<Vec<RebalanceRecommendationEvidence>, AllocationPolicyServiceError> {
        validate_allocation_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;

        let policy_key = input
            .policy_key
            .as_deref()
            .map(normalize_allocation_identifier);
        let pending = self
            .repository
            .load_pending_recommendations(policy_key.as_deref())?;

        emit_allocation_telemetry(
            "rebalance_pending_query_v1",
            "allow",
            &input.actor_id,
            policy_key.as_deref().unwrap_or("all_policies"),
            RebalanceReasonCode::ApprovalRequired.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(pending
            .into_iter()
            .map(|recommendation| RebalanceRecommendationEvidence {
                recommendation_id: recommendation.recommendation_id,
                policy_key: recommendation.policy_key,
                policy_version: recommendation.policy_version,
                status: recommendation.status.as_str().to_string(),
                approval_status: recommendation.approval_status.as_str().to_string(),
                action_type: recommendation.action_type,
                rationale: recommendation.rationale,
                recommended_next_action:
                    "Complete dual approval and execute recommendation when governance evidence is ready."
                        .to_string(),
                actor_id: recommendation.actor_id,
                reason_code: recommendation.reason_code,
                correlation_id: recommendation.correlation_id,
                created_at_utc: recommendation.created_at_utc,
                updated_at_utc: recommendation.updated_at_utc,
                approval_reference: recommendation.approval_reference,
            })
            .collect())
    }
}

fn build_recommendation_id(policy_key: &str, policy_version: i64, correlation_id: &str) -> String {
    format!(
        "reco::{}::{}::{}",
        normalize_allocation_identifier(policy_key),
        policy_version,
        normalize_allocation_identifier(correlation_id)
    )
}

fn validate_allocation_role(role: &str) -> Result<(), AllocationPolicyServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(AllocationPolicyServiceError::unauthorized_role()),
    }
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), AllocationPolicyServiceError> {
    if value.trim().is_empty() {
        return Err(AllocationPolicyServiceError::invalid_payload(
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

fn emit_allocation_telemetry(
    event_name: &'static str,
    outcome: &'static str,
    actor_id: &str,
    policy_key: &str,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = AllocationTelemetryEvent {
        event_name,
        action: match event_name {
            "allocation_policy_upsert_v1" => "allocation_policy_upsert",
            "rebalance_recommendation_evaluate_v1" => "rebalance_recommendation_evaluate",
            "rebalance_recommendation_execute_v1" => "rebalance_recommendation_execute",
            "rebalance_pending_query_v1" => "rebalance_pending_query",
            _ => "allocation_decision",
        },
        outcome,
        actor_id,
        policy_key,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(AllocationSecuritySignal {
                name: "allocation_policy_denied_v1",
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
        serde_json::to_string(&event).expect("allocation telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct AllocationTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    policy_key: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<AllocationSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct AllocationSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresAllocationPolicyRepository {
    pool: PgPool,
}

impl PostgresAllocationPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, AllocationPolicyServiceError>
    where
        F: Future<Output = Result<T, AllocationPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AllocationPolicyServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl AllocationPolicyRepositoryPort for PostgresAllocationPolicyRepository {
    fn upsert_policy(
        &self,
        policy: AllocationPolicyVersion,
    ) -> Result<(), AllocationPolicyServiceError> {
        self.run_with_runtime(pg_upsert_policy(&self.pool, &policy))
    }

    fn load_active_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<AllocationPolicyVersion>, AllocationPolicyServiceError> {
        self.run_with_runtime(pg_load_active_policy(&self.pool, policy_key))
    }

    fn upsert_recommendation(
        &self,
        recommendation: RebalanceRecommendation,
    ) -> Result<(), AllocationPolicyServiceError> {
        self.run_with_runtime(pg_upsert_recommendation(&self.pool, &recommendation))
    }

    fn load_pending_recommendations(
        &self,
        policy_key: Option<&str>,
    ) -> Result<Vec<RebalanceRecommendation>, AllocationPolicyServiceError> {
        self.run_with_runtime(pg_load_pending_recommendations(&self.pool, policy_key))
    }

    fn load_latest_recommendation_for_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<RebalanceRecommendation>, AllocationPolicyServiceError> {
        self.run_with_runtime(pg_load_latest_recommendation_for_policy(
            &self.pool, policy_key,
        ))
    }

    fn load_recommendation_by_id(
        &self,
        recommendation_id: &str,
    ) -> Result<Option<RebalanceRecommendation>, AllocationPolicyServiceError> {
        self.run_with_runtime(pg_load_recommendation_by_id(&self.pool, recommendation_id))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAllocationPolicyRepository {
    policies: Mutex<BTreeMap<(String, i64), AllocationPolicyVersion>>,
    recommendations: Mutex<BTreeMap<String, RebalanceRecommendation>>,
}

impl AllocationPolicyRepositoryPort for InMemoryAllocationPolicyRepository {
    fn upsert_policy(
        &self,
        policy: AllocationPolicyVersion,
    ) -> Result<(), AllocationPolicyServiceError> {
        let mut policies = self
            .policies
            .lock()
            .expect("in-memory allocation policy lock should not be poisoned");
        if policy.approval_status == AllocationApprovalStatus::Approved {
            for existing in policies.values_mut() {
                if normalize_allocation_identifier(&existing.policy_key)
                    == normalize_allocation_identifier(&policy.policy_key)
                    && existing.approval_status == AllocationApprovalStatus::Approved
                {
                    existing.approval_status = AllocationApprovalStatus::Denied;
                    existing.reason_code = RebalanceReasonCode::AllocationPolicyDenied
                        .code()
                        .to_string();
                    existing.approval_reference = None;
                }
            }
        }
        policies.insert(
            (
                normalize_allocation_identifier(&policy.policy_key),
                policy.version,
            ),
            policy,
        );
        Ok(())
    }

    fn load_active_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<AllocationPolicyVersion>, AllocationPolicyServiceError> {
        let policies = self
            .policies
            .lock()
            .expect("in-memory allocation policy lock should not be poisoned");
        let normalized = normalize_allocation_identifier(policy_key);
        Ok(policies
            .iter()
            .filter(|((key, _), policy)| {
                *key == normalized && policy.approval_status == AllocationApprovalStatus::Approved
            })
            .max_by_key(|((_, version), _)| *version)
            .map(|(_, policy)| policy.clone()))
    }

    fn upsert_recommendation(
        &self,
        recommendation: RebalanceRecommendation,
    ) -> Result<(), AllocationPolicyServiceError> {
        let mut recommendations = self
            .recommendations
            .lock()
            .expect("in-memory rebalance recommendations lock should not be poisoned");
        recommendations.insert(
            normalize_allocation_identifier(&recommendation.recommendation_id),
            recommendation,
        );
        Ok(())
    }

    fn load_pending_recommendations(
        &self,
        policy_key: Option<&str>,
    ) -> Result<Vec<RebalanceRecommendation>, AllocationPolicyServiceError> {
        let recommendations = self
            .recommendations
            .lock()
            .expect("in-memory rebalance recommendations lock should not be poisoned");
        let filter_key = policy_key.map(normalize_allocation_identifier);
        let mut pending: Vec<_> = recommendations
            .values()
            .filter(|recommendation| {
                recommendation.approval_status == AllocationApprovalStatus::Pending
            })
            .filter(|recommendation| {
                filter_key.as_ref().is_none_or(|expected| {
                    normalize_allocation_identifier(&recommendation.policy_key) == *expected
                })
            })
            .cloned()
            .collect();
        pending.sort_by(|left, right| {
            left.policy_key
                .cmp(&right.policy_key)
                .then(right.created_at_utc.cmp(&left.created_at_utc))
                .then(right.recommendation_id.cmp(&left.recommendation_id))
        });
        Ok(pending)
    }

    fn load_latest_recommendation_for_policy(
        &self,
        policy_key: &str,
    ) -> Result<Option<RebalanceRecommendation>, AllocationPolicyServiceError> {
        let recommendations = self
            .recommendations
            .lock()
            .expect("in-memory rebalance recommendations lock should not be poisoned");
        let normalized = normalize_allocation_identifier(policy_key);
        Ok(recommendations
            .values()
            .filter(|recommendation| {
                normalize_allocation_identifier(&recommendation.policy_key) == normalized
            })
            .max_by(|left, right| {
                left.created_at_utc
                    .cmp(&right.created_at_utc)
                    .then(left.recommendation_id.cmp(&right.recommendation_id))
            })
            .cloned())
    }

    fn load_recommendation_by_id(
        &self,
        recommendation_id: &str,
    ) -> Result<Option<RebalanceRecommendation>, AllocationPolicyServiceError> {
        let recommendations = self
            .recommendations
            .lock()
            .expect("in-memory rebalance recommendations lock should not be poisoned");
        Ok(recommendations
            .get(&normalize_allocation_identifier(recommendation_id))
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_input() -> UpsertAllocationPolicyInput {
        UpsertAllocationPolicyInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            policy_key: "portfolio-default".to_string(),
            version: 1,
            portfolio_scope_id: "portfolio::default".to_string(),
            target_exposure_pct_nav: 40.0,
            target_relative_alpha_weight: 1.2,
            exposure_drift_threshold_pct: 10.0,
            relative_alpha_drift_threshold_pct: 15.0,
            advanced_parameters: json!({
                "execution_window_minutes": 15
            }),
            correlation_id: "corr-allocation-policy-001".to_string(),
            updated_at_utc: "2026-04-06T07:00:00Z".to_string(),
            approval_reference: None,
        }
    }

    fn evaluate_input() -> EvaluateRebalanceDriftInput {
        EvaluateRebalanceDriftInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            policy_key: "portfolio-default".to_string(),
            exposure_drift_pct: 12.0,
            relative_alpha_drift_pct: 11.0,
            observed_at_utc: "2026-04-06T07:01:00Z".to_string(),
            stale_after_seconds: 120.0,
            correlation_id: "corr-rebalance-001".to_string(),
            require_execution: false,
            approval_reference: None,
        }
    }

    #[test]
    fn policy_upsert_rejects_unauthorized_role() {
        let service = AllocationPolicyService::default();
        let mut input = policy_input();
        input.actor_role = "read_only_analytics".to_string();

        let error = service
            .upsert_allocation_policy(input)
            .expect_err("unauthorized roles should fail closed");
        assert_eq!(error.code, "allocation_policy_unauthorized_role");
    }

    #[test]
    fn critical_policy_increase_without_approval_returns_pending() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");

        let mut increased = policy_input();
        increased.version = 2;
        increased.target_exposure_pct_nav = 45.0;
        increased.updated_at_utc = "2026-04-06T07:02:00Z".to_string();

        let evidence = service
            .upsert_allocation_policy(increased)
            .expect("critical increase should transition to pending");
        assert_eq!(evidence.approval_status, "pending");
        assert_eq!(
            evidence.reason_code,
            RebalanceReasonCode::AllocationPolicyPendingApproval.code()
        );
    }

    #[test]
    fn drift_equal_threshold_returns_in_bounds_deterministically() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");

        let mut input = evaluate_input();
        input.exposure_drift_pct = 10.0;
        input.relative_alpha_drift_pct = 15.0;
        let evidence = service
            .evaluate_rebalance_drift(input)
            .expect("equal threshold should remain in-bounds");
        assert_eq!(evidence.status, "denied");
        assert_eq!(evidence.reason_code, RebalanceReasonCode::InBounds.code());
    }

    #[test]
    fn drift_exceedance_returns_proposed_recommendation() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");

        let evidence = service
            .evaluate_rebalance_drift(evaluate_input())
            .expect("drift exceedance should produce recommendation evidence");
        assert_eq!(evidence.status, "proposed");
        assert_eq!(evidence.approval_status, "not_required");
        assert_eq!(
            evidence.reason_code,
            RebalanceReasonCode::RecommendationProposed.code()
        );
        assert!(evidence.rationale.contains("Drift exceeded threshold"));
    }

    #[test]
    fn drift_exceedance_for_execution_path_requires_approval_context() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");

        let mut input = evaluate_input();
        input.require_execution = true;
        let evidence = service
            .evaluate_rebalance_drift(input)
            .expect("execution-path recommendation should be pending without approval");
        assert_eq!(evidence.status, "pending_approval");
        assert_eq!(evidence.approval_status, "pending");
        assert_eq!(
            evidence.reason_code,
            RebalanceReasonCode::ApprovalRequired.code()
        );
    }

    #[test]
    fn execute_pending_recommendation_requires_approval_reference() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");
        let mut evaluate = evaluate_input();
        evaluate.require_execution = true;
        let recommendation = service
            .evaluate_rebalance_drift(evaluate)
            .expect("pending recommendation should be created");

        let error = service
            .execute_rebalance_recommendation(ExecuteRebalanceRecommendationInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                recommendation_id: recommendation.recommendation_id,
                correlation_id: "corr-exec-001".to_string(),
                executed_at_utc: "2026-04-06T07:02:00Z".to_string(),
                approval_reference: None,
            })
            .expect_err("pending recommendation execution should fail without approval");
        assert_eq!(error.code, RebalanceReasonCode::ApprovalRequired.code());
    }

    #[test]
    fn execute_pending_recommendation_succeeds_with_approval_reference() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");
        let mut evaluate = evaluate_input();
        evaluate.require_execution = true;
        let recommendation = service
            .evaluate_rebalance_drift(evaluate)
            .expect("pending recommendation should be created");

        let executed = service
            .execute_rebalance_recommendation(ExecuteRebalanceRecommendationInput {
                actor_id: "ops-2".to_string(),
                actor_role: "administrative_actions".to_string(),
                recommendation_id: recommendation.recommendation_id,
                correlation_id: "corr-exec-002".to_string(),
                executed_at_utc: "2026-04-06T07:03:00Z".to_string(),
                approval_reference: Some("apr-rebalance-001".to_string()),
            })
            .expect("execution with approval reference should succeed");
        assert_eq!(executed.status, "executed");
        assert_eq!(executed.approval_status, "approved");
        assert_eq!(
            executed.reason_code,
            RebalanceReasonCode::RecommendationExecuted.code()
        );
        assert_eq!(
            executed.approval_reference.as_deref(),
            Some("apr-rebalance-001")
        );
    }

    #[test]
    fn pending_query_returns_pending_recommendations_only() {
        let service = AllocationPolicyService::default();
        service
            .upsert_allocation_policy(policy_input())
            .expect("baseline policy should be applied");
        let mut evaluate = evaluate_input();
        evaluate.require_execution = true;
        service
            .evaluate_rebalance_drift(evaluate)
            .expect("pending recommendation should be created");

        let pending = service
            .list_pending_rebalance_recommendations(PendingRebalanceRecommendationsInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-pending-query-001".to_string(),
                queried_at_utc: "2026-04-06T07:04:00Z".to_string(),
                policy_key: Some("portfolio-default".to_string()),
            })
            .expect("pending query should succeed");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].status, "pending_approval");
    }
}
