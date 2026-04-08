use domain::research::{
    AlphaLifecycleActionRecord, AlphaLifecycleActionStatus, AlphaLifecycleActionType,
    AlphaLifecycleContractError, AlphaLifecycleDeallocationPolicy, AlphaLifecycleReasonCode,
    AlphaLifecycleValidationIssue, PromotionDecisionRecord, PromotionDecisionState,
    PromotionLifecycleAction, build_stop_research_trigger_evidence,
    canonicalize_alpha_lifecycle_action_record, compose_alpha_lifecycle_action_id_with_context,
    evaluate_fr47_deallocation_trigger, evaluate_fr48_stop_research_criteria,
    normalize_research_identifier, parse_alpha_lifecycle_utc_timestamp, stop_research_triggered,
};
use persistence::postgres::alpha_health_metrics::{
    AlphaHealthPersistenceError,
    list_alpha_threshold_breaches_by_alpha as pg_list_alpha_threshold_breaches,
};
use persistence::postgres::alpha_lifecycle_actions::{
    AlphaLifecyclePersistenceError,
    list_alpha_lifecycle_actions_by_alpha as pg_list_alpha_lifecycle_actions_by_alpha,
    load_alpha_lifecycle_action as pg_load_alpha_lifecycle_action,
    upsert_alpha_lifecycle_action as pg_upsert_alpha_lifecycle_action,
};
use persistence::postgres::promotion_decisions::{
    PromotionDecisionPersistenceError,
    list_promotion_decisions_by_candidate as pg_list_promotion_decisions_by_candidate,
};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

const DEFAULT_LIST_LIMIT: i64 = 25;
const MAX_LIST_LIMIT: i64 = 200;
const DEFAULT_HISTORY_LIMIT: i64 = 50;
const PROMOTION_FAILURE_RATE_WINDOW: usize = 10;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AlphaLifecycleActionServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<AlphaLifecycleValidationIssue>,
}

impl AlphaLifecycleActionServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlphaLifecycleValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: AlphaLifecycleReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for alpha lifecycle mutations".to_string(),
            field_errors: vec![AlphaLifecycleValidationIssue {
                field: "remediation_guidance".to_string(),
                code: AlphaLifecycleReasonCode::UnauthorizedRole.code(),
                message: "use an operational_control or administrative_actions role and retry with an approved request context"
                    .to_string(),
            }],
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: AlphaLifecycleReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for alpha lifecycle reads".to_string(),
            field_errors: vec![AlphaLifecycleValidationIssue {
                field: "remediation_guidance".to_string(),
                code: AlphaLifecycleReasonCode::UnauthorizedRole.code(),
                message:
                    "use a read_only_analytics, operational_control, or administrative_actions role"
                        .to_string(),
            }],
        }
    }

    fn action_not_found(action_id: &str) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::ActionNotFound.code(),
            message: format!("alpha lifecycle action `{action_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: vec![AlphaLifecycleValidationIssue {
                field: "remediation_guidance".to_string(),
                code: AlphaLifecycleReasonCode::DependencyUnavailable.code(),
                message:
                    "restore dependency availability and retry once source systems report healthy"
                        .to_string(),
            }],
        }
    }

    fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: vec![AlphaLifecycleValidationIssue {
                field: "remediation_guidance".to_string(),
                code: AlphaLifecycleReasonCode::StateUnavailable.code(),
                message: "refresh required lifecycle state inputs and retry with complete evidence"
                    .to_string(),
            }],
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: vec![AlphaLifecycleValidationIssue {
                field: "remediation_guidance".to_string(),
                code: AlphaLifecycleReasonCode::PersistenceUnavailable.code(),
                message: "restore persistence connectivity and retry after storage recovery"
                    .to_string(),
            }],
        }
    }
}

impl Display for AlphaLifecycleActionServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AlphaLifecycleActionServiceError {}

#[derive(Debug, Clone)]
pub struct StartAlphaLifecycleActionInput {
    pub actor_id: String,
    pub actor_role: String,
    pub alpha_id: String,
    pub action_type: String,
    pub deallocation_policies: Vec<AlphaLifecycleDeallocationPolicy>,
    pub trade_count_30d: Option<i64>,
    pub out_of_sample_sharpe_30d: Option<f64>,
    pub promotion_failure_rate_last_10: Option<f64>,
    pub approval_request_id: Option<String>,
    pub approval_reference: Option<String>,
    pub history_limit: Option<i64>,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadAlphaLifecycleActionInput {
    pub actor_id: String,
    pub actor_role: String,
    pub action_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListAlphaLifecycleActionsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub alpha_id: String,
    pub limit: Option<i64>,
    pub acted_after_utc: Option<String>,
    pub acted_before_utc: Option<String>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlphaLifecycleActionEvidence {
    pub action: AlphaLifecycleActionRecord,
    pub reason_code: String,
}

pub trait AlphaLifecycleActionOrchestrator: Send + Sync {
    fn start_alpha_lifecycle_action(
        &self,
        input: StartAlphaLifecycleActionInput,
    ) -> Result<AlphaLifecycleActionEvidence, AlphaLifecycleActionServiceError>;

    fn read_alpha_lifecycle_action(
        &self,
        input: ReadAlphaLifecycleActionInput,
    ) -> Result<AlphaLifecycleActionEvidence, AlphaLifecycleActionServiceError>;

    fn list_alpha_lifecycle_actions(
        &self,
        input: ListAlphaLifecycleActionsInput,
    ) -> Result<Vec<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError>;
}

pub trait AlphaLifecycleActionRepositoryPort: Send + Sync {
    fn upsert(
        &self,
        record: AlphaLifecycleActionRecord,
    ) -> Result<(), AlphaLifecycleActionServiceError>;
    fn load(
        &self,
        action_id: &str,
    ) -> Result<Option<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError>;
    fn list_by_alpha(
        &self,
        alpha_id: &str,
        acted_after_utc: Option<&str>,
        acted_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError>;
}

pub trait AlphaBreachEvidencePort: Send + Sync {
    fn list_recent_breaches(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::research::AlphaThresholdBreachRecord>, AlphaLifecycleActionServiceError>;
}

pub trait PromotionHistoryPort: Send + Sync {
    fn list_recent_promotions(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::research::PromotionDecisionRecord>, AlphaLifecycleActionServiceError>;
}

#[derive(Clone)]
pub struct AlphaLifecycleActionService {
    repository: Arc<dyn AlphaLifecycleActionRepositoryPort>,
    breach_evidence: Arc<dyn AlphaBreachEvidencePort>,
    promotion_history: Arc<dyn PromotionHistoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl AlphaLifecycleActionService {
    pub fn new(
        repository: Arc<dyn AlphaLifecycleActionRepositoryPort>,
        breach_evidence: Arc<dyn AlphaBreachEvidencePort>,
        promotion_history: Arc<dyn PromotionHistoryPort>,
    ) -> Self {
        Self {
            repository,
            breach_evidence,
            promotion_history,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryAlphaLifecycleActionRepository::default()),
            Arc::new(InMemoryAlphaBreachEvidencePort::default()),
            Arc::new(InMemoryPromotionHistoryPort::default()),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresAlphaLifecycleActionRepository::new(pool.clone())),
            Arc::new(PostgresAlphaBreachEvidencePort::new(pool.clone())),
            Arc::new(PostgresPromotionHistoryPort::new(pool)),
        )
    }

    pub fn with_breach_evidence_port(
        mut self,
        breach_evidence: Arc<dyn AlphaBreachEvidencePort>,
    ) -> Self {
        self.breach_evidence = breach_evidence;
        self
    }

    pub fn with_promotion_history_port(
        mut self,
        promotion_history: Arc<dyn PromotionHistoryPort>,
    ) -> Self {
        self.promotion_history = promotion_history;
        self
    }

    pub fn with_repository(
        mut self,
        repository: Arc<dyn AlphaLifecycleActionRepositoryPort>,
    ) -> Self {
        self.repository = repository;
        self
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, AlphaLifecycleActionServiceError> {
        self.operation_lock.lock().map_err(|_| {
            AlphaLifecycleActionServiceError::persistence_unavailable(
                "alpha lifecycle operation lock is poisoned",
            )
        })
    }
}

impl Default for AlphaLifecycleActionService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl AlphaLifecycleActionOrchestrator for AlphaLifecycleActionService {
    fn start_alpha_lifecycle_action(
        &self,
        input: StartAlphaLifecycleActionInput,
    ) -> Result<AlphaLifecycleActionEvidence, AlphaLifecycleActionServiceError> {
        validate_mutation_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("alpha_id", &input.alpha_id)?;
        let normalized_correlation_id =
            normalize_required_identifier("correlation_id", &input.correlation_id)?;
        validate_utc_timestamp("requested_at_utc", &input.requested_at_utc)?;
        let history_limit =
            resolve_bounded_limit("history_limit", input.history_limit, DEFAULT_HISTORY_LIMIT)?;

        let normalized_alpha_id = normalize_research_identifier(&input.alpha_id);
        let action_type =
            AlphaLifecycleActionType::parse(&input.action_type).map_err(map_contract_error)?;
        let action_id = compose_alpha_lifecycle_action_id_with_context(
            &normalized_alpha_id,
            action_type.as_str(),
            &normalized_correlation_id,
            &input.requested_at_utc,
        )
        .map_err(map_contract_error)?;
        let approval_request_id = input
            .approval_request_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_research_identifier);
        let approval_reference = input
            .approval_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if approval_reference.is_some() && approval_request_id.is_none() {
            return Err(AlphaLifecycleActionServiceError::invalid_payload(
                "approval_request_id is required when approval_reference is provided",
                vec![AlphaLifecycleValidationIssue {
                    field: "approval_request_id".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: "approval_request_id is required when approval_reference is provided"
                        .to_string(),
                }],
            ));
        }

        let _lock = self.lock_operations()?;

        let evaluated = match action_type {
            AlphaLifecycleActionType::Deallocate => {
                let mut field_errors = Vec::new();
                if input.trade_count_30d.is_some() {
                    field_errors.push(AlphaLifecycleValidationIssue {
                        field: "trade_count_30d".to_string(),
                        code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                        message: "trade_count_30d is only allowed for stop_research actions"
                            .to_string(),
                    });
                }
                if input.out_of_sample_sharpe_30d.is_some() {
                    field_errors.push(AlphaLifecycleValidationIssue {
                        field: "out_of_sample_sharpe_30d".to_string(),
                        code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                        message:
                            "out_of_sample_sharpe_30d is only allowed for stop_research actions"
                                .to_string(),
                    });
                }
                if input.promotion_failure_rate_last_10.is_some() {
                    field_errors.push(AlphaLifecycleValidationIssue {
                        field: "promotion_failure_rate_last_10".to_string(),
                        code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                        message:
                            "promotion_failure_rate_last_10 is only allowed for stop_research actions"
                                .to_string(),
                    });
                }
                if !field_errors.is_empty() {
                    return Err(AlphaLifecycleActionServiceError::invalid_payload(
                        "deallocate actions do not accept stop-research criteria fields",
                        field_errors,
                    ));
                }
                let breaches = self
                    .breach_evidence
                    .list_recent_breaches(&normalized_alpha_id, history_limit)?;
                let evaluation = evaluate_fr47_deallocation_trigger(
                    &breaches,
                    input.deallocation_policies.as_slice(),
                )
                .map_err(map_contract_error)?;
                if evaluation.triggered {
                    let breach = evaluation
                        .trigger_breach
                        .clone()
                        .expect("triggered deallocation evaluations include a breach");
                    (
                        AlphaLifecycleActionStatus::Applied,
                        AlphaLifecycleReasonCode::DeallocationThresholdBreached
                            .code()
                            .to_string(),
                        json!({
                            "criterion_keys": evaluation.triggered_criteria,
                            "breach_id": breach.breach_id,
                            "metric_key": breach.metric_key.as_str(),
                            "comparator": breach.comparator.as_str(),
                            "observed_value": breach.observed_value,
                            "threshold_value": breach.threshold_value,
                            "breached_at_utc": breach.breached_at_utc,
                            "reflected_lifecycle_state": "deallocated",
                            "reflection_source": "alpha_lifecycle_actions",
                            "reflection_mode": "canonical_lifecycle_contract",
                        }),
                        None,
                        None,
                    )
                } else {
                    (
                        AlphaLifecycleActionStatus::Denied,
                        AlphaLifecycleReasonCode::ActionDenied.code().to_string(),
                        json!({
                            "criterion_keys": [],
                            "source": "alpha_threshold_breaches",
                            "evaluated": true,
                        }),
                        None,
                        Some(
                            "deallocation criteria were not met; review threshold policy values and monitor additional breach evidence"
                                .to_string(),
                        ),
                    )
                }
            }
            AlphaLifecycleActionType::StopResearch => {
                if !input.deallocation_policies.is_empty() {
                    return Err(AlphaLifecycleActionServiceError::invalid_payload(
                        "deallocation_policies must be empty for stop_research actions",
                        vec![AlphaLifecycleValidationIssue {
                            field: "deallocation_policies".to_string(),
                            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                            message:
                                "deallocation_policies must be empty for stop_research actions"
                                    .to_string(),
                        }],
                    ));
                }
                let trade_count_30d = input.trade_count_30d.ok_or_else(|| {
                    AlphaLifecycleActionServiceError::invalid_payload(
                        "trade_count_30d is required for stop_research evaluation",
                        vec![AlphaLifecycleValidationIssue {
                            field: "trade_count_30d".to_string(),
                            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                            message: "trade_count_30d is required for stop_research evaluation"
                                .to_string(),
                        }],
                    )
                })?;
                let out_of_sample_sharpe_30d = input.out_of_sample_sharpe_30d.ok_or_else(|| {
                    AlphaLifecycleActionServiceError::invalid_payload(
                        "out_of_sample_sharpe_30d is required for stop_research evaluation",
                        vec![AlphaLifecycleValidationIssue {
                            field: "out_of_sample_sharpe_30d".to_string(),
                            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                            message:
                                "out_of_sample_sharpe_30d is required for stop_research evaluation"
                                    .to_string(),
                        }],
                    )
                })?;
                if input.promotion_failure_rate_last_10.is_some() {
                    return Err(AlphaLifecycleActionServiceError::invalid_payload(
                        "promotion_failure_rate_last_10 is derived from promotion history and must not be provided",
                        vec![AlphaLifecycleValidationIssue {
                            field: "promotion_failure_rate_last_10".to_string(),
                            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                            message:
                                "promotion_failure_rate_last_10 is derived from promotion history and must not be provided"
                                    .to_string(),
                        }],
                    ));
                }
                let promotion_failure_rate_last_10 = self
                    .derive_promotion_failure_rate_last_10(&normalized_alpha_id, history_limit)?;
                let snapshot = evaluate_fr48_stop_research_criteria(
                    trade_count_30d,
                    out_of_sample_sharpe_30d,
                    promotion_failure_rate_last_10,
                )
                .map_err(map_contract_error)?;
                let action_status = if stop_research_triggered(&snapshot) {
                    AlphaLifecycleActionStatus::Applied
                } else {
                    AlphaLifecycleActionStatus::Denied
                };
                let reason_code = if action_status == AlphaLifecycleActionStatus::Applied {
                    AlphaLifecycleReasonCode::StopResearchCriteriaMet
                        .code()
                        .to_string()
                } else {
                    AlphaLifecycleReasonCode::ActionDenied.code().to_string()
                };
                (
                    action_status,
                    reason_code,
                    build_stop_research_trigger_evidence(&snapshot),
                    Some(snapshot.clone()),
                    if action_status == AlphaLifecycleActionStatus::Applied {
                        None
                    } else {
                        Some(
                            "stop_research criteria were not met; continue monitoring live alpha quality and promotion outcomes"
                                .to_string(),
                        )
                    },
                )
            }
        };

        let record = AlphaLifecycleActionRecord {
            action_id,
            alpha_id: normalized_alpha_id.clone(),
            action_type,
            action_status: evaluated.0,
            reason_code: evaluated.1.clone(),
            trigger_evidence: evaluated.2,
            stop_research_criteria: evaluated.3.clone(),
            remediation_guidance: evaluated.4,
            actor_id: input.actor_id.clone(),
            correlation_id: normalized_correlation_id.clone(),
            acted_at_utc: input.requested_at_utc.clone(),
            approval_request_id,
            approval_reference: approval_reference.clone(),
        };
        self.repository.upsert(record.clone())?;

        emit_alpha_lifecycle_action_telemetry(
            "alpha_lifecycle_action_start_v1",
            "alpha_lifecycle_action_start",
            if record.action_status == AlphaLifecycleActionStatus::Applied {
                "applied"
            } else {
                "deny"
            },
            &record.actor_id,
            &record.alpha_id,
            Some(&record.action_id),
            &record.reason_code,
            &record.correlation_id,
            &record.acted_at_utc,
        );

        Ok(AlphaLifecycleActionEvidence {
            reason_code: record.reason_code.clone(),
            action: record,
        })
    }

    fn read_alpha_lifecycle_action(
        &self,
        input: ReadAlphaLifecycleActionInput,
    ) -> Result<AlphaLifecycleActionEvidence, AlphaLifecycleActionServiceError> {
        validate_read_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("action_id", &input.action_id)?;
        let normalized_correlation_id =
            normalize_required_identifier("correlation_id", &input.correlation_id)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        let normalized_action_id = normalize_research_identifier(&input.action_id);
        let Some(action) = self.repository.load(&normalized_action_id)? else {
            emit_alpha_lifecycle_action_telemetry(
                "alpha_lifecycle_action_read_v1",
                "alpha_lifecycle_action_read",
                "deny",
                &input.actor_id,
                "unknown",
                Some(&normalized_action_id),
                AlphaLifecycleReasonCode::ActionNotFound.code(),
                &normalized_correlation_id,
                &input.queried_at_utc,
            );
            return Err(AlphaLifecycleActionServiceError::action_not_found(
                &normalized_action_id,
            ));
        };

        emit_alpha_lifecycle_action_telemetry(
            "alpha_lifecycle_action_read_v1",
            "alpha_lifecycle_action_read",
            "allow",
            &input.actor_id,
            &action.alpha_id,
            Some(&action.action_id),
            AlphaLifecycleReasonCode::ActionRead.code(),
            &normalized_correlation_id,
            &input.queried_at_utc,
        );
        Ok(AlphaLifecycleActionEvidence {
            action,
            reason_code: AlphaLifecycleReasonCode::ActionRead.code().to_string(),
        })
    }

    fn list_alpha_lifecycle_actions(
        &self,
        input: ListAlphaLifecycleActionsInput,
    ) -> Result<Vec<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError> {
        validate_read_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("alpha_id", &input.alpha_id)?;
        let normalized_correlation_id =
            normalize_required_identifier("correlation_id", &input.correlation_id)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        let normalized_alpha_id = normalize_research_identifier(&input.alpha_id);
        let normalized_after =
            normalize_optional_timestamp("acted_after_utc", input.acted_after_utc.as_deref())?;
        let normalized_before =
            normalize_optional_timestamp("acted_before_utc", input.acted_before_utc.as_deref())?;

        if let (Some(acted_after), Some(acted_before)) =
            (normalized_after.as_deref(), normalized_before.as_deref())
        {
            let acted_after_ts =
                parse_alpha_lifecycle_utc_timestamp(acted_after).map_err(map_contract_error)?;
            let acted_before_ts =
                parse_alpha_lifecycle_utc_timestamp(acted_before).map_err(map_contract_error)?;
            if acted_before_ts <= acted_after_ts {
                return Err(AlphaLifecycleActionServiceError::invalid_payload(
                    "acted_before_utc must be greater than acted_after_utc",
                    vec![AlphaLifecycleValidationIssue {
                        field: "acted_before_utc".to_string(),
                        code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                        message: "acted_before_utc must be greater than acted_after_utc"
                            .to_string(),
                    }],
                ));
            }
        }

        let limit = resolve_bounded_limit("limit", input.limit, DEFAULT_LIST_LIMIT)?;
        let actions = self.repository.list_by_alpha(
            &normalized_alpha_id,
            normalized_after.as_deref(),
            normalized_before.as_deref(),
            limit,
        )?;
        emit_alpha_lifecycle_action_telemetry(
            "alpha_lifecycle_action_list_v1",
            "alpha_lifecycle_action_list",
            "allow",
            &input.actor_id,
            &normalized_alpha_id,
            None,
            AlphaLifecycleReasonCode::ActionListed.code(),
            &normalized_correlation_id,
            &input.queried_at_utc,
        );
        Ok(actions)
    }
}

impl AlphaLifecycleActionService {
    fn derive_promotion_failure_rate_last_10(
        &self,
        alpha_id: &str,
        history_limit: i64,
    ) -> Result<f64, AlphaLifecycleActionServiceError> {
        let required_window = PROMOTION_FAILURE_RATE_WINDOW as i64;
        let initial_fetch_limit = history_limit.max(required_window);
        let mut recent_promotions =
            self.load_recent_promotions_for_stop_research(alpha_id, initial_fetch_limit)?;
        if recent_promotions.len() < PROMOTION_FAILURE_RATE_WINDOW
            && initial_fetch_limit < MAX_LIST_LIMIT
        {
            recent_promotions =
                self.load_recent_promotions_for_stop_research(alpha_id, MAX_LIST_LIMIT)?;
        }
        if recent_promotions.len() < PROMOTION_FAILURE_RATE_WINDOW {
            return Err(AlphaLifecycleActionServiceError::state_unavailable(
                "promotion history must include at least 10 promote decisions for stop_research evaluation",
            ));
        }
        let denied = recent_promotions
            .iter()
            .take(PROMOTION_FAILURE_RATE_WINDOW)
            .filter(|decision| decision.decision_state == PromotionDecisionState::Denied)
            .count();
        Ok(denied as f64 / PROMOTION_FAILURE_RATE_WINDOW as f64)
    }

    fn load_recent_promotions_for_stop_research(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<PromotionDecisionRecord>, AlphaLifecycleActionServiceError> {
        let normalized_alpha_id = normalize_research_identifier(alpha_id);
        let primary_records =
            self.list_recent_promote_decisions_for_lookup_key(&normalized_alpha_id, limit)?;
        let secondary_records = alternate_promotion_lookup_key(&normalized_alpha_id)
            .map(|alternate_key| {
                self.list_recent_promote_decisions_for_lookup_key(&alternate_key, limit)
            })
            .transpose()?
            .unwrap_or_default();
        if !primary_records.is_empty() && !secondary_records.is_empty() {
            return Err(AlphaLifecycleActionServiceError::state_unavailable(
                "promotion history lookup is ambiguous across alpha_id and candidate_id keys; explicit canonical mapping is required",
            ));
        }
        let mut selected_records = if !primary_records.is_empty() {
            primary_records
        } else {
            secondary_records
        };
        selected_records.sort_by(|left, right| {
            right
                .decided_at_utc
                .cmp(&left.decided_at_utc)
                .then_with(|| left.decision_id.cmp(&right.decision_id))
        });
        Ok(selected_records)
    }

    fn list_recent_promote_decisions_for_lookup_key(
        &self,
        lookup_key: &str,
        limit: i64,
    ) -> Result<Vec<PromotionDecisionRecord>, AlphaLifecycleActionServiceError> {
        let mut decisions = self
            .promotion_history
            .list_recent_promotions(lookup_key, limit)?;
        decisions.retain(|decision| decision.lifecycle_action == PromotionLifecycleAction::Promote);
        Ok(decisions)
    }
}

fn map_contract_error(error: AlphaLifecycleContractError) -> AlphaLifecycleActionServiceError {
    AlphaLifecycleActionServiceError::invalid_payload(error.message, error.field_errors)
}

fn validate_mutation_role(role: &str) -> Result<(), AlphaLifecycleActionServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(AlphaLifecycleActionServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), AlphaLifecycleActionServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(AlphaLifecycleActionServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AlphaLifecycleActionServiceError> {
    if value.trim().is_empty() {
        return Err(AlphaLifecycleActionServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn normalize_required_identifier(
    field: &str,
    value: &str,
) -> Result<String, AlphaLifecycleActionServiceError> {
    let normalized = normalize_research_identifier(value);
    if normalized.is_empty() {
        return Err(AlphaLifecycleActionServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    if normalized.len() > 160 {
        return Err(AlphaLifecycleActionServiceError::invalid_payload(
            format!("{field} must be 160 characters or fewer"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} must be 160 characters or fewer"),
            }],
        ));
    }
    Ok(normalized)
}

fn validate_utc_timestamp(
    field: &str,
    value: &str,
) -> Result<(), AlphaLifecycleActionServiceError> {
    parse_alpha_lifecycle_utc_timestamp(value).map_err(|_| {
        AlphaLifecycleActionServiceError::invalid_payload(
            format!("{field} must be RFC3339 UTC"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} must be RFC3339 UTC"),
            }],
        )
    })?;
    Ok(())
}

fn resolve_bounded_limit(
    field: &str,
    value: Option<i64>,
    default_value: i64,
) -> Result<i64, AlphaLifecycleActionServiceError> {
    let resolved = value.unwrap_or(default_value);
    if resolved <= 0 || resolved > MAX_LIST_LIMIT {
        return Err(AlphaLifecycleActionServiceError::invalid_payload(
            format!("{field} must be between 1 and {MAX_LIST_LIMIT}"),
            vec![AlphaLifecycleValidationIssue {
                field: field.to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: format!("{field} must be between 1 and {MAX_LIST_LIMIT}"),
            }],
        ));
    }
    Ok(resolved)
}

fn alternate_promotion_lookup_key(alpha_id: &str) -> Option<String> {
    if let Some(suffix) = alpha_id.strip_prefix("alpha::") {
        return (!suffix.is_empty()).then(|| format!("candidate::{suffix}"));
    }
    if let Some(suffix) = alpha_id.strip_prefix("candidate::") {
        return (!suffix.is_empty()).then(|| format!("alpha::{suffix}"));
    }
    None
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, AlphaLifecycleActionServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    validate_utc_timestamp(field, trimmed)?;
    Ok(Some(trimmed.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn emit_alpha_lifecycle_action_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    alpha_id: &str,
    action_id: Option<&str>,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = AlphaLifecycleTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        alpha_id,
        action_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if should_emit_fail_closed_security_signal(outcome, reason_code) {
            Some(AlphaLifecycleSecuritySignal {
                name: "alpha_lifecycle_action_fail_closed_v1",
                severity: "critical",
                alert_compatible: true,
                alert_target_seconds: 30,
            })
        } else {
            None
        },
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("alpha lifecycle telemetry should serialize")
    );
}

fn should_emit_fail_closed_security_signal(outcome: &str, reason_code: &str) -> bool {
    outcome == "fail_closed"
        || reason_code == AlphaLifecycleReasonCode::AuthorizationFailed.code()
        || reason_code == AlphaLifecycleReasonCode::DependencyUnavailable.code()
        || reason_code == AlphaLifecycleReasonCode::StateUnavailable.code()
        || reason_code == AlphaLifecycleReasonCode::PersistenceUnavailable.code()
}

#[derive(Debug, Serialize)]
struct AlphaLifecycleTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    alpha_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_id: Option<&'a str>,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<AlphaLifecycleSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct AlphaLifecycleSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresAlphaLifecycleActionRepository {
    pool: PgPool,
}

impl PostgresAlphaLifecycleActionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, AlphaLifecycleActionServiceError>
    where
        F: Future<Output = Result<T, AlphaLifecyclePersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_alpha_lifecycle_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaLifecycleActionServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_alpha_lifecycle_persistence_error),
        }
    }
}

impl AlphaLifecycleActionRepositoryPort for PostgresAlphaLifecycleActionRepository {
    fn upsert(
        &self,
        record: AlphaLifecycleActionRecord,
    ) -> Result<(), AlphaLifecycleActionServiceError> {
        self.run_future(pg_upsert_alpha_lifecycle_action(&self.pool, &record))
    }

    fn load(
        &self,
        action_id: &str,
    ) -> Result<Option<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError> {
        self.run_future(pg_load_alpha_lifecycle_action(&self.pool, action_id))
    }

    fn list_by_alpha(
        &self,
        alpha_id: &str,
        acted_after_utc: Option<&str>,
        acted_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError> {
        self.run_future(pg_list_alpha_lifecycle_actions_by_alpha(
            &self.pool,
            alpha_id,
            acted_after_utc,
            acted_before_utc,
            limit,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct PostgresAlphaBreachEvidencePort {
    pool: PgPool,
}

impl PostgresAlphaBreachEvidencePort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, AlphaLifecycleActionServiceError>
    where
        F: Future<Output = Result<T, AlphaHealthPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_alpha_health_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaLifecycleActionServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_alpha_health_persistence_error),
        }
    }
}

impl AlphaBreachEvidencePort for PostgresAlphaBreachEvidencePort {
    fn list_recent_breaches(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::research::AlphaThresholdBreachRecord>, AlphaLifecycleActionServiceError>
    {
        self.run_future(pg_list_alpha_threshold_breaches(
            &self.pool, alpha_id, None, None, limit,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct PostgresPromotionHistoryPort {
    pool: PgPool,
}

impl PostgresPromotionHistoryPort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, AlphaLifecycleActionServiceError>
    where
        F: Future<Output = Result<T, PromotionDecisionPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_promotion_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaLifecycleActionServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_promotion_persistence_error),
        }
    }
}

impl PromotionHistoryPort for PostgresPromotionHistoryPort {
    fn list_recent_promotions(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::research::PromotionDecisionRecord>, AlphaLifecycleActionServiceError>
    {
        self.run_future(pg_list_promotion_decisions_by_candidate(
            &self.pool, alpha_id, None, None, limit,
        ))
    }
}

fn map_alpha_lifecycle_persistence_error(
    error: AlphaLifecyclePersistenceError,
) -> AlphaLifecycleActionServiceError {
    match error.code {
        "alpha_lifecycle_action_query_failed"
        | "alpha_lifecycle_action_row_decode_failed"
        | "alpha_lifecycle_action_constraint_violation" => {
            AlphaLifecycleActionServiceError::persistence_unavailable(error.message)
        }
        _ => AlphaLifecycleActionServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

fn map_alpha_health_persistence_error(
    error: AlphaHealthPersistenceError,
) -> AlphaLifecycleActionServiceError {
    match error.code {
        "alpha_health_query_failed" => {
            AlphaLifecycleActionServiceError::dependency_unavailable(error.message)
        }
        "alpha_health_row_decode_failed" => {
            AlphaLifecycleActionServiceError::state_unavailable(error.message)
        }
        _ => AlphaLifecycleActionServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| AlphaLifecycleValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn map_promotion_persistence_error(
    error: PromotionDecisionPersistenceError,
) -> AlphaLifecycleActionServiceError {
    match error.code {
        "promotion_decision_query_failed" => {
            AlphaLifecycleActionServiceError::dependency_unavailable(error.message)
        }
        "promotion_decision_row_decode_failed" => {
            AlphaLifecycleActionServiceError::state_unavailable(error.message)
        }
        _ => AlphaLifecycleActionServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| AlphaLifecycleValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

#[derive(Default)]
pub struct InMemoryAlphaLifecycleActionRepository {
    actions: Mutex<std::collections::BTreeMap<String, AlphaLifecycleActionRecord>>,
}

impl AlphaLifecycleActionRepositoryPort for InMemoryAlphaLifecycleActionRepository {
    fn upsert(
        &self,
        record: AlphaLifecycleActionRecord,
    ) -> Result<(), AlphaLifecycleActionServiceError> {
        let canonical =
            canonicalize_alpha_lifecycle_action_record(&record).map_err(map_contract_error)?;
        let mut actions = self.actions.lock().map_err(|_| {
            AlphaLifecycleActionServiceError::persistence_unavailable(
                "in-memory alpha lifecycle action store lock poisoned",
            )
        })?;
        actions.insert(canonical.action_id.clone(), canonical);
        Ok(())
    }

    fn load(
        &self,
        action_id: &str,
    ) -> Result<Option<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError> {
        let normalized_action_id = normalize_research_identifier(action_id);
        let actions = self.actions.lock().map_err(|_| {
            AlphaLifecycleActionServiceError::persistence_unavailable(
                "in-memory alpha lifecycle action store lock poisoned",
            )
        })?;
        actions
            .get(&normalized_action_id)
            .cloned()
            .map(|record| {
                canonicalize_alpha_lifecycle_action_record(&record).map_err(map_contract_error)
            })
            .transpose()
    }

    fn list_by_alpha(
        &self,
        alpha_id: &str,
        acted_after_utc: Option<&str>,
        acted_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaLifecycleActionRecord>, AlphaLifecycleActionServiceError> {
        if limit <= 0 {
            return Err(AlphaLifecycleActionServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaLifecycleValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_alpha_id = normalize_research_identifier(alpha_id);
        let acted_after = acted_after_utc
            .map(parse_alpha_lifecycle_utc_timestamp)
            .transpose()
            .map_err(map_contract_error)?;
        let acted_before = acted_before_utc
            .map(parse_alpha_lifecycle_utc_timestamp)
            .transpose()
            .map_err(map_contract_error)?;
        let actions = self.actions.lock().map_err(|_| {
            AlphaLifecycleActionServiceError::persistence_unavailable(
                "in-memory alpha lifecycle action store lock poisoned",
            )
        })?;
        let mut listed = actions
            .values()
            .filter(|record| record.alpha_id == normalized_alpha_id)
            .filter(|record| {
                let acted_at = parse_alpha_lifecycle_utc_timestamp(&record.acted_at_utc).ok();
                if acted_after.is_some() && acted_at.is_none() {
                    return false;
                }
                if acted_before.is_some() && acted_at.is_none() {
                    return false;
                }
                let Some(acted_at) = acted_at else {
                    return true;
                };
                if let Some(acted_after) = acted_after
                    && acted_at < acted_after
                {
                    return false;
                }
                if let Some(acted_before) = acted_before
                    && acted_at >= acted_before
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect::<Vec<_>>();
        listed.sort_by(|left, right| {
            right
                .acted_at_utc
                .cmp(&left.acted_at_utc)
                .then_with(|| left.action_id.cmp(&right.action_id))
        });
        listed
            .into_iter()
            .take(limit as usize)
            .map(|record| {
                canonicalize_alpha_lifecycle_action_record(&record).map_err(map_contract_error)
            })
            .collect()
    }
}

#[derive(Default)]
pub struct InMemoryAlphaBreachEvidencePort {
    breaches: Mutex<Vec<domain::research::AlphaThresholdBreachRecord>>,
}

impl InMemoryAlphaBreachEvidencePort {
    pub fn with_breaches(breaches: Vec<domain::research::AlphaThresholdBreachRecord>) -> Self {
        Self {
            breaches: Mutex::new(breaches),
        }
    }
}

impl AlphaBreachEvidencePort for InMemoryAlphaBreachEvidencePort {
    fn list_recent_breaches(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::research::AlphaThresholdBreachRecord>, AlphaLifecycleActionServiceError>
    {
        if limit <= 0 {
            return Err(AlphaLifecycleActionServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaLifecycleValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_alpha_id = normalize_research_identifier(alpha_id);
        let breaches = self.breaches.lock().map_err(|_| {
            AlphaLifecycleActionServiceError::dependency_unavailable(
                "in-memory alpha breach store lock poisoned",
            )
        })?;
        let mut records = breaches
            .iter()
            .filter(|breach| breach.alpha_id == normalized_alpha_id)
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .breached_at_utc
                .cmp(&left.breached_at_utc)
                .then_with(|| left.breach_id.cmp(&right.breach_id))
        });
        Ok(records.into_iter().take(limit as usize).collect())
    }
}

#[derive(Default)]
pub struct InMemoryPromotionHistoryPort {
    decisions: Mutex<Vec<domain::research::PromotionDecisionRecord>>,
}

impl InMemoryPromotionHistoryPort {
    pub fn with_decisions(decisions: Vec<domain::research::PromotionDecisionRecord>) -> Self {
        Self {
            decisions: Mutex::new(decisions),
        }
    }
}

impl PromotionHistoryPort for InMemoryPromotionHistoryPort {
    fn list_recent_promotions(
        &self,
        alpha_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::research::PromotionDecisionRecord>, AlphaLifecycleActionServiceError>
    {
        if limit <= 0 {
            return Err(AlphaLifecycleActionServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaLifecycleValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_alpha_id = normalize_research_identifier(alpha_id);
        let decisions = self.decisions.lock().map_err(|_| {
            AlphaLifecycleActionServiceError::dependency_unavailable(
                "in-memory promotion history store lock poisoned",
            )
        })?;
        let mut records = decisions
            .iter()
            .filter(|decision| decision.candidate_id == normalized_alpha_id)
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .decided_at_utc
                .cmp(&left.decided_at_utc)
                .then_with(|| left.decision_id.cmp(&right.decision_id))
        });
        Ok(records.into_iter().take(limit as usize).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::research::{
        AlphaHealthMetricKey, AlphaThresholdBreachRecord, PromotionDecisionReasonCode,
        PromotionThresholdOutcome, ValidationGateComparator,
    };
    use serde_json::json;

    fn sample_breach() -> AlphaThresholdBreachRecord {
        AlphaThresholdBreachRecord {
            breach_id: "alpha::mean-reversion::rolling_drawdown::1712534400000000000".to_string(),
            metric_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            metric_key: AlphaHealthMetricKey::RollingDrawdown,
            comparator: ValidationGateComparator::Gt,
            observed_value: 0.11,
            threshold_value: 0.1,
            breach_reason: "rolling_drawdown exceeded configured ceiling".to_string(),
            reason_code: "alpha_health_threshold_breach_detected".to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-lifecycle-001".to_string(),
            breached_at_utc: "2026-04-08T01:00:00Z".to_string(),
        }
    }

    fn sample_promotion_decision_for_candidate(
        decision_id: &str,
        state: PromotionDecisionState,
        decided_at_utc: &str,
        candidate_id: &str,
    ) -> domain::research::PromotionDecisionRecord {
        domain::research::PromotionDecisionRecord {
            decision_id: decision_id.to_string(),
            candidate_id: candidate_id.to_string(),
            validation_run_id: format!("{candidate_id}::1712447000"),
            lifecycle_action: PromotionLifecycleAction::Promote,
            decision_state: state,
            reason_code: PromotionDecisionReasonCode::DecisionDenied
                .code()
                .to_string(),
            observed_metrics: json!({
                "out_of_sample_sharpe": 0.4
            }),
            evidence_packet: json!({
                "data_quality_report": {"artifact_id": "quality::001"},
                "purged_cpcv_results": {"artifact_id": "cpcv::001"},
                "calibration_report": {"artifact_id": "calibration::001"},
                "counterfactual_replay_summary": {"run_id": "replay::001"},
            }),
            threshold_results: vec![PromotionThresholdOutcome {
                metric_key: "out_of_sample_sharpe".to_string(),
                comparator: ValidationGateComparator::Gt,
                threshold_value: 0.3,
                observed_value: 0.4,
                passed: true,
                reason_code: "promotion_threshold_passed".to_string(),
            }],
            missing_evidence_fields: Vec::new(),
            gate_evaluation: json!({
                "outcome": "allow"
            }),
            shadow_readiness: Some(json!({"reason_code": "shadow_ready"})),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-lifecycle-001".to_string(),
            decided_at_utc: decided_at_utc.to_string(),
            approval_request_id: None,
            approval_reference: None,
        }
    }

    fn sample_promotion_decision(
        decision_id: &str,
        state: PromotionDecisionState,
        decided_at_utc: &str,
    ) -> domain::research::PromotionDecisionRecord {
        sample_promotion_decision_for_candidate(
            decision_id,
            state,
            decided_at_utc,
            "alpha::mean-reversion",
        )
    }

    fn promotion_history_boundary_window() -> Vec<domain::research::PromotionDecisionRecord> {
        let mut decisions = Vec::new();
        for index in 0..7 {
            decisions.push(sample_promotion_decision(
                &format!("decision-denied-boundary-{index}"),
                PromotionDecisionState::Denied,
                &format!("2026-04-08T01:{index:02}:00Z"),
            ));
        }
        for index in 0..3 {
            decisions.push(sample_promotion_decision(
                &format!("decision-allowed-boundary-{index}"),
                PromotionDecisionState::Allowed,
                &format!("2026-04-08T00:{index:02}:00Z"),
            ));
        }
        decisions
    }

    fn service_with_boundary_promotion_history() -> AlphaLifecycleActionService {
        AlphaLifecycleActionService::in_memory().with_promotion_history_port(Arc::new(
            InMemoryPromotionHistoryPort::with_decisions(promotion_history_boundary_window()),
        ))
    }

    #[derive(Default)]
    struct FailingAlphaBreachEvidencePort;

    impl AlphaBreachEvidencePort for FailingAlphaBreachEvidencePort {
        fn list_recent_breaches(
            &self,
            _alpha_id: &str,
            _limit: i64,
        ) -> Result<
            Vec<domain::research::AlphaThresholdBreachRecord>,
            AlphaLifecycleActionServiceError,
        > {
            Err(AlphaLifecycleActionServiceError::dependency_unavailable(
                "alpha breach evidence dependency unavailable",
            ))
        }
    }

    #[test]
    fn alpha_lifecycle_action_start_deallocation_applies_when_breach_threshold_triggers() {
        let service = AlphaLifecycleActionService::in_memory().with_breach_evidence_port(Arc::new(
            InMemoryAlphaBreachEvidencePort::with_breaches(vec![sample_breach()]),
        ));
        let evidence = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "deallocate".to_string(),
                deallocation_policies: vec![AlphaLifecycleDeallocationPolicy {
                    metric_key: AlphaHealthMetricKey::RollingDrawdown,
                    comparator: ValidationGateComparator::Gt,
                    threshold_value: 0.1,
                }],
                trade_count_30d: None,
                out_of_sample_sharpe_30d: None,
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-start-001".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect("deallocation start should succeed");
        assert_eq!(
            evidence.action.action_status,
            AlphaLifecycleActionStatus::Applied
        );
        assert_eq!(
            evidence.action.reason_code,
            AlphaLifecycleReasonCode::DeallocationThresholdBreached.code()
        );
        assert_eq!(
            evidence.action.trigger_evidence["criterion_keys"][0],
            "deallocation_threshold:rolling_drawdown"
        );
        assert_eq!(
            evidence.action.trigger_evidence["reflected_lifecycle_state"],
            "deallocated"
        );
        assert_eq!(
            evidence.action.trigger_evidence["reflection_mode"],
            "canonical_lifecycle_contract"
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_stop_research_boundary_values_stay_on_deny_path() {
        let service = service_with_boundary_promotion_history();
        let evidence = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(200),
                out_of_sample_sharpe_30d: Some(0.2),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-start-002".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect("stop_research boundary evaluation should succeed");
        assert_eq!(
            evidence.action.action_status,
            AlphaLifecycleActionStatus::Denied
        );
        assert_eq!(
            evidence.action.reason_code,
            AlphaLifecycleReasonCode::ActionDenied.code()
        );
        assert_eq!(
            evidence.action.trigger_evidence["criterion_keys"],
            json!([])
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_rejects_caller_supplied_failure_rate() {
        let service = service_with_boundary_promotion_history();
        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(200),
                out_of_sample_sharpe_30d: Some(0.2),
                promotion_failure_rate_last_10: Some(0.70),
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-start-provided-rate".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect_err("caller-supplied promotion failure rate must be rejected");
        assert_eq!(error.code, AlphaLifecycleReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "promotion_failure_rate_last_10")
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_rejects_out_of_range_history_limit() {
        let service = AlphaLifecycleActionService::in_memory();
        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "deallocate".to_string(),
                deallocation_policies: vec![AlphaLifecycleDeallocationPolicy {
                    metric_key: AlphaHealthMetricKey::RollingDrawdown,
                    comparator: ValidationGateComparator::Gt,
                    threshold_value: 0.1,
                }],
                trade_count_30d: None,
                out_of_sample_sharpe_30d: None,
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(0),
                correlation_id: "corr-alpha-lifecycle-start-invalid-history-limit".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect_err("history_limit=0 must be rejected");
        assert_eq!(error.code, AlphaLifecycleReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "history_limit")
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_rejects_mixed_stop_research_fields_on_deallocate() {
        let service = AlphaLifecycleActionService::in_memory();
        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "deallocate".to_string(),
                deallocation_policies: vec![AlphaLifecycleDeallocationPolicy {
                    metric_key: AlphaHealthMetricKey::RollingDrawdown,
                    comparator: ValidationGateComparator::Gt,
                    threshold_value: 0.1,
                }],
                trade_count_30d: Some(210),
                out_of_sample_sharpe_30d: None,
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-mixed-fields-001".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect_err("deallocate must reject stop-research input fields");
        assert_eq!(error.code, AlphaLifecycleReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "trade_count_30d")
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_rejects_deallocation_policies_on_stop_research() {
        let service = AlphaLifecycleActionService::in_memory();
        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: vec![AlphaLifecycleDeallocationPolicy {
                    metric_key: AlphaHealthMetricKey::RollingDrawdown,
                    comparator: ValidationGateComparator::Gt,
                    threshold_value: 0.1,
                }],
                trade_count_30d: Some(180),
                out_of_sample_sharpe_30d: Some(0.4),
                promotion_failure_rate_last_10: Some(0.8),
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-mixed-fields-002".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect_err("stop_research must reject deallocation policy payloads");
        assert_eq!(error.code, AlphaLifecycleReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "deallocation_policies")
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_stop_research_derives_failure_rate_from_promotion_history() {
        let mut decisions = Vec::new();
        for index in 0..8 {
            decisions.push(sample_promotion_decision(
                &format!("decision-denied-{index}"),
                PromotionDecisionState::Denied,
                &format!("2026-04-08T01:{index:02}:00Z"),
            ));
        }
        for index in 0..2 {
            decisions.push(sample_promotion_decision(
                &format!("decision-allowed-{index}"),
                PromotionDecisionState::Allowed,
                &format!("2026-04-08T00:{index:02}:00Z"),
            ));
        }
        let service = AlphaLifecycleActionService::in_memory().with_promotion_history_port(
            Arc::new(InMemoryPromotionHistoryPort::with_decisions(decisions)),
        );
        let evidence = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(260),
                out_of_sample_sharpe_30d: Some(0.41),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-start-003".to_string(),
                requested_at_utc: "2026-04-08T01:30:00Z".to_string(),
            })
            .expect("stop_research derived failure-rate evaluation should succeed");
        assert_eq!(
            evidence.action.action_status,
            AlphaLifecycleActionStatus::Applied
        );
        assert_eq!(
            evidence.action.reason_code,
            AlphaLifecycleReasonCode::StopResearchCriteriaMet.code()
        );
        let snapshot = evidence
            .action
            .stop_research_criteria
            .expect("snapshot should be persisted");
        assert!(snapshot.promotion_failure_rate_last_10 > 0.70);
        assert!(
            snapshot
                .triggered_criteria
                .iter()
                .any(|criterion| criterion == "promotion_failure_rate_last_10_above_maximum")
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_stop_research_supports_candidate_lookup_seam() {
        let mut decisions = Vec::new();
        for index in 0..8 {
            decisions.push(sample_promotion_decision_for_candidate(
                &format!("candidate-decision-denied-{index}"),
                PromotionDecisionState::Denied,
                &format!("2026-04-08T01:{index:02}:00Z"),
                "candidate::mean-reversion",
            ));
        }
        for index in 0..2 {
            decisions.push(sample_promotion_decision_for_candidate(
                &format!("candidate-decision-allowed-{index}"),
                PromotionDecisionState::Allowed,
                &format!("2026-04-08T00:{index:02}:00Z"),
                "candidate::mean-reversion",
            ));
        }
        let service = AlphaLifecycleActionService::in_memory().with_promotion_history_port(
            Arc::new(InMemoryPromotionHistoryPort::with_decisions(decisions)),
        );
        let evidence = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(260),
                out_of_sample_sharpe_30d: Some(0.41),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-candidate-seam-001".to_string(),
                requested_at_utc: "2026-04-08T01:30:00Z".to_string(),
            })
            .expect("stop_research should support canonical candidate lookup seam");
        assert_eq!(
            evidence.action.reason_code,
            AlphaLifecycleReasonCode::StopResearchCriteriaMet.code()
        );
    }

    #[test]
    fn alpha_lifecycle_action_start_stop_research_fails_closed_when_promotion_lookup_is_ambiguous()
    {
        let mut decisions = Vec::new();
        for index in 0..10 {
            decisions.push(sample_promotion_decision_for_candidate(
                &format!("alpha-decision-{index}"),
                PromotionDecisionState::Denied,
                &format!("2026-04-08T02:{index:02}:00Z"),
                "alpha::mean-reversion",
            ));
            decisions.push(sample_promotion_decision_for_candidate(
                &format!("candidate-decision-{index}"),
                PromotionDecisionState::Denied,
                &format!("2026-04-08T01:{index:02}:00Z"),
                "candidate::mean-reversion",
            ));
        }
        let service = AlphaLifecycleActionService::in_memory().with_promotion_history_port(
            Arc::new(InMemoryPromotionHistoryPort::with_decisions(decisions)),
        );
        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(260),
                out_of_sample_sharpe_30d: Some(0.41),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-candidate-seam-ambiguous".to_string(),
                requested_at_utc: "2026-04-08T01:30:00Z".to_string(),
            })
            .expect_err("ambiguous alpha/candidate promotion history must fail closed");
        assert_eq!(
            error.code,
            AlphaLifecycleReasonCode::StateUnavailable.code()
        );
        assert!(error.message.contains("ambiguous"));
    }

    #[test]
    fn alpha_lifecycle_action_start_stop_research_fails_closed_when_promotion_window_is_incomplete()
    {
        let decisions = vec![
            sample_promotion_decision(
                "decision-denied-0",
                PromotionDecisionState::Denied,
                "2026-04-08T01:00:00Z",
            ),
            sample_promotion_decision(
                "decision-allowed-0",
                PromotionDecisionState::Allowed,
                "2026-04-08T00:59:00Z",
            ),
            sample_promotion_decision(
                "decision-denied-1",
                PromotionDecisionState::Denied,
                "2026-04-08T00:58:00Z",
            ),
        ];
        let service = AlphaLifecycleActionService::in_memory().with_promotion_history_port(
            Arc::new(InMemoryPromotionHistoryPort::with_decisions(decisions)),
        );

        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(260),
                out_of_sample_sharpe_30d: Some(0.41),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(3),
                correlation_id: "corr-alpha-lifecycle-start-incomplete-window".to_string(),
                requested_at_utc: "2026-04-08T01:30:00Z".to_string(),
            })
            .expect_err("stop_research should fail closed when fewer than 10 promotions exist");

        assert_eq!(
            error.code,
            AlphaLifecycleReasonCode::StateUnavailable.code()
        );
        assert!(error.message.contains("at least 10 promote decisions"));
    }

    #[test]
    fn alpha_lifecycle_action_start_stop_research_expands_history_lookup_when_initial_window_has_non_promote_actions()
     {
        let mut decisions = Vec::new();
        for index in 0..10 {
            let mut pause_decision = sample_promotion_decision(
                &format!("decision-pause-{index}"),
                PromotionDecisionState::Allowed,
                &format!("2026-04-08T02:{index:02}:00Z"),
            );
            pause_decision.lifecycle_action = PromotionLifecycleAction::Pause;
            decisions.push(pause_decision);
        }
        for index in 0..8 {
            decisions.push(sample_promotion_decision(
                &format!("decision-denied-history-{index}"),
                PromotionDecisionState::Denied,
                &format!("2026-04-08T01:{index:02}:00Z"),
            ));
        }
        for index in 0..2 {
            decisions.push(sample_promotion_decision(
                &format!("decision-allowed-history-{index}"),
                PromotionDecisionState::Allowed,
                &format!("2026-04-08T00:{index:02}:00Z"),
            ));
        }
        let service = AlphaLifecycleActionService::in_memory().with_promotion_history_port(
            Arc::new(InMemoryPromotionHistoryPort::with_decisions(decisions)),
        );
        let evidence = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(260),
                out_of_sample_sharpe_30d: Some(0.41),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-history-expansion-001".to_string(),
                requested_at_utc: "2026-04-08T03:30:00Z".to_string(),
            })
            .expect("stop_research should expand fetch scope and derive failure rate");
        assert_eq!(
            evidence.action.reason_code,
            AlphaLifecycleReasonCode::StopResearchCriteriaMet.code()
        );
    }

    #[test]
    fn alpha_lifecycle_fail_closed_signal_filters_expected_reason_codes() {
        assert!(should_emit_fail_closed_security_signal(
            "deny",
            AlphaLifecycleReasonCode::DependencyUnavailable.code()
        ));
        assert!(should_emit_fail_closed_security_signal(
            "deny",
            AlphaLifecycleReasonCode::StateUnavailable.code()
        ));
        assert!(should_emit_fail_closed_security_signal(
            "deny",
            AlphaLifecycleReasonCode::PersistenceUnavailable.code()
        ));
        assert!(should_emit_fail_closed_security_signal(
            "deny",
            AlphaLifecycleReasonCode::AuthorizationFailed.code()
        ));
        assert!(should_emit_fail_closed_security_signal(
            "fail_closed",
            AlphaLifecycleReasonCode::ActionDenied.code()
        ));
        assert!(!should_emit_fail_closed_security_signal(
            "deny",
            AlphaLifecycleReasonCode::ActionDenied.code()
        ));
        assert!(!should_emit_fail_closed_security_signal(
            "allow",
            AlphaLifecycleReasonCode::ActionRead.code()
        ));
    }

    #[test]
    fn alpha_lifecycle_action_list_orders_records_deterministically() {
        let service = service_with_boundary_promotion_history();
        let first = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(180),
                out_of_sample_sharpe_30d: Some(0.4),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-list-001".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect("first action should persist")
            .action;
        let second = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(170),
                out_of_sample_sharpe_30d: Some(0.3),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-list-002".to_string(),
                requested_at_utc: "2026-04-08T02:00:00Z".to_string(),
            })
            .expect("second action should persist")
            .action;

        let listed = service
            .list_alpha_lifecycle_actions(ListAlphaLifecycleActionsInput {
                actor_id: "ops-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                limit: Some(10),
                acted_after_utc: None,
                acted_before_utc: None,
                correlation_id: "corr-alpha-lifecycle-list-003".to_string(),
                queried_at_utc: "2026-04-08T02:10:00Z".to_string(),
            })
            .expect("list should succeed");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].action_id, second.action_id);
        assert_eq!(listed[1].action_id, first.action_id);
    }

    #[test]
    fn alpha_lifecycle_action_start_same_timestamp_uses_distinct_contextual_ids() {
        let service = service_with_boundary_promotion_history();
        let first = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(180),
                out_of_sample_sharpe_30d: Some(0.4),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-collision-001".to_string(),
                requested_at_utc: "2026-04-08T02:00:00Z".to_string(),
            })
            .expect("first action should persist")
            .action;
        let second = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(170),
                out_of_sample_sharpe_30d: Some(0.3),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-collision-002".to_string(),
                requested_at_utc: "2026-04-08T02:00:00Z".to_string(),
            })
            .expect("second action should persist")
            .action;

        assert_ne!(first.action_id, second.action_id);
        assert!(first.action_id.starts_with(
            "alpha::mean-reversion::stop_research::corr-alpha-lifecycle-collision-001::"
        ));
        assert!(second.action_id.starts_with(
            "alpha::mean-reversion::stop_research::corr-alpha-lifecycle-collision-002::"
        ));
    }

    #[test]
    fn alpha_lifecycle_action_list_rejects_out_of_range_limit() {
        let service = AlphaLifecycleActionService::in_memory();
        let error = service
            .list_alpha_lifecycle_actions(ListAlphaLifecycleActionsInput {
                actor_id: "ops-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                limit: Some(0),
                acted_after_utc: None,
                acted_before_utc: None,
                correlation_id: "corr-alpha-lifecycle-list-invalid-limit".to_string(),
                queried_at_utc: "2026-04-08T02:10:00Z".to_string(),
            })
            .expect_err("limit=0 must be rejected");
        assert_eq!(error.code, AlphaLifecycleReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );
    }

    #[test]
    fn alpha_lifecycle_action_list_rejects_invalid_time_window_ordering() {
        let service = AlphaLifecycleActionService::in_memory();
        let error = service
            .list_alpha_lifecycle_actions(ListAlphaLifecycleActionsInput {
                actor_id: "ops-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                limit: Some(10),
                acted_after_utc: Some("2026-04-08T03:00:00Z".to_string()),
                acted_before_utc: Some("2026-04-08T02:00:00Z".to_string()),
                correlation_id: "corr-alpha-lifecycle-list-invalid-window".to_string(),
                queried_at_utc: "2026-04-08T03:05:00Z".to_string(),
            })
            .expect_err("invalid window ordering must be rejected");
        assert_eq!(error.code, AlphaLifecycleReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "acted_before_utc")
        );
    }

    #[test]
    fn alpha_lifecycle_action_list_applies_inclusive_after_and_exclusive_before_boundaries() {
        let service = service_with_boundary_promotion_history();
        let first = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(180),
                out_of_sample_sharpe_30d: Some(0.4),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-window-001".to_string(),
                requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
            })
            .expect("first action should persist")
            .action;
        let second = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "stop_research".to_string(),
                deallocation_policies: Vec::new(),
                trade_count_30d: Some(170),
                out_of_sample_sharpe_30d: Some(0.3),
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-window-002".to_string(),
                requested_at_utc: "2026-04-08T02:00:00Z".to_string(),
            })
            .expect("second action should persist")
            .action;

        let listed = service
            .list_alpha_lifecycle_actions(ListAlphaLifecycleActionsInput {
                actor_id: "ops-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                limit: Some(10),
                acted_after_utc: Some("2026-04-08T01:00:00Z".to_string()),
                acted_before_utc: Some("2026-04-08T02:00:00Z".to_string()),
                correlation_id: "corr-alpha-lifecycle-window-query".to_string(),
                queried_at_utc: "2026-04-08T02:10:00Z".to_string(),
            })
            .expect("windowed list should succeed");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].action_id, first.action_id);
        assert_ne!(listed[0].action_id, second.action_id);
    }

    #[test]
    fn alpha_lifecycle_action_start_fails_closed_when_breach_dependency_is_unavailable() {
        let service = AlphaLifecycleActionService::in_memory()
            .with_breach_evidence_port(Arc::new(FailingAlphaBreachEvidencePort));
        let error = service
            .start_alpha_lifecycle_action(StartAlphaLifecycleActionInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                action_type: "deallocate".to_string(),
                deallocation_policies: vec![AlphaLifecycleDeallocationPolicy {
                    metric_key: AlphaHealthMetricKey::RollingDrawdown,
                    comparator: ValidationGateComparator::Gt,
                    threshold_value: 0.1,
                }],
                trade_count_30d: None,
                out_of_sample_sharpe_30d: None,
                promotion_failure_rate_last_10: None,
                approval_request_id: None,
                approval_reference: None,
                history_limit: Some(10),
                correlation_id: "corr-alpha-lifecycle-dependency-001".to_string(),
                requested_at_utc: "2026-04-08T03:00:00Z".to_string(),
            })
            .expect_err(
                "deallocation start should fail closed when breach dependency is unavailable",
            );
        assert_eq!(
            error.code,
            AlphaLifecycleReasonCode::DependencyUnavailable.code()
        );
        assert!(error.message.contains("dependency unavailable"));
    }
}
