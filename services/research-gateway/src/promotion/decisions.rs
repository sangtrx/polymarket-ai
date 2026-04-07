use crate::promotion::counterfactual_replay::{
    CounterfactualReplayOrchestrator, CounterfactualReplayService,
    CounterfactualReplayServiceError, StartCounterfactualReplayInput,
};
use crate::promotion::evaluate_promotion_entry_gates;
use crate::validation::gate_policies::{
    ValidationGatePolicyOrchestrator, ValidationGatePolicyService, ValidationGatePolicyServiceError,
};
use domain::research::{
    CounterfactualReplayGateOutcome, CounterfactualReplayReasonCode,
    PromotionDecisionContractError, PromotionDecisionReasonCode, PromotionDecisionRecord,
    PromotionDecisionState, PromotionDecisionValidationIssue, PromotionLifecycleAction,
    PromotionThresholdDefinition, ShadowEvaluationRecord, ValidationGateReasonCode,
    ValidationWorkflowArtifactRecord, ValidationWorkflowRunRecord, ValidationWorkflowRunState,
    ValidationWorkflowStage, canonicalize_promotion_decision_record, compose_promotion_decision_id,
    evaluate_promotion_thresholds, normalize_research_identifier, parse_promotion_utc_timestamp,
    validate_promotion_evidence_packet,
};
use persistence::postgres::promotion_decisions::{
    PromotionDecisionPersistenceError,
    list_promotion_decisions_by_candidate as pg_list_promotion_decisions_by_candidate,
    load_promotion_decision as pg_load_promotion_decision,
    upsert_promotion_decision as pg_upsert_promotion_decision,
};
use persistence::postgres::shadow_evaluations::{
    ShadowEvaluationPersistenceError,
    list_shadow_evaluations_by_candidate as pg_list_shadow_evaluations_by_candidate,
};
use persistence::postgres::validation_artifacts::{
    ValidationArtifactPersistenceError,
    list_validation_artifacts_by_run as pg_list_validation_artifacts_by_run,
};
use persistence::postgres::validation_runs::{
    ValidationRunPersistenceError, load_validation_run as pg_load_validation_run,
};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

const DEFAULT_LIST_LIMIT: i64 = 25;
const MAX_LIST_LIMIT: i64 = 200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PromotionDecisionServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<PromotionDecisionValidationIssue>,
}

impl PromotionDecisionServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<PromotionDecisionValidationIssue>,
    ) -> Self {
        Self {
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: PromotionDecisionReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for promotion decision mutations".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: PromotionDecisionReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for promotion decision reads".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn decision_not_found(decision_id: &str) -> Self {
        Self {
            code: PromotionDecisionReasonCode::DecisionNotFound.code(),
            message: format!("promotion decision `{decision_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: PromotionDecisionReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: PromotionDecisionReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: PromotionDecisionReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for PromotionDecisionServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for PromotionDecisionServiceError {}

#[derive(Debug, Clone)]
pub struct StartPromotionDecisionInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub validation_run_id: String,
    pub lifecycle_action: String,
    pub observed_metrics: Value,
    pub thresholds: Vec<PromotionThresholdDefinition>,
    pub evidence_packet: Value,
    pub shadow_readiness: Option<Value>,
    pub approval_request_id: Option<String>,
    pub approval_reference: Option<String>,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadPromotionDecisionInput {
    pub actor_id: String,
    pub actor_role: String,
    pub decision_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListPromotionDecisionsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub limit: Option<i64>,
    pub decided_after_utc: Option<String>,
    pub decided_before_utc: Option<String>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PromotionDecisionEvidence {
    pub decision: PromotionDecisionRecord,
    pub reason_code: String,
}

pub trait PromotionDecisionOrchestrator: Send + Sync {
    fn start_promotion_decision(
        &self,
        input: StartPromotionDecisionInput,
    ) -> Result<PromotionDecisionEvidence, PromotionDecisionServiceError>;

    fn read_promotion_decision(
        &self,
        input: ReadPromotionDecisionInput,
    ) -> Result<PromotionDecisionEvidence, PromotionDecisionServiceError>;

    fn list_promotion_decisions(
        &self,
        input: ListPromotionDecisionsInput,
    ) -> Result<Vec<PromotionDecisionRecord>, PromotionDecisionServiceError>;
}

pub trait PromotionDecisionRepositoryPort: Send + Sync {
    fn upsert(&self, record: PromotionDecisionRecord) -> Result<(), PromotionDecisionServiceError>;
    fn load(
        &self,
        decision_id: &str,
    ) -> Result<Option<PromotionDecisionRecord>, PromotionDecisionServiceError>;
    fn list_by_candidate(
        &self,
        candidate_id: &str,
        decided_after_utc: Option<&str>,
        decided_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PromotionDecisionRecord>, PromotionDecisionServiceError>;
}

pub trait ValidationEvidencePort: Send + Sync {
    fn load_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, PromotionDecisionServiceError>;
    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, PromotionDecisionServiceError>;
}

pub trait ShadowEvidencePort: Send + Sync {
    fn read_latest_shadow_readiness(
        &self,
        candidate_id: &str,
    ) -> Result<Option<Value>, PromotionDecisionServiceError>;
}

#[derive(Clone)]
pub struct PromotionDecisionService {
    repository: Arc<dyn PromotionDecisionRepositoryPort>,
    validation_evidence: Arc<dyn ValidationEvidencePort>,
    validation_gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
    shadow_evidence: Arc<dyn ShadowEvidencePort>,
    counterfactual_replay_orchestrator: Arc<dyn CounterfactualReplayOrchestrator>,
    operation_lock: Arc<Mutex<()>>,
}

impl PromotionDecisionService {
    pub fn new(
        repository: Arc<dyn PromotionDecisionRepositoryPort>,
        validation_evidence: Arc<dyn ValidationEvidencePort>,
        validation_gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
        shadow_evidence: Arc<dyn ShadowEvidencePort>,
        counterfactual_replay_orchestrator: Arc<dyn CounterfactualReplayOrchestrator>,
    ) -> Self {
        Self {
            repository,
            validation_evidence,
            validation_gate_orchestrator,
            shadow_evidence,
            counterfactual_replay_orchestrator,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryPromotionDecisionRepository::default()),
            Arc::new(StaticValidationEvidencePort::default()),
            Arc::new(ValidationGatePolicyService::default()),
            Arc::new(StaticShadowEvidencePort),
            Arc::new(CounterfactualReplayService::in_memory()),
        )
    }

    pub fn postgres(
        pool: PgPool,
        gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
    ) -> Self {
        Self::new(
            Arc::new(PostgresPromotionDecisionRepository::new(pool.clone())),
            Arc::new(PostgresValidationEvidencePort::new(pool.clone())),
            gate_orchestrator,
            Arc::new(PostgresShadowEvidencePort::new(pool.clone())),
            Arc::new(CounterfactualReplayService::postgres(pool.clone())),
        )
    }

    pub fn with_validation_evidence_port(
        mut self,
        validation_evidence: Arc<dyn ValidationEvidencePort>,
    ) -> Self {
        self.validation_evidence = validation_evidence;
        self
    }

    pub fn with_validation_gate_orchestrator(
        mut self,
        validation_gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
    ) -> Self {
        self.validation_gate_orchestrator = validation_gate_orchestrator;
        self
    }

    pub fn with_shadow_evidence_port(
        mut self,
        shadow_evidence: Arc<dyn ShadowEvidencePort>,
    ) -> Self {
        self.shadow_evidence = shadow_evidence;
        self
    }

    pub fn with_counterfactual_replay_orchestrator(
        mut self,
        counterfactual_replay_orchestrator: Arc<dyn CounterfactualReplayOrchestrator>,
    ) -> Self {
        self.counterfactual_replay_orchestrator = counterfactual_replay_orchestrator;
        self
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, PromotionDecisionServiceError> {
        self.operation_lock.lock().map_err(|_| {
            PromotionDecisionServiceError::persistence_unavailable(
                "promotion decision operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for PromotionDecisionService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl PromotionDecisionOrchestrator for PromotionDecisionService {
    fn start_promotion_decision(
        &self,
        input: StartPromotionDecisionInput,
    ) -> Result<PromotionDecisionEvidence, PromotionDecisionServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_promotion_decision_telemetry(
                "promotion_decision_start_v1",
                "promotion_decision_start",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                None,
                None,
                error.code,
                input.approval_reference.as_deref(),
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("candidate_id", &input.candidate_id)?;
        validate_non_empty("validation_run_id", &input.validation_run_id)?;
        validate_non_empty("lifecycle_action", &input.lifecycle_action)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("requested_at_utc", &input.requested_at_utc)?;
        validate_utc_timestamp("requested_at_utc", &input.requested_at_utc)?;

        let lifecycle_action =
            PromotionLifecycleAction::parse(&input.lifecycle_action).map_err(map_contract_error)?;
        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        let normalized_validation_run_id = normalize_research_identifier(&input.validation_run_id);
        let decision_id =
            compose_promotion_decision_id(&normalized_candidate_id, &input.requested_at_utc)
                .map_err(map_contract_error)?;
        let _requested_at =
            parse_promotion_utc_timestamp(&input.requested_at_utc).map_err(map_contract_error)?;
        let _lock = self.lock_operations()?;

        let mut evidence_packet = input.evidence_packet.clone();
        let threshold_results =
            evaluate_promotion_thresholds(&input.thresholds, &input.observed_metrics)
                .map_err(map_contract_error)?;
        let packet_missing_fields =
            validate_promotion_evidence_packet(lifecycle_action, &evidence_packet)
                .map_err(map_contract_error)?;

        let validation_run = self
            .validation_evidence
            .load_validation_run(&normalized_validation_run_id)?;
        let validation_artifacts = self
            .validation_evidence
            .list_validation_artifacts_by_run(&normalized_validation_run_id)?;
        let mut missing_evidence_fields = collect_missing_evidence_fields(
            lifecycle_action,
            &normalized_candidate_id,
            validation_run.as_ref(),
            &validation_artifacts,
            packet_missing_fields,
        );
        let mut replay_gate_denied = false;
        let mut replay_run_id: Option<String> = None;

        if lifecycle_action == PromotionLifecycleAction::Promote {
            let replay_evidence = self
                .counterfactual_replay_orchestrator
                .start_counterfactual_replay(StartCounterfactualReplayInput {
                    actor_id: input.actor_id.clone(),
                    actor_role: input.actor_role.clone(),
                    candidate_id: normalized_candidate_id.clone(),
                    validation_run_id: normalized_validation_run_id.clone(),
                    correlation_id: input.correlation_id.clone(),
                    requested_at_utc: input.requested_at_utc.clone(),
                })
                .map_err(map_replay_error)?;
            if let Some(packet) = evidence_packet.as_object_mut() {
                packet.insert(
                    "counterfactual_replay_summary".to_string(),
                    serde_json::to_value(&replay_evidence.replay_run.replay_summary).map_err(
                        |_| {
                            PromotionDecisionServiceError::state_unavailable(
                                "counterfactual replay summary serialization failed",
                            )
                        },
                    )?,
                );
            }
            missing_evidence_fields.retain(|field| field != "counterfactual_replay_summary");
            replay_gate_denied = replay_evidence.replay_run.replay_summary.gate_outcome
                == CounterfactualReplayGateOutcome::Deny;
            replay_run_id = Some(replay_evidence.replay_run.run_id.clone());
        }

        let mut gate_denied = false;
        let gate_evaluation = match evaluate_promotion_entry_gates(
            self.validation_gate_orchestrator.as_ref(),
            input.actor_id.clone(),
            input.actor_role.clone(),
            normalized_candidate_id.clone(),
            input.observed_metrics.clone(),
            input.correlation_id.clone(),
            input.requested_at_utc.clone(),
        ) {
            Ok(evidence) => serde_json::to_value(&evidence).unwrap_or_else(|_| {
                json!({
                    "outcome": evidence.outcome,
                    "reason_code": evidence.reason_code,
                    "failed_gate_ids": evidence.failed_gate_ids
                })
            }),
            Err(error) if is_gate_denied_error(error.code) => {
                gate_denied = true;
                json!({
                    "outcome": "deny",
                    "reason_code": error.code,
                    "message": error.message,
                    "field_errors": error.field_errors,
                    "failed_gate_ids": error.failed_gate_ids,
                })
            }
            Err(error) if is_gate_dependency_error(error.code) => {
                let service_error =
                    PromotionDecisionServiceError::dependency_unavailable(error.message);
                emit_promotion_decision_telemetry(
                    "promotion_decision_start_v1",
                    "promotion_decision_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    Some(&decision_id),
                    None,
                    service_error.code,
                    input.approval_reference.as_deref(),
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
                return Err(service_error);
            }
            Err(error) => {
                let service_error = map_gate_error(error);
                emit_promotion_decision_telemetry(
                    "promotion_decision_start_v1",
                    "promotion_decision_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    Some(&decision_id),
                    None,
                    service_error.code,
                    input.approval_reference.as_deref(),
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
                return Err(service_error);
            }
        };

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
            return Err(PromotionDecisionServiceError::invalid_payload(
                "approval_request_id is required when approval_reference is provided",
                vec![PromotionDecisionValidationIssue {
                    field: "approval_request_id".to_string(),
                    code: PromotionDecisionReasonCode::ApprovalInvalidState.code(),
                    message: "approval_request_id is required when approval_reference is provided"
                        .to_string(),
                }],
            ));
        }

        let shadow_readiness = match input.shadow_readiness {
            Some(shadow_readiness) => Some(shadow_readiness),
            None => self
                .shadow_evidence
                .read_latest_shadow_readiness(&normalized_candidate_id)?,
        };

        missing_evidence_fields.sort();
        missing_evidence_fields.dedup();

        let has_failed_thresholds = threshold_results.iter().any(|result| !result.passed);
        let mut decision_state = PromotionDecisionState::Allowed;
        let mut decision_reason_code = PromotionDecisionReasonCode::DecisionAllowed
            .code()
            .to_string();
        if !missing_evidence_fields.is_empty() {
            decision_state = PromotionDecisionState::Denied;
            decision_reason_code = PromotionDecisionReasonCode::MissingEvidence
                .code()
                .to_string();
        } else if has_failed_thresholds {
            decision_state = PromotionDecisionState::Denied;
            decision_reason_code = PromotionDecisionReasonCode::ThresholdFailed
                .code()
                .to_string();
        } else if gate_denied {
            decision_state = PromotionDecisionState::Denied;
            decision_reason_code = PromotionDecisionReasonCode::GateDenied.code().to_string();
        } else if replay_gate_denied {
            decision_state = PromotionDecisionState::Denied;
            decision_reason_code = PromotionDecisionReasonCode::ReplayGateDenied
                .code()
                .to_string();
        } else if lifecycle_action == PromotionLifecycleAction::Promote
            && approval_reference.is_none()
        {
            decision_state = PromotionDecisionState::Denied;
            decision_reason_code = PromotionDecisionReasonCode::ApprovalRequired
                .code()
                .to_string();
        }

        let decision = canonicalize_promotion_decision_record(&PromotionDecisionRecord {
            decision_id: decision_id.clone(),
            candidate_id: normalized_candidate_id.clone(),
            validation_run_id: normalized_validation_run_id.clone(),
            lifecycle_action,
            decision_state,
            reason_code: decision_reason_code.clone(),
            observed_metrics: input.observed_metrics.clone(),
            evidence_packet,
            threshold_results,
            missing_evidence_fields,
            gate_evaluation,
            shadow_readiness,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            decided_at_utc: input.requested_at_utc.clone(),
            approval_request_id,
            approval_reference: approval_reference.clone(),
        })
        .map_err(map_contract_error)?;

        self.repository.upsert(decision.clone())?;
        emit_promotion_decision_telemetry(
            "promotion_decision_start_v1",
            "promotion_decision_start",
            if decision.decision_state == PromotionDecisionState::Allowed {
                "allow"
            } else {
                "deny"
            },
            &input.actor_id,
            &decision.candidate_id,
            Some(&decision.decision_id),
            replay_run_id.as_deref(),
            &decision.reason_code,
            decision.approval_reference.as_deref(),
            &decision.correlation_id,
            &decision.decided_at_utc,
        );

        Ok(PromotionDecisionEvidence {
            decision,
            reason_code: PromotionDecisionReasonCode::DecisionStarted
                .code()
                .to_string(),
        })
    }

    fn read_promotion_decision(
        &self,
        input: ReadPromotionDecisionInput,
    ) -> Result<PromotionDecisionEvidence, PromotionDecisionServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_promotion_decision_telemetry(
                "promotion_decision_read_v1",
                "promotion_decision_read",
                "deny",
                &input.actor_id,
                &input.decision_id,
                Some(&input.decision_id),
                None,
                error.code,
                None,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("decision_id", &input.decision_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        let normalized_decision_id = normalize_research_identifier(&input.decision_id);
        let Some(decision) = self.repository.load(&normalized_decision_id)? else {
            let error = PromotionDecisionServiceError::decision_not_found(&normalized_decision_id);
            emit_promotion_decision_telemetry(
                "promotion_decision_read_v1",
                "promotion_decision_read",
                "deny",
                &input.actor_id,
                &normalized_decision_id,
                Some(&normalized_decision_id),
                None,
                error.code,
                None,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        };

        emit_promotion_decision_telemetry(
            "promotion_decision_read_v1",
            "promotion_decision_read",
            "allow",
            &input.actor_id,
            &decision.candidate_id,
            Some(&decision.decision_id),
            None,
            PromotionDecisionReasonCode::DecisionRead.code(),
            decision.approval_reference.as_deref(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(PromotionDecisionEvidence {
            decision,
            reason_code: PromotionDecisionReasonCode::DecisionRead.code().to_string(),
        })
    }

    fn list_promotion_decisions(
        &self,
        input: ListPromotionDecisionsInput,
    ) -> Result<Vec<PromotionDecisionRecord>, PromotionDecisionServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_promotion_decision_telemetry(
                "promotion_decision_list_v1",
                "promotion_decision_list",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                None,
                None,
                error.code,
                None,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("candidate_id", &input.candidate_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        if let Some(limit) = input.limit
            && limit <= 0
        {
            return Err(PromotionDecisionServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![PromotionDecisionValidationIssue {
                    field: "limit".to_string(),
                    code: PromotionDecisionReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_decided_after =
            normalize_optional_timestamp("decided_after_utc", input.decided_after_utc.as_deref())?;
        let normalized_decided_before = normalize_optional_timestamp(
            "decided_before_utc",
            input.decided_before_utc.as_deref(),
        )?;
        if let (Some(decided_after), Some(decided_before)) = (
            normalized_decided_after.as_deref(),
            normalized_decided_before.as_deref(),
        ) {
            let decided_after_ts =
                parse_promotion_utc_timestamp(decided_after).map_err(map_contract_error)?;
            let decided_before_ts =
                parse_promotion_utc_timestamp(decided_before).map_err(map_contract_error)?;
            if decided_before_ts <= decided_after_ts {
                return Err(PromotionDecisionServiceError::invalid_payload(
                    "decided_before_utc must be greater than decided_after_utc",
                    vec![PromotionDecisionValidationIssue {
                        field: "decided_before_utc".to_string(),
                        code: PromotionDecisionReasonCode::InvalidPayload.code(),
                        message: "decided_before_utc must be greater than decided_after_utc"
                            .to_string(),
                    }],
                ));
            }
        }

        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            return Err(PromotionDecisionServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![PromotionDecisionValidationIssue {
                    field: "candidate_id".to_string(),
                    code: PromotionDecisionReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            ));
        }
        let limit = input
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT);
        let decisions = self.repository.list_by_candidate(
            &normalized_candidate_id,
            normalized_decided_after.as_deref(),
            normalized_decided_before.as_deref(),
            limit,
        )?;
        emit_promotion_decision_telemetry(
            "promotion_decision_list_v1",
            "promotion_decision_list",
            "allow",
            &input.actor_id,
            &normalized_candidate_id,
            None,
            None,
            PromotionDecisionReasonCode::DecisionListed.code(),
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(decisions)
    }
}

fn collect_missing_evidence_fields(
    lifecycle_action: PromotionLifecycleAction,
    candidate_id: &str,
    validation_run: Option<&ValidationWorkflowRunRecord>,
    artifacts: &[ValidationWorkflowArtifactRecord],
    mut packet_missing_fields: Vec<String>,
) -> Vec<String> {
    let validation_run_valid = validation_run
        .as_ref()
        .map(|run| {
            run.run_state == ValidationWorkflowRunState::Completed
                && run.candidate_id == candidate_id
        })
        .unwrap_or(false);
    if !validation_run_valid {
        packet_missing_fields.push("validation_run_id".to_string());
    }

    if lifecycle_action == PromotionLifecycleAction::Promote {
        let coverage = collect_validation_artifact_coverage(artifacts);
        if !coverage.has_quality {
            packet_missing_fields.push("data_quality_report".to_string());
        }
        if !(coverage.has_purged_cv && coverage.has_cpcv) {
            packet_missing_fields.push("purged_cpcv_results".to_string());
        }
        if !coverage.has_calibration {
            packet_missing_fields.push("calibration_report".to_string());
        }
    }

    packet_missing_fields
        .into_iter()
        .map(|field| normalize_research_identifier(&field))
        .filter(|field| !field.is_empty())
        .collect()
}

#[derive(Default)]
struct ValidationArtifactCoverage {
    has_quality: bool,
    has_purged_cv: bool,
    has_cpcv: bool,
    has_calibration: bool,
}

fn collect_validation_artifact_coverage(
    artifacts: &[ValidationWorkflowArtifactRecord],
) -> ValidationArtifactCoverage {
    let mut coverage = ValidationArtifactCoverage::default();
    for artifact in artifacts {
        match artifact.stage {
            ValidationWorkflowStage::Quality => coverage.has_quality = true,
            ValidationWorkflowStage::PurgedCv => coverage.has_purged_cv = true,
            ValidationWorkflowStage::Cpcv => coverage.has_cpcv = true,
            ValidationWorkflowStage::OverfitDiagnostics => {}
            ValidationWorkflowStage::Labeling => {}
        }
        if artifact.diagnostics.brier_score.is_some()
            || artifact.diagnostics.expected_calibration_error.is_some()
        {
            coverage.has_calibration = true;
        }
    }
    coverage
}

fn is_gate_denied_error(code: &str) -> bool {
    code == ValidationGateReasonCode::GateFailed.code()
        || code == ValidationGateReasonCode::PolicyUnresolved.code()
        || code == ValidationGateReasonCode::MissingMandatoryPolicy.code()
}

fn is_gate_dependency_error(code: &str) -> bool {
    code == ValidationGateReasonCode::DependencyUnavailable.code()
        || code == ValidationGateReasonCode::StateUnavailable.code()
        || code == ValidationGateReasonCode::PersistenceUnavailable.code()
}

fn map_gate_error(error: ValidationGatePolicyServiceError) -> PromotionDecisionServiceError {
    PromotionDecisionServiceError::invalid_payload(
        error.message,
        error
            .field_errors
            .into_iter()
            .map(|issue| PromotionDecisionValidationIssue {
                field: issue.field,
                code: issue.code,
                message: issue.message,
            })
            .collect(),
    )
}

fn map_replay_error(error: CounterfactualReplayServiceError) -> PromotionDecisionServiceError {
    if error.code == CounterfactualReplayReasonCode::DependencyUnavailable.code() {
        return PromotionDecisionServiceError::dependency_unavailable(error.message);
    }
    if error.code == CounterfactualReplayReasonCode::StateUnavailable.code() {
        return PromotionDecisionServiceError::state_unavailable(error.message);
    }
    if error.code == CounterfactualReplayReasonCode::PersistenceUnavailable.code() {
        return PromotionDecisionServiceError::persistence_unavailable(error.message);
    }
    PromotionDecisionServiceError::invalid_payload(
        error.message,
        error
            .field_errors
            .into_iter()
            .map(|issue| PromotionDecisionValidationIssue {
                field: issue.field,
                code: issue.code,
                message: issue.message,
            })
            .collect(),
    )
}

fn map_contract_error(error: PromotionDecisionContractError) -> PromotionDecisionServiceError {
    PromotionDecisionServiceError::invalid_payload(error.message, error.field_errors)
}

fn validate_mutation_role(role: &str) -> Result<(), PromotionDecisionServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(PromotionDecisionServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), PromotionDecisionServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(PromotionDecisionServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), PromotionDecisionServiceError> {
    if value.trim().is_empty() {
        return Err(PromotionDecisionServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![PromotionDecisionValidationIssue {
                field: field.to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn validate_utc_timestamp(field: &str, value: &str) -> Result<(), PromotionDecisionServiceError> {
    parse_promotion_utc_timestamp(value).map_err(|_| {
        PromotionDecisionServiceError::invalid_payload(
            format!("{field} must be RFC3339 UTC"),
            vec![PromotionDecisionValidationIssue {
                field: field.to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: format!("{field} must be RFC3339 UTC"),
            }],
        )
    })?;
    Ok(())
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, PromotionDecisionServiceError> {
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
fn emit_promotion_decision_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    candidate_id: &str,
    decision_id: Option<&str>,
    replay_run_id: Option<&str>,
    reason_code: &str,
    approval_reference: Option<&str>,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = PromotionDecisionTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        candidate_id,
        decision_id,
        replay_run_id,
        reason_code,
        approval_reference,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(PromotionDecisionSecuritySignal {
                name: "promotion_decision_denied_v1",
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
        serde_json::to_string(&event).expect("promotion decision telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct PromotionDecisionTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    candidate_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay_run_id: Option<&'a str>,
    reason_code: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_reference: Option<&'a str>,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<PromotionDecisionSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct PromotionDecisionSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresPromotionDecisionRepository {
    pool: PgPool,
}

impl PostgresPromotionDecisionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, PromotionDecisionServiceError>
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
                    PromotionDecisionServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_promotion_persistence_error),
        }
    }
}

impl PromotionDecisionRepositoryPort for PostgresPromotionDecisionRepository {
    fn upsert(&self, record: PromotionDecisionRecord) -> Result<(), PromotionDecisionServiceError> {
        self.run_future(pg_upsert_promotion_decision(&self.pool, &record))
    }

    fn load(
        &self,
        decision_id: &str,
    ) -> Result<Option<PromotionDecisionRecord>, PromotionDecisionServiceError> {
        self.run_future(pg_load_promotion_decision(&self.pool, decision_id))
    }

    fn list_by_candidate(
        &self,
        candidate_id: &str,
        decided_after_utc: Option<&str>,
        decided_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PromotionDecisionRecord>, PromotionDecisionServiceError> {
        self.run_future(pg_list_promotion_decisions_by_candidate(
            &self.pool,
            candidate_id,
            decided_after_utc,
            decided_before_utc,
            limit,
        ))
    }
}

fn map_promotion_persistence_error(
    error: PromotionDecisionPersistenceError,
) -> PromotionDecisionServiceError {
    match error.code {
        "promotion_decision_query_failed" | "promotion_decision_row_decode_failed" => {
            PromotionDecisionServiceError::persistence_unavailable(error.message)
        }
        _ => PromotionDecisionServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct PostgresValidationEvidencePort {
    pool: PgPool,
}

impl PostgresValidationEvidencePort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_validation_run_future<F, T>(&self, future: F) -> Result<T, PromotionDecisionServiceError>
    where
        F: Future<Output = Result<T, ValidationRunPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_validation_run_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    PromotionDecisionServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_validation_run_error),
        }
    }

    fn run_validation_artifact_future<F, T>(
        &self,
        future: F,
    ) -> Result<T, PromotionDecisionServiceError>
    where
        F: Future<Output = Result<T, ValidationArtifactPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_validation_artifact_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    PromotionDecisionServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_validation_artifact_error),
        }
    }
}

impl ValidationEvidencePort for PostgresValidationEvidencePort {
    fn load_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, PromotionDecisionServiceError> {
        self.run_validation_run_future(pg_load_validation_run(&self.pool, run_id))
    }

    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, PromotionDecisionServiceError> {
        self.run_validation_artifact_future(pg_list_validation_artifacts_by_run(&self.pool, run_id))
    }
}

fn map_validation_run_error(error: ValidationRunPersistenceError) -> PromotionDecisionServiceError {
    match error.code {
        "validation_run_query_failed" => {
            PromotionDecisionServiceError::dependency_unavailable(error.message)
        }
        "validation_run_row_decode_failed" => {
            PromotionDecisionServiceError::state_unavailable(error.message)
        }
        _ => PromotionDecisionServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| PromotionDecisionValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn map_validation_artifact_error(
    error: ValidationArtifactPersistenceError,
) -> PromotionDecisionServiceError {
    match error.code {
        "validation_artifact_query_failed" => {
            PromotionDecisionServiceError::dependency_unavailable(error.message)
        }
        "validation_artifact_row_decode_failed" => {
            PromotionDecisionServiceError::state_unavailable(error.message)
        }
        _ => PromotionDecisionServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| PromotionDecisionValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

#[derive(Debug, Clone)]
pub struct PostgresShadowEvidencePort {
    pool: PgPool,
}

impl PostgresShadowEvidencePort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_shadow_future<F, T>(&self, future: F) -> Result<T, PromotionDecisionServiceError>
    where
        F: Future<Output = Result<T, ShadowEvaluationPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_shadow_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    PromotionDecisionServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_shadow_persistence_error),
        }
    }
}

impl ShadowEvidencePort for PostgresShadowEvidencePort {
    fn read_latest_shadow_readiness(
        &self,
        candidate_id: &str,
    ) -> Result<Option<Value>, PromotionDecisionServiceError> {
        let evaluations = self.run_shadow_future(pg_list_shadow_evaluations_by_candidate(
            &self.pool,
            candidate_id,
            None,
            None,
            1,
        ))?;
        Ok(evaluations
            .into_iter()
            .next()
            .map(shadow_record_to_readiness))
    }
}

fn map_shadow_persistence_error(
    error: ShadowEvaluationPersistenceError,
) -> PromotionDecisionServiceError {
    match error.code {
        "shadow_evaluation_query_failed" => {
            PromotionDecisionServiceError::dependency_unavailable(error.message)
        }
        "shadow_evaluation_row_decode_failed" => {
            PromotionDecisionServiceError::state_unavailable(error.message)
        }
        _ => PromotionDecisionServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| PromotionDecisionValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn shadow_record_to_readiness(record: ShadowEvaluationRecord) -> Value {
    json!({
        "evaluation_id": record.evaluation_id,
        "evaluation_state": record.evaluation_state.as_str(),
        "reason_code": record.reason_code,
        "completed_at_utc": record.completed_at_utc,
        "correlation_id": record.correlation_id,
    })
}

#[derive(Debug, Default)]
pub struct InMemoryPromotionDecisionRepository {
    records: Mutex<BTreeMap<String, PromotionDecisionRecord>>,
}

impl PromotionDecisionRepositoryPort for InMemoryPromotionDecisionRepository {
    fn upsert(&self, record: PromotionDecisionRecord) -> Result<(), PromotionDecisionServiceError> {
        let canonical =
            canonicalize_promotion_decision_record(&record).map_err(map_contract_error)?;
        let mut records = self.records.lock().map_err(|_| {
            PromotionDecisionServiceError::persistence_unavailable(
                "in-memory promotion decision store lock poisoned",
            )
        })?;
        records.insert(canonical.decision_id.clone(), canonical);
        Ok(())
    }

    fn load(
        &self,
        decision_id: &str,
    ) -> Result<Option<PromotionDecisionRecord>, PromotionDecisionServiceError> {
        let normalized_decision_id = normalize_research_identifier(decision_id);
        let records = self.records.lock().map_err(|_| {
            PromotionDecisionServiceError::persistence_unavailable(
                "in-memory promotion decision store lock poisoned",
            )
        })?;
        records
            .get(&normalized_decision_id)
            .cloned()
            .map(|record| {
                canonicalize_promotion_decision_record(&record).map_err(map_contract_error)
            })
            .transpose()
    }

    fn list_by_candidate(
        &self,
        candidate_id: &str,
        decided_after_utc: Option<&str>,
        decided_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PromotionDecisionRecord>, PromotionDecisionServiceError> {
        if limit <= 0 {
            return Err(PromotionDecisionServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![PromotionDecisionValidationIssue {
                    field: "limit".to_string(),
                    code: PromotionDecisionReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_candidate_id = normalize_research_identifier(candidate_id);
        let decided_after = decided_after_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_promotion_utc_timestamp(value).map_err(map_contract_error))
            .transpose()?;
        let decided_before = decided_before_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_promotion_utc_timestamp(value).map_err(map_contract_error))
            .transpose()?;

        let records = self.records.lock().map_err(|_| {
            PromotionDecisionServiceError::persistence_unavailable(
                "in-memory promotion decision store lock poisoned",
            )
        })?;
        let mut decisions = records
            .values()
            .filter(|record| record.candidate_id == normalized_candidate_id)
            .cloned()
            .collect::<Vec<_>>();
        decisions.sort_by(|left, right| {
            right
                .decided_at_utc
                .cmp(&left.decided_at_utc)
                .then_with(|| left.decision_id.cmp(&right.decision_id))
        });

        let filtered = decisions
            .into_iter()
            .filter(|record| {
                let decided_at = parse_promotion_utc_timestamp(&record.decided_at_utc);
                if let Ok(decided_at) = decided_at {
                    let after_ok = decided_after
                        .map(|after| decided_at >= after)
                        .unwrap_or(true);
                    let before_ok = decided_before
                        .map(|before| decided_at < before)
                        .unwrap_or(true);
                    after_ok && before_ok
                } else {
                    false
                }
            })
            .take(limit as usize)
            .map(|record| {
                canonicalize_promotion_decision_record(&record).map_err(map_contract_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(filtered)
    }
}

#[derive(Debug, Clone)]
pub struct StaticValidationEvidencePort {
    run: ValidationWorkflowRunRecord,
    artifacts: Vec<ValidationWorkflowArtifactRecord>,
}

impl Default for StaticValidationEvidencePort {
    fn default() -> Self {
        let run = ValidationWorkflowRunRecord {
            run_id: "candidate::alpha-1::1712447000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            run_state: ValidationWorkflowRunState::Completed,
            reason_code: "validation_run_completed".to_string(),
            gate_evaluation: json!({
                "reason_code": "validation_gate_evaluation_allowed",
                "outcome": "allow",
            }),
            comparison_ready: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-validation-001".to_string(),
            started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:05:00Z".to_string()),
        };
        let diagnostics = domain::research::ValidationDiagnosticsPayload {
            out_of_sample_sharpe: 1.24,
            max_drawdown: -0.19,
            brier_score: Some(0.11),
            expected_calibration_error: None,
            overfit_indicator: 0.21,
            overfit_flag: false,
        };
        let artifacts = vec![
            ValidationWorkflowArtifactRecord {
                artifact_id: format!("{}::quality", run.run_id),
                run_id: run.run_id.clone(),
                candidate_id: run.candidate_id.clone(),
                stage: ValidationWorkflowStage::Quality,
                stage_index: 1,
                stage_outcome: domain::research::ValidationWorkflowStageOutcome::Passed,
                reason_code: "validation_run_stage_passed".to_string(),
                diagnostics: diagnostics.clone(),
                actor_id: "ops-1".to_string(),
                correlation_id: "corr-validation-001".to_string(),
                stage_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
                stage_completed_at_utc: "2026-04-07T00:01:00Z".to_string(),
            },
            ValidationWorkflowArtifactRecord {
                artifact_id: format!("{}::purged-cv", run.run_id),
                run_id: run.run_id.clone(),
                candidate_id: run.candidate_id.clone(),
                stage: ValidationWorkflowStage::PurgedCv,
                stage_index: 3,
                stage_outcome: domain::research::ValidationWorkflowStageOutcome::Passed,
                reason_code: "validation_run_stage_passed".to_string(),
                diagnostics: diagnostics.clone(),
                actor_id: "ops-1".to_string(),
                correlation_id: "corr-validation-001".to_string(),
                stage_started_at_utc: "2026-04-07T00:02:00Z".to_string(),
                stage_completed_at_utc: "2026-04-07T00:03:00Z".to_string(),
            },
            ValidationWorkflowArtifactRecord {
                artifact_id: format!("{}::cpcv", run.run_id),
                run_id: run.run_id.clone(),
                candidate_id: run.candidate_id.clone(),
                stage: ValidationWorkflowStage::Cpcv,
                stage_index: 4,
                stage_outcome: domain::research::ValidationWorkflowStageOutcome::Passed,
                reason_code: "validation_run_stage_passed".to_string(),
                diagnostics: diagnostics.clone(),
                actor_id: "ops-1".to_string(),
                correlation_id: "corr-validation-001".to_string(),
                stage_started_at_utc: "2026-04-07T00:03:00Z".to_string(),
                stage_completed_at_utc: "2026-04-07T00:04:00Z".to_string(),
            },
            ValidationWorkflowArtifactRecord {
                artifact_id: format!("{}::overfit", run.run_id),
                run_id: run.run_id.clone(),
                candidate_id: run.candidate_id.clone(),
                stage: ValidationWorkflowStage::OverfitDiagnostics,
                stage_index: 5,
                stage_outcome: domain::research::ValidationWorkflowStageOutcome::Passed,
                reason_code: "validation_run_stage_passed".to_string(),
                diagnostics,
                actor_id: "ops-1".to_string(),
                correlation_id: "corr-validation-001".to_string(),
                stage_started_at_utc: "2026-04-07T00:04:00Z".to_string(),
                stage_completed_at_utc: "2026-04-07T00:05:00Z".to_string(),
            },
        ];
        Self { run, artifacts }
    }
}

impl ValidationEvidencePort for StaticValidationEvidencePort {
    fn load_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, PromotionDecisionServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        Ok((self.run.run_id == normalized_run_id).then_some(self.run.clone()))
    }

    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, PromotionDecisionServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        Ok(self
            .artifacts
            .iter()
            .filter(|artifact| artifact.run_id == normalized_run_id)
            .cloned()
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct StaticShadowEvidencePort;

impl ShadowEvidencePort for StaticShadowEvidencePort {
    fn read_latest_shadow_readiness(
        &self,
        _candidate_id: &str,
    ) -> Result<Option<Value>, PromotionDecisionServiceError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::gate_policies::{
        EvaluateValidationGatePoliciesInput, ListValidationGatePoliciesInput,
        ReadValidationGatePolicyInput, UpsertValidationGatePolicyInput,
        ValidationGateEvaluationEvidence, ValidationGateEvaluationResultEvidence,
        ValidationGatePolicyEvidence,
    };

    #[derive(Debug, Default, Clone)]
    struct StubValidationEvidencePort {
        run: Option<ValidationWorkflowRunRecord>,
        artifacts: Vec<ValidationWorkflowArtifactRecord>,
    }

    impl StubValidationEvidencePort {
        fn with_defaults() -> Self {
            let defaults = StaticValidationEvidencePort::default();
            Self {
                run: Some(defaults.run),
                artifacts: defaults.artifacts,
            }
        }
    }

    impl ValidationEvidencePort for StubValidationEvidencePort {
        fn load_validation_run(
            &self,
            _run_id: &str,
        ) -> Result<Option<ValidationWorkflowRunRecord>, PromotionDecisionServiceError> {
            Ok(self.run.clone())
        }

        fn list_validation_artifacts_by_run(
            &self,
            _run_id: &str,
        ) -> Result<Vec<ValidationWorkflowArtifactRecord>, PromotionDecisionServiceError> {
            Ok(self.artifacts.clone())
        }
    }

    #[derive(Debug, Clone)]
    struct StubShadowEvidencePort {
        value: Option<Value>,
        error: Option<PromotionDecisionServiceError>,
    }

    impl Default for StubShadowEvidencePort {
        fn default() -> Self {
            Self {
                value: Some(json!({
                    "evaluation_id": "candidate::alpha-1::1712447999",
                    "reason_code": "shadow_evaluation_completed"
                })),
                error: None,
            }
        }
    }

    impl ShadowEvidencePort for StubShadowEvidencePort {
        fn read_latest_shadow_readiness(
            &self,
            _candidate_id: &str,
        ) -> Result<Option<Value>, PromotionDecisionServiceError> {
            if let Some(error) = self.error.clone() {
                return Err(error);
            }
            Ok(self.value.clone())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct StubValidationGateOrchestrator {
        evaluate_error: Option<ValidationGatePolicyServiceError>,
    }

    impl ValidationGatePolicyOrchestrator for StubValidationGateOrchestrator {
        fn upsert_validation_gate_policy(
            &self,
            _input: UpsertValidationGatePolicyInput,
        ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError> {
            Err(ValidationGatePolicyServiceError {
                code: ValidationGateReasonCode::DependencyUnavailable.code(),
                message: "unused in tests".to_string(),
                field_errors: Vec::new(),
                failed_gate_ids: Vec::new(),
            })
        }

        fn read_validation_gate_policy(
            &self,
            _input: ReadValidationGatePolicyInput,
        ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError> {
            Err(ValidationGatePolicyServiceError {
                code: ValidationGateReasonCode::DependencyUnavailable.code(),
                message: "unused in tests".to_string(),
                field_errors: Vec::new(),
                failed_gate_ids: Vec::new(),
            })
        }

        fn list_validation_gate_policies(
            &self,
            _input: ListValidationGatePoliciesInput,
        ) -> Result<Vec<ValidationGatePolicyEvidence>, ValidationGatePolicyServiceError> {
            Err(ValidationGatePolicyServiceError {
                code: ValidationGateReasonCode::DependencyUnavailable.code(),
                message: "unused in tests".to_string(),
                field_errors: Vec::new(),
                failed_gate_ids: Vec::new(),
            })
        }

        fn evaluate_validation_gates(
            &self,
            input: EvaluateValidationGatePoliciesInput,
        ) -> Result<ValidationGateEvaluationEvidence, ValidationGatePolicyServiceError> {
            if let Some(error) = self.evaluate_error.clone() {
                return Err(error);
            }
            Ok(ValidationGateEvaluationEvidence {
                candidate_id: normalize_research_identifier(&input.candidate_id),
                stage: "promotion".to_string(),
                outcome: "allow".to_string(),
                reason_code: "validation_gate_evaluation_allowed".to_string(),
                failed_gate_ids: Vec::new(),
                gate_results: vec![ValidationGateEvaluationResultEvidence {
                    policy_key: "fr43::forward-bias::primary".to_string(),
                    gate_type: "forward_bias".to_string(),
                    metric_key: "out_of_sample_sharpe".to_string(),
                    comparator: "gte".to_string(),
                    threshold_value: 1.0,
                    observed_value: Some(1.2),
                    passed: true,
                    reason_code: "validation_gate_evaluation_allowed".to_string(),
                }],
                actor_id: input.actor_id,
                correlation_id: input.correlation_id,
                evaluated_at_utc: input.evaluated_at_utc,
            })
        }
    }

    fn sample_start_input() -> StartPromotionDecisionInput {
        StartPromotionDecisionInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            lifecycle_action: "promote".to_string(),
            observed_metrics: json!({
                "out_of_sample_sharpe": 1.28,
                "max_drawdown": -0.17
            }),
            thresholds: vec![
                PromotionThresholdDefinition {
                    metric_key: "out_of_sample_sharpe".to_string(),
                    comparator: domain::research::ValidationGateComparator::Gte,
                    threshold_value: 1.0,
                },
                PromotionThresholdDefinition {
                    metric_key: "max_drawdown".to_string(),
                    comparator: domain::research::ValidationGateComparator::Gte,
                    threshold_value: -0.2,
                },
            ],
            evidence_packet: json!({
                "data_quality_report": { "artifact_id": "quality::001" },
                "purged_cpcv_results": { "artifact_id": "cpcv::001" },
                "calibration_report": { "artifact_id": "calibration::001" },
                "counterfactual_replay_summary": { "status": "deferred_to_story_6_6" }
            }),
            shadow_readiness: None,
            approval_request_id: Some("request::promotion-001".to_string()),
            approval_reference: Some("approval::promotion-001".to_string()),
            correlation_id: "corr-promotion-001".to_string(),
            requested_at_utc: "2026-04-07T00:10:00Z".to_string(),
        }
    }

    fn service_with_ports(
        validation_evidence: Arc<dyn ValidationEvidencePort>,
        gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
        shadow_evidence: Arc<dyn ShadowEvidencePort>,
    ) -> PromotionDecisionService {
        PromotionDecisionService::new(
            Arc::new(InMemoryPromotionDecisionRepository::default()),
            validation_evidence,
            gate_orchestrator,
            shadow_evidence,
            Arc::new(CounterfactualReplayService::in_memory()),
        )
    }

    #[test]
    fn promotion_decision_start_backfills_counterfactual_replay_summary_when_missing() {
        let service = service_with_ports(
            Arc::new(StubValidationEvidencePort::with_defaults()),
            Arc::new(StubValidationGateOrchestrator::default()),
            Arc::new(StubShadowEvidencePort::default()),
        );
        let mut input = sample_start_input();
        input.evidence_packet = json!({
            "data_quality_report": { "artifact_id": "quality::001" },
            "purged_cpcv_results": { "artifact_id": "cpcv::001" },
            "calibration_report": { "artifact_id": "calibration::001" }
        });

        let evidence = service
            .start_promotion_decision(input)
            .expect("decision should persist");
        assert_eq!(
            evidence.decision.decision_state,
            PromotionDecisionState::Allowed
        );
        assert_eq!(
            evidence.decision.reason_code,
            PromotionDecisionReasonCode::DecisionAllowed.code()
        );
        assert!(
            !evidence
                .decision
                .missing_evidence_fields
                .contains(&"counterfactual_replay_summary".to_string())
        );
        let summary = evidence
            .decision
            .evidence_packet
            .get("counterfactual_replay_summary")
            .and_then(Value::as_object)
            .expect("replay summary should be materialized");
        assert_eq!(
            summary.get("gate_outcome").and_then(Value::as_str),
            Some("allow")
        );
        assert!(
            summary
                .get("run_id")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        );
    }

    #[test]
    fn promotion_decision_start_denies_when_validation_run_is_missing() {
        let mut validation_port = StubValidationEvidencePort::with_defaults();
        validation_port.run = None;
        let service = service_with_ports(
            Arc::new(validation_port),
            Arc::new(StubValidationGateOrchestrator::default()),
            Arc::new(StubShadowEvidencePort::default()),
        );

        let evidence = service
            .start_promotion_decision(sample_start_input())
            .expect("missing validation run should persist as denied");
        assert_eq!(
            evidence.decision.decision_state,
            PromotionDecisionState::Denied
        );
        assert_eq!(
            evidence.decision.reason_code,
            PromotionDecisionReasonCode::MissingEvidence.code()
        );
        assert!(
            evidence
                .decision
                .missing_evidence_fields
                .contains(&"validation_run_id".to_string())
        );
    }

    #[test]
    fn promotion_decision_start_allows_when_all_gates_pass() {
        let service = service_with_ports(
            Arc::new(StubValidationEvidencePort::with_defaults()),
            Arc::new(StubValidationGateOrchestrator::default()),
            Arc::new(StubShadowEvidencePort::default()),
        );

        let evidence = service
            .start_promotion_decision(sample_start_input())
            .expect("allow decision should succeed");
        assert_eq!(
            evidence.decision.decision_state,
            PromotionDecisionState::Allowed
        );
        assert_eq!(
            evidence.decision.reason_code,
            PromotionDecisionReasonCode::DecisionAllowed.code()
        );
        assert!(evidence.decision.missing_evidence_fields.is_empty());
    }

    #[test]
    fn promotion_decision_start_denies_when_counterfactual_replay_gate_fails() {
        let replay_orchestrator = CounterfactualReplayService::in_memory()
            .with_shadow_evidence_port(Arc::new(
                crate::promotion::counterfactual_replay::StaticShadowEvidencePort {
                    slippage_bps: 120.0,
                    simulated_fill_price: 0.43,
                },
            ));
        let service = service_with_ports(
            Arc::new(StubValidationEvidencePort::with_defaults()),
            Arc::new(StubValidationGateOrchestrator::default()),
            Arc::new(StubShadowEvidencePort::default()),
        )
        .with_counterfactual_replay_orchestrator(Arc::new(replay_orchestrator));

        let evidence = service
            .start_promotion_decision(sample_start_input())
            .expect("replay denial should persist as denied decision");
        assert_eq!(
            evidence.decision.decision_state,
            PromotionDecisionState::Denied
        );
        assert_eq!(
            evidence.decision.reason_code,
            PromotionDecisionReasonCode::ReplayGateDenied.code()
        );
    }

    #[test]
    fn promotion_decision_start_denies_when_gate_fails() {
        let service = service_with_ports(
            Arc::new(StubValidationEvidencePort::with_defaults()),
            Arc::new(StubValidationGateOrchestrator {
                evaluate_error: Some(ValidationGatePolicyServiceError {
                    code: ValidationGateReasonCode::GateFailed.code(),
                    message: "promotion gate failed".to_string(),
                    field_errors: Vec::new(),
                    failed_gate_ids: vec!["fr43::forward-bias::primary".to_string()],
                }),
            }),
            Arc::new(StubShadowEvidencePort::default()),
        );

        let evidence = service
            .start_promotion_decision(sample_start_input())
            .expect("gate denial should persist as denied decision");
        assert_eq!(
            evidence.decision.decision_state,
            PromotionDecisionState::Denied
        );
        assert_eq!(
            evidence.decision.reason_code,
            PromotionDecisionReasonCode::GateDenied.code()
        );
    }

    #[test]
    fn promotion_decision_start_fails_closed_when_shadow_dependency_is_unavailable() {
        let service = service_with_ports(
            Arc::new(StubValidationEvidencePort::with_defaults()),
            Arc::new(StubValidationGateOrchestrator::default()),
            Arc::new(StubShadowEvidencePort {
                value: None,
                error: Some(PromotionDecisionServiceError::dependency_unavailable(
                    "shadow evidence port unavailable",
                )),
            }),
        );

        let error = service
            .start_promotion_decision(sample_start_input())
            .expect_err("shadow dependency errors must fail closed");
        assert_eq!(
            error.code,
            PromotionDecisionReasonCode::DependencyUnavailable.code()
        );
    }

    #[test]
    fn promotion_decision_list_orders_results_deterministically() {
        let service = service_with_ports(
            Arc::new(StubValidationEvidencePort::with_defaults()),
            Arc::new(StubValidationGateOrchestrator::default()),
            Arc::new(StubShadowEvidencePort::default()),
        );
        let mut first = sample_start_input();
        first.requested_at_utc = "2026-04-07T00:10:00Z".to_string();
        service
            .start_promotion_decision(first)
            .expect("first decision should persist");

        let mut second = sample_start_input();
        second.requested_at_utc = "2026-04-07T00:12:00Z".to_string();
        second.correlation_id = "corr-promotion-002".to_string();
        second.approval_request_id = Some("request::promotion-002".to_string());
        second.approval_reference = Some("approval::promotion-002".to_string());
        service
            .start_promotion_decision(second)
            .expect("second decision should persist");

        let listed = service
            .list_promotion_decisions(ListPromotionDecisionsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                limit: Some(10),
                decided_after_utc: None,
                decided_before_utc: None,
                correlation_id: "corr-promotion-list-001".to_string(),
                queried_at_utc: "2026-04-07T00:13:00Z".to_string(),
            })
            .expect("list should succeed");
        assert_eq!(listed.len(), 2);
        assert!(listed[0].decided_at_utc >= listed[1].decided_at_utc);
    }
}
