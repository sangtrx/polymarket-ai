use domain::research::{
    ShadowEvaluationContractError, ShadowEvaluationReasonCode, ShadowEvaluationRecord,
    ShadowEvaluationState, ShadowEvaluationValidationIssue, ShadowSimulationDecisionSide,
    ShadowSimulationOutcome, ValidationDiagnosticsPayload, ValidationWorkflowArtifactRecord,
    ValidationWorkflowReasonCode, ValidationWorkflowRunRecord, ValidationWorkflowRunState,
    ValidationWorkflowStage, ValidationWorkflowStageOutcome, canonicalize_shadow_evaluation_record,
    compose_shadow_evaluation_id, normalize_research_identifier, parse_shadow_utc_timestamp,
};
use persistence::postgres::shadow_evaluations::{
    ShadowEvaluationPersistenceError,
    list_shadow_evaluations_by_candidate as pg_list_shadow_evaluations_by_candidate,
    load_shadow_evaluation as pg_load_shadow_evaluation,
    upsert_shadow_evaluation as pg_upsert_shadow_evaluation,
};
use persistence::postgres::validation_artifacts::{
    ValidationArtifactPersistenceError,
    list_validation_artifacts_by_run as pg_list_validation_artifacts_by_run,
};
use persistence::postgres::validation_runs::{
    ValidationRunPersistenceError, load_validation_run as pg_load_validation_run,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};
use time::{Duration, format_description::well_known::Rfc3339};

const SHADOW_SIMULATION_READ_ONLY_ENFORCED_CODE: &str = "shadow_simulation_read_only_enforced";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShadowModeServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ShadowEvaluationValidationIssue>,
}

impl ShadowModeServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ShadowEvaluationValidationIssue>,
    ) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: ShadowEvaluationReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for shadow evaluation mutations".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: ShadowEvaluationReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for shadow evaluation reads".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn evaluation_not_found(evaluation_id: &str) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::EvaluationNotFound.code(),
            message: format!("shadow evaluation `{evaluation_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn validation_run_ineligible(message: impl Into<String>) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::ValidationRunIneligible.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ShadowModeServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ShadowModeServiceError {}

#[derive(Debug, Clone)]
pub struct StartShadowEvaluationInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub validation_run_id: String,
    pub market_context: Value,
    pub signal_decisions: Value,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadShadowEvaluationInput {
    pub actor_id: String,
    pub actor_role: String,
    pub evaluation_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListShadowEvaluationsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub limit: Option<i64>,
    pub started_after_utc: Option<String>,
    pub started_before_utc: Option<String>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ShadowEvaluationEvidence {
    pub evaluation: ShadowEvaluationRecord,
    pub reason_code: String,
}

pub trait ShadowModeOrchestrator: Send + Sync {
    fn start_shadow_evaluation(
        &self,
        input: StartShadowEvaluationInput,
    ) -> Result<ShadowEvaluationEvidence, ShadowModeServiceError>;

    fn read_shadow_evaluation(
        &self,
        input: ReadShadowEvaluationInput,
    ) -> Result<ShadowEvaluationEvidence, ShadowModeServiceError>;

    fn list_shadow_evaluations(
        &self,
        input: ListShadowEvaluationsInput,
    ) -> Result<Vec<ShadowEvaluationRecord>, ShadowModeServiceError>;
}

pub trait ShadowEvaluationRepositoryPort: Send + Sync {
    fn upsert(&self, record: ShadowEvaluationRecord) -> Result<(), ShadowModeServiceError>;
    fn load(
        &self,
        evaluation_id: &str,
    ) -> Result<Option<ShadowEvaluationRecord>, ShadowModeServiceError>;
    fn list_by_candidate(
        &self,
        candidate_id: &str,
        started_after_utc: Option<&str>,
        started_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ShadowEvaluationRecord>, ShadowModeServiceError>;
}

pub trait ValidationEvidencePort: Send + Sync {
    fn load_completed_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, ShadowModeServiceError>;
    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ShadowModeServiceError>;
}

pub trait MarketContextPort: Send + Sync {
    fn load_market_context(
        &self,
        candidate_id: &str,
        requested_at_utc: &str,
        proposed_market_context: &Value,
    ) -> Result<Value, ShadowModeServiceError>;
}

pub trait ShadowSimulationPort: Send + Sync {
    fn simulate_shadow_outcomes(
        &self,
        candidate_id: &str,
        market_context: &Value,
        signal_decisions: &Value,
        requested_at_utc: &str,
    ) -> Result<Vec<ShadowSimulationOutcome>, ShadowModeServiceError>;
}

#[derive(Clone)]
pub struct ShadowModeService {
    repository: Arc<dyn ShadowEvaluationRepositoryPort>,
    validation_evidence: Arc<dyn ValidationEvidencePort>,
    market_context_port: Arc<dyn MarketContextPort>,
    simulation_port: Arc<dyn ShadowSimulationPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl ShadowModeService {
    pub fn new(
        repository: Arc<dyn ShadowEvaluationRepositoryPort>,
        validation_evidence: Arc<dyn ValidationEvidencePort>,
        market_context_port: Arc<dyn MarketContextPort>,
        simulation_port: Arc<dyn ShadowSimulationPort>,
    ) -> Self {
        Self {
            repository,
            validation_evidence,
            market_context_port,
            simulation_port,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryShadowEvaluationRepository::default()),
            Arc::new(StaticValidationEvidencePort::default()),
            Arc::new(DeterministicMarketContextPort),
            Arc::new(DeterministicShadowSimulationPort),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresShadowEvaluationRepository::new(pool.clone())),
            Arc::new(PostgresValidationEvidencePort::new(pool)),
            Arc::new(DeterministicMarketContextPort),
            Arc::new(DeterministicShadowSimulationPort),
        )
    }

    pub fn with_validation_evidence_port(
        mut self,
        validation_evidence: Arc<dyn ValidationEvidencePort>,
    ) -> Self {
        self.validation_evidence = validation_evidence;
        self
    }

    pub fn with_market_context_port(
        mut self,
        market_context_port: Arc<dyn MarketContextPort>,
    ) -> Self {
        self.market_context_port = market_context_port;
        self
    }

    pub fn with_simulation_port(mut self, simulation_port: Arc<dyn ShadowSimulationPort>) -> Self {
        self.simulation_port = simulation_port;
        self
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, ShadowModeServiceError> {
        self.operation_lock.lock().map_err(|_| {
            ShadowModeServiceError::persistence_unavailable(
                "shadow evaluation operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for ShadowModeService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ShadowModeOrchestrator for ShadowModeService {
    fn start_shadow_evaluation(
        &self,
        input: StartShadowEvaluationInput,
    ) -> Result<ShadowEvaluationEvidence, ShadowModeServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "candidate_id",
            &input.candidate_id,
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "validation_run_id",
            &input.validation_run_id,
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "requested_at_utc",
            &input.requested_at_utc,
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "requested_at_utc",
            &input.requested_at_utc,
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;

        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        let normalized_validation_run_id = normalize_research_identifier(&input.validation_run_id);
        let _lock = self.lock_operations().inspect_err(|error| {
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
        })?;

        let Some(validation_run) = self
            .validation_evidence
            .load_completed_validation_run(&normalized_validation_run_id)
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?
        else {
            let error = ShadowModeServiceError::validation_run_ineligible(
                "validation_run_id must reference a completed Story 6.3 validation run",
            );
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        };
        if validation_run.run_state != ValidationWorkflowRunState::Completed
            || validation_run.candidate_id != normalized_candidate_id
        {
            let error = ShadowModeServiceError::validation_run_ineligible(
                "validation_run_id must match candidate_id and be completed before shadow evaluation",
            );
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }

        let artifacts = self
            .validation_evidence
            .list_validation_artifacts_by_run(&validation_run.run_id)
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;
        if artifacts.is_empty() {
            let error = ShadowModeServiceError::validation_run_ineligible(
                "validation_run_id must include diagnostics artifacts to be shadow-eligible",
            );
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }

        let market_context = self
            .market_context_port
            .load_market_context(
                &normalized_candidate_id,
                &input.requested_at_utc,
                &input.market_context,
            )
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;
        let simulation_outcomes = self
            .simulation_port
            .simulate_shadow_outcomes(
                &normalized_candidate_id,
                &market_context,
                &input.signal_decisions,
                &input.requested_at_utc,
            )
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;

        if simulation_outcomes.is_empty() {
            let error =
                ShadowModeServiceError::state_unavailable("simulation_outcomes cannot be empty");
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }

        let evaluation_id =
            compose_shadow_evaluation_id(&normalized_candidate_id, &input.requested_at_utc)
                .map_err(map_contract_error)
                .inspect_err(|error| {
                    emit_shadow_evaluation_telemetry(
                        "shadow_evaluation_start_v1",
                        "shadow_evaluation_start",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        None,
                        error.code,
                        &input.correlation_id,
                        &input.requested_at_utc,
                    );
                })?;

        let requested_at = parse_shadow_utc_timestamp(&input.requested_at_utc)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    Some(&evaluation_id),
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;
        let completed_at = simulation_outcomes
            .last()
            .map(|outcome| parse_shadow_utc_timestamp(&outcome.simulated_at_utc))
            .transpose()
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    Some(&evaluation_id),
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?
            .unwrap_or(requested_at);
        let completed_at_utc = completed_at
            .max(requested_at)
            .format(&Rfc3339)
            .expect("shadow evaluation completion timestamp should format");
        let evaluation = canonicalize_shadow_evaluation_record(&ShadowEvaluationRecord {
            evaluation_id: evaluation_id.clone(),
            candidate_id: normalized_candidate_id.clone(),
            validation_run_id: normalized_validation_run_id,
            evaluation_state: ShadowEvaluationState::Completed,
            reason_code: ShadowEvaluationReasonCode::EvaluationCompleted
                .code()
                .to_string(),
            market_context,
            signal_decisions: input.signal_decisions.clone(),
            simulation_outcomes,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            started_at_utc: input.requested_at_utc.clone(),
            completed_at_utc: Some(completed_at_utc.clone()),
        })
        .map_err(map_contract_error)
        .inspect_err(|error| {
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_start_v1",
                "shadow_evaluation_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                Some(&evaluation_id),
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
        })?;
        self.repository
            .upsert(evaluation.clone())
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_start_v1",
                    "shadow_evaluation_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    Some(&evaluation_id),
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;

        emit_shadow_evaluation_telemetry(
            "shadow_evaluation_start_v1",
            "shadow_evaluation_start",
            "allow",
            &input.actor_id,
            &normalized_candidate_id,
            Some(&evaluation_id),
            ShadowEvaluationReasonCode::EvaluationCompleted.code(),
            &input.correlation_id,
            &completed_at_utc,
        );

        Ok(ShadowEvaluationEvidence {
            evaluation,
            reason_code: ShadowEvaluationReasonCode::EvaluationStarted
                .code()
                .to_string(),
        })
    }

    fn read_shadow_evaluation(
        &self,
        input: ReadShadowEvaluationInput,
    ) -> Result<ShadowEvaluationEvidence, ShadowModeServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_read_v1",
                "shadow_evaluation_read",
                "deny",
                &input.actor_id,
                &input.evaluation_id,
                Some(&input.evaluation_id),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "shadow_evaluation_read_v1",
            "shadow_evaluation_read",
            &input.actor_id,
            &input.evaluation_id,
            Some(&input.evaluation_id),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "evaluation_id",
            &input.evaluation_id,
            "shadow_evaluation_read_v1",
            "shadow_evaluation_read",
            &input.actor_id,
            &input.evaluation_id,
            Some(&input.evaluation_id),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "shadow_evaluation_read_v1",
            "shadow_evaluation_read",
            &input.actor_id,
            &input.evaluation_id,
            Some(&input.evaluation_id),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "shadow_evaluation_read_v1",
            "shadow_evaluation_read",
            &input.actor_id,
            &input.evaluation_id,
            Some(&input.evaluation_id),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "shadow_evaluation_read_v1",
            "shadow_evaluation_read",
            &input.actor_id,
            &input.evaluation_id,
            Some(&input.evaluation_id),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;

        let normalized_evaluation_id = normalize_research_identifier(&input.evaluation_id);
        let Some(evaluation) = self
            .repository
            .load(&normalized_evaluation_id)
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_read_v1",
                    "shadow_evaluation_read",
                    "deny",
                    &input.actor_id,
                    &normalized_evaluation_id,
                    Some(&normalized_evaluation_id),
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?
        else {
            let error = ShadowModeServiceError::evaluation_not_found(&normalized_evaluation_id);
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_read_v1",
                "shadow_evaluation_read",
                "deny",
                &input.actor_id,
                &normalized_evaluation_id,
                Some(&normalized_evaluation_id),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        };

        emit_shadow_evaluation_telemetry(
            "shadow_evaluation_read_v1",
            "shadow_evaluation_read",
            "allow",
            &input.actor_id,
            &evaluation.candidate_id,
            Some(&evaluation.evaluation_id),
            ShadowEvaluationReasonCode::EvaluationRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(ShadowEvaluationEvidence {
            evaluation,
            reason_code: ShadowEvaluationReasonCode::EvaluationRead
                .code()
                .to_string(),
        })
    }

    fn list_shadow_evaluations(
        &self,
        input: ListShadowEvaluationsInput,
    ) -> Result<Vec<ShadowEvaluationRecord>, ShadowModeServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_list_v1",
                "shadow_evaluation_list",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "shadow_evaluation_list_v1",
            "shadow_evaluation_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "candidate_id",
            &input.candidate_id,
            "shadow_evaluation_list_v1",
            "shadow_evaluation_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "shadow_evaluation_list_v1",
            "shadow_evaluation_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "shadow_evaluation_list_v1",
            "shadow_evaluation_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "shadow_evaluation_list_v1",
            "shadow_evaluation_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            let error = ShadowModeServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![ShadowEvaluationValidationIssue {
                    field: "candidate_id".to_string(),
                    code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            );
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_list_v1",
                "shadow_evaluation_list",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }

        if let Some(limit) = input.limit
            && limit <= 0
        {
            let error = ShadowModeServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![ShadowEvaluationValidationIssue {
                    field: "limit".to_string(),
                    code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            );
            emit_shadow_evaluation_telemetry(
                "shadow_evaluation_list_v1",
                "shadow_evaluation_list",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        let normalized_started_after =
            normalize_optional_timestamp("started_after_utc", input.started_after_utc.as_deref())
                .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_list_v1",
                    "shadow_evaluation_list",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;
        let normalized_started_before =
            normalize_optional_timestamp("started_before_utc", input.started_before_utc.as_deref())
                .inspect_err(|error| {
                    emit_shadow_evaluation_telemetry(
                        "shadow_evaluation_list_v1",
                        "shadow_evaluation_list",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        None,
                        error.code,
                        &input.correlation_id,
                        &input.queried_at_utc,
                    );
                })?;
        if let (Some(started_after), Some(started_before)) = (
            normalized_started_after.as_deref(),
            normalized_started_before.as_deref(),
        ) {
            let started_after_ts = parse_shadow_utc_timestamp(started_after)
                .map_err(map_contract_error)
                .inspect_err(|error| {
                    emit_shadow_evaluation_telemetry(
                        "shadow_evaluation_list_v1",
                        "shadow_evaluation_list",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        None,
                        error.code,
                        &input.correlation_id,
                        &input.queried_at_utc,
                    );
                })?;
            let started_before_ts = parse_shadow_utc_timestamp(started_before)
                .map_err(map_contract_error)
                .inspect_err(|error| {
                    emit_shadow_evaluation_telemetry(
                        "shadow_evaluation_list_v1",
                        "shadow_evaluation_list",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        None,
                        error.code,
                        &input.correlation_id,
                        &input.queried_at_utc,
                    );
                })?;
            if started_before_ts <= started_after_ts {
                let error = ShadowModeServiceError::invalid_payload(
                    "started_before_utc must be greater than started_after_utc",
                    vec![ShadowEvaluationValidationIssue {
                        field: "started_before_utc".to_string(),
                        code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                        message: "started_before_utc must be greater than started_after_utc"
                            .to_string(),
                    }],
                );
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_list_v1",
                    "shadow_evaluation_list",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
                return Err(error);
            }
        }

        let limit = input.limit.unwrap_or(25).clamp(1, 200);
        let evaluations = self
            .repository
            .list_by_candidate(
                &normalized_candidate_id,
                normalized_started_after.as_deref(),
                normalized_started_before.as_deref(),
                limit,
            )
            .inspect_err(|error| {
                emit_shadow_evaluation_telemetry(
                    "shadow_evaluation_list_v1",
                    "shadow_evaluation_list",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;

        emit_shadow_evaluation_telemetry(
            "shadow_evaluation_list_v1",
            "shadow_evaluation_list",
            "allow",
            &input.actor_id,
            &normalized_candidate_id,
            None,
            ShadowEvaluationReasonCode::EvaluationListed.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(evaluations)
    }
}

fn map_contract_error(error: ShadowEvaluationContractError) -> ShadowModeServiceError {
    ShadowModeServiceError::invalid_payload(error.message, error.field_errors)
}

fn map_shadow_persistence_error(error: ShadowEvaluationPersistenceError) -> ShadowModeServiceError {
    match error.code {
        "shadow_evaluation_query_failed" | "shadow_evaluation_row_decode_failed" => {
            ShadowModeServiceError::persistence_unavailable(error.message)
        }
        _ => ShadowModeServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

fn map_validation_run_persistence_error(
    error: ValidationRunPersistenceError,
) -> ShadowModeServiceError {
    match error.code {
        "validation_run_query_failed" | "validation_run_row_decode_failed" => {
            ShadowModeServiceError::persistence_unavailable(error.message)
        }
        _ => ShadowModeServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| ShadowEvaluationValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn map_validation_artifact_persistence_error(
    error: ValidationArtifactPersistenceError,
) -> ShadowModeServiceError {
    match error.code {
        "validation_artifact_query_failed" | "validation_artifact_row_decode_failed" => {
            ShadowModeServiceError::persistence_unavailable(error.message)
        }
        _ => ShadowModeServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| ShadowEvaluationValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn validate_mutation_role(role: &str) -> Result<(), ShadowModeServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ShadowModeServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), ShadowModeServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ShadowModeServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ShadowModeServiceError> {
    if value.trim().is_empty() {
        return Err(ShadowModeServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![ShadowEvaluationValidationIssue {
                field: field.to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_non_empty_with_telemetry(
    field: &str,
    value: &str,
    event_name: &'static str,
    action: &'static str,
    actor_id: &str,
    candidate_id: &str,
    evaluation_id: Option<&str>,
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), ShadowModeServiceError> {
    validate_non_empty(field, value).inspect_err(|error| {
        emit_shadow_evaluation_telemetry(
            event_name,
            action,
            "deny",
            actor_id,
            candidate_id,
            evaluation_id,
            error.code,
            correlation_id,
            timestamp_utc,
        );
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_utc_timestamp_with_telemetry(
    field: &str,
    value: &str,
    event_name: &'static str,
    action: &'static str,
    actor_id: &str,
    candidate_id: &str,
    evaluation_id: Option<&str>,
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), ShadowModeServiceError> {
    parse_shadow_utc_timestamp(value)
        .map_err(|error| {
            ShadowModeServiceError::invalid_payload(
                error.message,
                vec![ShadowEvaluationValidationIssue {
                    field: field.to_string(),
                    code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                    message: format!("{field} must be RFC3339 UTC"),
                }],
            )
        })
        .inspect_err(|error| {
            emit_shadow_evaluation_telemetry(
                event_name,
                action,
                "deny",
                actor_id,
                candidate_id,
                evaluation_id,
                error.code,
                correlation_id,
                timestamp_utc,
            );
        })?;
    Ok(())
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, ShadowModeServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_shadow_utc_timestamp(trimmed).map_err(|_| {
        ShadowModeServiceError::invalid_payload(
            format!("{field} must be RFC3339 UTC"),
            vec![ShadowEvaluationValidationIssue {
                field: field.to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: format!("{field} must be RFC3339 UTC"),
            }],
        )
    })?;
    Ok(Some(trimmed.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn emit_shadow_evaluation_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    candidate_id: &str,
    evaluation_id: Option<&str>,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = ShadowEvaluationTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        candidate_id,
        evaluation_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(ShadowEvaluationSecuritySignal {
                name: "shadow_evaluation_denied_v1",
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
        serde_json::to_string(&event).expect("shadow evaluation telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct ShadowEvaluationTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    candidate_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation_id: Option<&'a str>,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<ShadowEvaluationSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct ShadowEvaluationSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresShadowEvaluationRepository {
    pool: PgPool,
}

impl PostgresShadowEvaluationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, ShadowModeServiceError>
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
                    ShadowModeServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_shadow_persistence_error),
        }
    }
}

impl ShadowEvaluationRepositoryPort for PostgresShadowEvaluationRepository {
    fn upsert(&self, record: ShadowEvaluationRecord) -> Result<(), ShadowModeServiceError> {
        self.run_future(pg_upsert_shadow_evaluation(&self.pool, &record))
    }

    fn load(
        &self,
        evaluation_id: &str,
    ) -> Result<Option<ShadowEvaluationRecord>, ShadowModeServiceError> {
        self.run_future(pg_load_shadow_evaluation(&self.pool, evaluation_id))
    }

    fn list_by_candidate(
        &self,
        candidate_id: &str,
        started_after_utc: Option<&str>,
        started_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ShadowEvaluationRecord>, ShadowModeServiceError> {
        self.run_future(pg_list_shadow_evaluations_by_candidate(
            &self.pool,
            candidate_id,
            started_after_utc,
            started_before_utc,
            limit,
        ))
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

    fn run_validation_run_future<F, T>(&self, future: F) -> Result<T, ShadowModeServiceError>
    where
        F: Future<Output = Result<T, ValidationRunPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_validation_run_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ShadowModeServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_validation_run_persistence_error),
        }
    }

    fn run_validation_artifact_future<F, T>(&self, future: F) -> Result<T, ShadowModeServiceError>
    where
        F: Future<Output = Result<T, ValidationArtifactPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_validation_artifact_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ShadowModeServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_validation_artifact_persistence_error),
        }
    }
}

impl ValidationEvidencePort for PostgresValidationEvidencePort {
    fn load_completed_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, ShadowModeServiceError> {
        self.run_validation_run_future(pg_load_validation_run(&self.pool, run_id))
    }

    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ShadowModeServiceError> {
        self.run_validation_artifact_future(pg_list_validation_artifacts_by_run(&self.pool, run_id))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryShadowEvaluationRepository {
    records: Mutex<BTreeMap<String, ShadowEvaluationRecord>>,
}

impl ShadowEvaluationRepositoryPort for InMemoryShadowEvaluationRepository {
    fn upsert(&self, record: ShadowEvaluationRecord) -> Result<(), ShadowModeServiceError> {
        self.records
            .lock()
            .expect("in-memory shadow evaluation repository lock should not be poisoned")
            .insert(record.evaluation_id.clone(), record);
        Ok(())
    }

    fn load(
        &self,
        evaluation_id: &str,
    ) -> Result<Option<ShadowEvaluationRecord>, ShadowModeServiceError> {
        let normalized_evaluation_id = normalize_research_identifier(evaluation_id);
        Ok(self
            .records
            .lock()
            .expect("in-memory shadow evaluation repository lock should not be poisoned")
            .get(&normalized_evaluation_id)
            .cloned())
    }

    fn list_by_candidate(
        &self,
        candidate_id: &str,
        started_after_utc: Option<&str>,
        started_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ShadowEvaluationRecord>, ShadowModeServiceError> {
        let normalized_candidate_id = normalize_research_identifier(candidate_id);
        let started_after = started_after_utc
            .map(parse_shadow_utc_timestamp)
            .transpose()
            .map_err(map_contract_error)?;
        let started_before = started_before_utc
            .map(parse_shadow_utc_timestamp)
            .transpose()
            .map_err(map_contract_error)?;
        let mut records = self
            .records
            .lock()
            .expect("in-memory shadow evaluation repository lock should not be poisoned")
            .values()
            .filter(|record| record.candidate_id == normalized_candidate_id)
            .filter(|record| {
                let started_at = match parse_shadow_utc_timestamp(&record.started_at_utc) {
                    Ok(parsed) => parsed,
                    Err(_) => return false,
                };
                if let Some(started_after) = started_after
                    && started_at < started_after
                {
                    return false;
                }
                if let Some(started_before) = started_before
                    && started_at >= started_before
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .started_at_utc
                .cmp(&left.started_at_utc)
                .then_with(|| left.evaluation_id.cmp(&right.evaluation_id))
        });
        records.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(records)
    }
}

#[derive(Debug, Clone)]
pub struct StaticValidationEvidencePort {
    run: ValidationWorkflowRunRecord,
    artifacts: Vec<ValidationWorkflowArtifactRecord>,
    available: bool,
}

impl StaticValidationEvidencePort {
    pub fn unavailable() -> Self {
        Self {
            available: false,
            ..Self::default()
        }
    }
}

impl Default for StaticValidationEvidencePort {
    fn default() -> Self {
        Self {
            run: ValidationWorkflowRunRecord {
                run_id: "candidate::alpha-1::1712447000".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                run_state: ValidationWorkflowRunState::Completed,
                reason_code: ValidationWorkflowReasonCode::RunCompleted
                    .code()
                    .to_string(),
                gate_evaluation: serde_json::json!({
                    "outcome": "allow",
                    "reason_code": "validation_gate_evaluation_allowed"
                }),
                comparison_ready: true,
                actor_id: "ops-1".to_string(),
                correlation_id: "corr-validation-001".to_string(),
                started_at_utc: "2026-04-07T00:00:00Z".to_string(),
                completed_at_utc: Some("2026-04-07T00:01:00Z".to_string()),
            },
            artifacts: vec![ValidationWorkflowArtifactRecord {
                artifact_id: "candidate::alpha-1::1712447000::quality".to_string(),
                run_id: "candidate::alpha-1::1712447000".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: ValidationWorkflowStage::Quality,
                stage_index: ValidationWorkflowStage::Quality.stage_index(),
                stage_outcome: ValidationWorkflowStageOutcome::Passed,
                reason_code: ValidationWorkflowReasonCode::StagePassed.code().to_string(),
                diagnostics: ValidationDiagnosticsPayload {
                    out_of_sample_sharpe: 1.2,
                    max_drawdown: -0.1,
                    brier_score: Some(0.11),
                    expected_calibration_error: None,
                    overfit_indicator: 0.2,
                    overfit_flag: false,
                },
                actor_id: "ops-1".to_string(),
                correlation_id: "corr-validation-001".to_string(),
                stage_started_at_utc: "2026-04-07T00:00:10Z".to_string(),
                stage_completed_at_utc: "2026-04-07T00:00:20Z".to_string(),
            }],
            available: true,
        }
    }
}

impl ValidationEvidencePort for StaticValidationEvidencePort {
    fn load_completed_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, ShadowModeServiceError> {
        if !self.available {
            return Err(ShadowModeServiceError::dependency_unavailable(
                "validation evidence dependency is unavailable",
            ));
        }
        if normalize_research_identifier(run_id) != normalize_research_identifier(&self.run.run_id)
        {
            return Ok(None);
        }
        Ok(Some(self.run.clone()))
    }

    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ShadowModeServiceError> {
        if !self.available {
            return Err(ShadowModeServiceError::dependency_unavailable(
                "validation artifact dependency is unavailable",
            ));
        }
        if normalize_research_identifier(run_id) != normalize_research_identifier(&self.run.run_id)
        {
            return Ok(Vec::new());
        }
        Ok(self.artifacts.clone())
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicMarketContextPort;

impl MarketContextPort for DeterministicMarketContextPort {
    fn load_market_context(
        &self,
        candidate_id: &str,
        requested_at_utc: &str,
        proposed_market_context: &Value,
    ) -> Result<Value, ShadowModeServiceError> {
        let Value::Object(context) = proposed_market_context else {
            return Err(ShadowModeServiceError::invalid_payload(
                "market_context must be a JSON object",
                vec![ShadowEvaluationValidationIssue {
                    field: "market_context".to_string(),
                    code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                    message: "market_context must be a JSON object".to_string(),
                }],
            ));
        };
        if context.is_empty() {
            return Err(ShadowModeServiceError::dependency_unavailable(
                "market context dependency is unavailable",
            ));
        }

        let mut enriched = context.clone();
        enriched.insert(
            "candidate_id".to_string(),
            Value::String(candidate_id.to_string()),
        );
        enriched.insert(
            "observed_at_utc".to_string(),
            Value::String(requested_at_utc.to_string()),
        );
        Ok(Value::Object(enriched))
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicShadowSimulationPort;

impl ShadowSimulationPort for DeterministicShadowSimulationPort {
    fn simulate_shadow_outcomes(
        &self,
        _candidate_id: &str,
        market_context: &Value,
        signal_decisions: &Value,
        requested_at_utc: &str,
    ) -> Result<Vec<ShadowSimulationOutcome>, ShadowModeServiceError> {
        let Value::Object(market_context_map) = market_context else {
            return Err(ShadowModeServiceError::state_unavailable(
                "market context state is unavailable",
            ));
        };
        let mid_price = market_context_map
            .get("mid_price")
            .and_then(Value::as_f64)
            .or_else(|| {
                match (
                    market_context_map.get("best_bid").and_then(Value::as_f64),
                    market_context_map.get("best_ask").and_then(Value::as_f64),
                ) {
                    (Some(best_bid), Some(best_ask)) => Some((best_bid + best_ask) / 2.0),
                    _ => None,
                }
            })
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| {
                ShadowModeServiceError::dependency_unavailable(
                    "market context must include finite `mid_price` or both `best_bid` and `best_ask`",
                )
            })?;

        let Value::Object(signal_decisions_map) = signal_decisions else {
            return Err(ShadowModeServiceError::invalid_payload(
                "signal_decisions must be a JSON object",
                vec![ShadowEvaluationValidationIssue {
                    field: "signal_decisions".to_string(),
                    code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                    message: "signal_decisions must be a JSON object".to_string(),
                }],
            ));
        };
        let decisions = signal_decisions_map
            .get("decisions")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ShadowModeServiceError::dependency_unavailable(
                    "signal_decisions.decisions dependency is unavailable",
                )
            })?;
        if decisions.is_empty() {
            return Err(ShadowModeServiceError::dependency_unavailable(
                "signal_decisions.decisions cannot be empty",
            ));
        }

        let mut outcomes = Vec::new();
        for (index, decision) in decisions.iter().enumerate() {
            let Value::Object(decision) = decision else {
                return Err(ShadowModeServiceError::invalid_payload(
                    format!("signal_decisions.decisions[{index}] must be a JSON object"),
                    vec![ShadowEvaluationValidationIssue {
                        field: format!("signal_decisions.decisions[{index}]"),
                        code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                        message: "decision must be a JSON object".to_string(),
                    }],
                ));
            };
            let decision_side = decision
                .get("decision_side")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ShadowModeServiceError::state_unavailable(format!(
                        "signal_decisions.decisions[{index}].decision_side is unavailable"
                    ))
                })
                .and_then(|value| {
                    ShadowSimulationDecisionSide::parse(value).map_err(map_contract_error)
                })?;
            let intended_size = decision
                .get("intended_size")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0)
                .ok_or_else(|| {
                    ShadowModeServiceError::state_unavailable(format!(
                        "signal_decisions.decisions[{index}].intended_size is unavailable"
                    ))
                })?;
            let decision_timestamp_utc = decision
                .get("decision_timestamp_utc")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(requested_at_utc);
            parse_shadow_utc_timestamp(decision_timestamp_utc).map_err(map_contract_error)?;
            let simulated_at = parse_shadow_utc_timestamp(decision_timestamp_utc)
                .map_err(map_contract_error)?
                + Duration::seconds(1);
            let simulated_at_utc = simulated_at
                .format(&Rfc3339)
                .expect("shadow simulation timestamp should format");

            let simulated_fill_price = if decision_side == ShadowSimulationDecisionSide::Hold {
                mid_price
            } else {
                mid_price * 1.0005
            };
            let simulated_slippage_bps = if decision_side == ShadowSimulationDecisionSide::Hold {
                0.0
            } else {
                5.0
            };

            outcomes.push(ShadowSimulationOutcome {
                decision_side,
                intended_size,
                simulated_fill_size: intended_size,
                simulated_fill_price,
                simulated_slippage_bps,
                simulation_reason_code: SHADOW_SIMULATION_READ_ONLY_ENFORCED_CODE.to_string(),
                decision_timestamp_utc: decision_timestamp_utc.to_string(),
                simulated_at_utc,
            });
        }

        outcomes.sort_by(|left, right| {
            left.decision_timestamp_utc
                .cmp(&right.decision_timestamp_utc)
                .then_with(|| left.decision_side.cmp(&right.decision_side))
                .then_with(|| left.simulated_at_utc.cmp(&right.simulated_at_utc))
        });
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::research::ShadowSimulationReasonCode;
    use serde_json::json;

    #[derive(Debug, Clone)]
    struct StubValidationEvidencePort {
        run: Option<ValidationWorkflowRunRecord>,
        artifacts: Vec<ValidationWorkflowArtifactRecord>,
    }

    impl Default for StubValidationEvidencePort {
        fn default() -> Self {
            Self {
                run: Some(StaticValidationEvidencePort::default().run),
                artifacts: StaticValidationEvidencePort::default().artifacts,
            }
        }
    }

    impl ValidationEvidencePort for StubValidationEvidencePort {
        fn load_completed_validation_run(
            &self,
            run_id: &str,
        ) -> Result<Option<ValidationWorkflowRunRecord>, ShadowModeServiceError> {
            Ok(self
                .run
                .as_ref()
                .filter(|run| run.run_id == normalize_research_identifier(run_id))
                .cloned())
        }

        fn list_validation_artifacts_by_run(
            &self,
            _run_id: &str,
        ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ShadowModeServiceError> {
            Ok(self.artifacts.clone())
        }
    }

    fn sample_start_input() -> StartShadowEvaluationInput {
        StartShadowEvaluationInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            market_context: json!({
                "best_bid": 0.42,
                "best_ask": 0.44
            }),
            signal_decisions: json!({
                "decisions": [
                    {
                        "decision_side": "buy",
                        "intended_size": 10.0,
                        "decision_timestamp_utc": "2026-04-07T00:00:01Z"
                    }
                ]
            }),
            correlation_id: "corr-shadow-001".to_string(),
            requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    fn service_with_validation_evidence(
        validation_evidence: Arc<dyn ValidationEvidencePort>,
    ) -> ShadowModeService {
        ShadowModeService::new(
            Arc::new(InMemoryShadowEvaluationRepository::default()),
            validation_evidence,
            Arc::new(DeterministicMarketContextPort),
            Arc::new(DeterministicShadowSimulationPort),
        )
    }

    #[test]
    fn shadow_evaluation_start_requires_completed_validation_run_with_artifacts() {
        let mut validation_run = StaticValidationEvidencePort::default().run;
        validation_run.run_state = ValidationWorkflowRunState::Blocked;
        let service = service_with_validation_evidence(Arc::new(StubValidationEvidencePort {
            run: Some(validation_run),
            artifacts: Vec::new(),
        }));

        let error = service
            .start_shadow_evaluation(sample_start_input())
            .expect_err("ineligible validation run must fail closed");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::ValidationRunIneligible.code()
        );
    }

    #[test]
    fn shadow_evaluation_start_persists_read_only_simulation_outcomes() {
        let service =
            service_with_validation_evidence(Arc::new(StubValidationEvidencePort::default()));

        let evidence = service
            .start_shadow_evaluation(sample_start_input())
            .expect("shadow evaluation should start and persist");
        assert_eq!(
            evidence.reason_code,
            ShadowEvaluationReasonCode::EvaluationStarted.code()
        );
        assert_eq!(
            evidence.evaluation.reason_code,
            ShadowEvaluationReasonCode::EvaluationCompleted.code()
        );
        assert!(
            evidence
                .evaluation
                .simulation_outcomes
                .iter()
                .all(|outcome| {
                    outcome.simulation_reason_code
                        == ShadowSimulationReasonCode::ReadOnlyEnforced.code()
                })
        );
    }

    #[test]
    fn shadow_evaluation_start_fails_closed_when_market_context_dependency_unavailable() {
        let service =
            service_with_validation_evidence(Arc::new(StubValidationEvidencePort::default()));
        let mut input = sample_start_input();
        input.market_context = json!({});

        let error = service
            .start_shadow_evaluation(input)
            .expect_err("missing market context should fail closed");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::DependencyUnavailable.code()
        );
    }

    #[test]
    fn shadow_evaluation_start_fails_closed_when_simulation_dependency_unavailable() {
        let service =
            service_with_validation_evidence(Arc::new(StubValidationEvidencePort::default()));
        let mut input = sample_start_input();
        input.signal_decisions = json!({ "decisions": [] });

        let error = service
            .start_shadow_evaluation(input)
            .expect_err("empty decisions should fail closed");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::DependencyUnavailable.code()
        );
    }

    #[test]
    fn shadow_evaluation_list_rejects_invalid_limit_and_malformed_timestamp_boundaries() {
        let service =
            service_with_validation_evidence(Arc::new(StubValidationEvidencePort::default()));

        let mut invalid_limit = ListShadowEvaluationsInput {
            actor_id: "analyst-1".to_string(),
            actor_role: "read_only_analytics".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            limit: Some(0),
            started_after_utc: None,
            started_before_utc: None,
            correlation_id: "corr-shadow-list-001".to_string(),
            queried_at_utc: "2026-04-07T00:10:00Z".to_string(),
        };
        let error = service
            .list_shadow_evaluations(invalid_limit.clone())
            .expect_err("invalid limit must fail");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );

        invalid_limit.limit = Some(10);
        invalid_limit.started_after_utc = Some("not-a-timestamp".to_string());
        let error = service
            .list_shadow_evaluations(invalid_limit)
            .expect_err("malformed timestamp must fail");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "started_after_utc")
        );
    }

    #[test]
    fn shadow_evaluation_list_orders_results_deterministically() {
        let service =
            service_with_validation_evidence(Arc::new(StubValidationEvidencePort::default()));

        let mut first = sample_start_input();
        first.requested_at_utc = "2026-04-07T00:00:00Z".to_string();
        service
            .start_shadow_evaluation(first)
            .expect("first evaluation should persist");
        let mut second = sample_start_input();
        second.correlation_id = "corr-shadow-002".to_string();
        second.requested_at_utc = "2026-04-07T00:05:00Z".to_string();
        service
            .start_shadow_evaluation(second)
            .expect("second evaluation should persist");

        let evaluations = service
            .list_shadow_evaluations(ListShadowEvaluationsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                limit: Some(10),
                started_after_utc: None,
                started_before_utc: None,
                correlation_id: "corr-shadow-list-002".to_string(),
                queried_at_utc: "2026-04-07T00:10:00Z".to_string(),
            })
            .expect("list should succeed");

        assert_eq!(evaluations.len(), 2);
        assert!(evaluations[0].started_at_utc >= evaluations[1].started_at_utc);
    }
}
