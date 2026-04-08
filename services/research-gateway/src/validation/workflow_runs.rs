use crate::validation::evaluate_training_entry_gates;
use crate::validation::gate_policies::{
    ValidationGatePolicyOrchestrator, ValidationGatePolicyService, ValidationGatePolicyServiceError,
};
use domain::research::{
    ValidationDiagnosticsPayload, ValidationStageComparison, ValidationWorkflowArtifactRecord,
    ValidationWorkflowContractError, ValidationWorkflowReasonCode, ValidationWorkflowRunRecord,
    ValidationWorkflowRunState, ValidationWorkflowStage, ValidationWorkflowStageOutcome,
    ValidationWorkflowValidationIssue, build_validation_stage_comparison,
    compose_validation_run_id, normalize_research_identifier, parse_validation_utc_timestamp,
    validate_validation_diagnostics_payload,
};
use persistence::postgres::validation_artifacts::{
    ValidationArtifactPersistenceError,
    list_validation_artifacts_by_run as pg_list_validation_artifacts_by_run,
    load_validation_artifact_by_stage as pg_load_validation_artifact_by_stage,
    upsert_validation_artifact as pg_upsert_validation_artifact,
};
use persistence::postgres::validation_runs::{
    ValidationRunPersistenceError,
    list_validation_runs_by_candidate as pg_list_validation_runs_by_candidate,
    load_validation_run as pg_load_validation_run,
    upsert_validation_run as pg_upsert_validation_run,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValidationWorkflowServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ValidationWorkflowValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_stages: Vec<String>,
}

impl ValidationWorkflowServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ValidationWorkflowValidationIssue>,
    ) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
            failed_stages: Vec::new(),
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: ValidationWorkflowReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for validation-run mutations".to_string(),
            field_errors: Vec::new(),
            failed_stages: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: ValidationWorkflowReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for validation-run reads".to_string(),
            field_errors: Vec::new(),
            failed_stages: Vec::new(),
        }
    }

    fn run_not_found(run_id: &str) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::RunNotFound.code(),
            message: format!("validation run `{run_id}` was not found"),
            field_errors: Vec::new(),
            failed_stages: Vec::new(),
        }
    }

    fn artifact_not_found(run_id: &str, stage: ValidationWorkflowStage) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::ArtifactNotFound.code(),
            message: format!(
                "validation artifact for run `{run_id}` and stage `{}` was not found",
                stage.as_str()
            ),
            field_errors: Vec::new(),
            failed_stages: vec![stage.as_str().to_string()],
        }
    }

    fn gate_denied(message: impl Into<String>) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::GateDenied.code(),
            message: message.into(),
            field_errors: Vec::new(),
            failed_stages: vec!["gate_precheck".to_string()],
        }
    }

    fn dependency_unavailable(stage: ValidationWorkflowStage, message: impl Into<String>) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
            failed_stages: vec![stage.as_str().to_string()],
        }
    }

    fn state_unavailable(stage: ValidationWorkflowStage, message: impl Into<String>) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
            failed_stages: vec![stage.as_str().to_string()],
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
            failed_stages: Vec::new(),
        }
    }
}

impl Display for ValidationWorkflowServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ValidationWorkflowServiceError {}

#[derive(Debug, Clone)]
pub struct StartValidationRunInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub training_entry_observed_metrics: Value,
    pub stage_inputs: Value,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadValidationRunInput {
    pub actor_id: String,
    pub actor_role: String,
    pub run_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListValidationRunsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub limit: Option<i64>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadValidationArtifactInput {
    pub actor_id: String,
    pub actor_role: String,
    pub run_id: String,
    pub stage: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationRunDetailEvidence {
    pub run: ValidationWorkflowRunRecord,
    pub artifacts: Vec<ValidationWorkflowArtifactRecord>,
    pub comparisons: Vec<ValidationStageComparison>,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationArtifactEvidence {
    pub artifact: ValidationWorkflowArtifactRecord,
    pub reason_code: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

pub trait ValidationWorkflowRunOrchestrator: Send + Sync {
    fn start_validation_run(
        &self,
        input: StartValidationRunInput,
    ) -> Result<ValidationRunDetailEvidence, ValidationWorkflowServiceError>;

    fn read_validation_run(
        &self,
        input: ReadValidationRunInput,
    ) -> Result<ValidationRunDetailEvidence, ValidationWorkflowServiceError>;

    fn list_validation_runs(
        &self,
        input: ListValidationRunsInput,
    ) -> Result<Vec<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError>;

    fn read_validation_artifact(
        &self,
        input: ReadValidationArtifactInput,
    ) -> Result<ValidationArtifactEvidence, ValidationWorkflowServiceError>;
}

pub trait ValidationWorkflowRepositoryPort: Send + Sync {
    fn upsert_run(
        &self,
        run: ValidationWorkflowRunRecord,
    ) -> Result<(), ValidationWorkflowServiceError>;
    fn load_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError>;
    fn list_runs_by_candidate(
        &self,
        candidate_id: &str,
        limit: i64,
    ) -> Result<Vec<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError>;
    fn upsert_artifact(
        &self,
        artifact: ValidationWorkflowArtifactRecord,
    ) -> Result<(), ValidationWorkflowServiceError>;
    fn load_artifact_by_stage(
        &self,
        run_id: &str,
        stage: ValidationWorkflowStage,
    ) -> Result<Option<ValidationWorkflowArtifactRecord>, ValidationWorkflowServiceError>;
    fn list_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ValidationWorkflowServiceError>;
}

#[derive(Debug, Clone)]
pub struct ValidationStageExecutionResult {
    pub stage_outcome: ValidationWorkflowStageOutcome,
    pub reason_code: String,
    pub diagnostics: ValidationDiagnosticsPayload,
}

pub trait ValidationStageExecutorPort: Send + Sync {
    fn execute_stage(
        &self,
        stage: ValidationWorkflowStage,
        candidate_id: &str,
        stage_input: &Value,
    ) -> Result<ValidationStageExecutionResult, ValidationWorkflowServiceError>;
}

#[derive(Clone)]
pub struct ValidationWorkflowRunService {
    repository: Arc<dyn ValidationWorkflowRepositoryPort>,
    gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
    stage_executor: Arc<dyn ValidationStageExecutorPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl ValidationWorkflowRunService {
    pub fn new(
        repository: Arc<dyn ValidationWorkflowRepositoryPort>,
        gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
        stage_executor: Arc<dyn ValidationStageExecutorPort>,
    ) -> Self {
        Self {
            repository,
            gate_orchestrator,
            stage_executor,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        let gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator> =
            Arc::new(ValidationGatePolicyService::default());
        Self::new(
            Arc::new(InMemoryValidationWorkflowRepository::default()),
            gate_orchestrator,
            Arc::new(DeterministicValidationStageExecutor),
        )
    }

    pub fn postgres(
        pool: PgPool,
        gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
    ) -> Self {
        Self::new(
            Arc::new(PostgresValidationWorkflowRepository::new(pool)),
            gate_orchestrator,
            Arc::new(DeterministicValidationStageExecutor),
        )
    }

    pub fn with_stage_executor(
        mut self,
        stage_executor: Arc<dyn ValidationStageExecutorPort>,
    ) -> Self {
        self.stage_executor = stage_executor;
        self
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, ValidationWorkflowServiceError> {
        self.operation_lock.lock().map_err(|_| {
            ValidationWorkflowServiceError::persistence_unavailable(
                "validation workflow operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for ValidationWorkflowRunService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ValidationWorkflowRunOrchestrator for ValidationWorkflowRunService {
    fn start_validation_run(
        &self,
        input: StartValidationRunInput,
    ) -> Result<ValidationRunDetailEvidence, ValidationWorkflowServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_validation_run_telemetry(
                "validation_run_start_v1",
                "validation_run_start",
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
            "validation_run_start_v1",
            "validation_run_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "candidate_id",
            &input.candidate_id,
            "validation_run_start_v1",
            "validation_run_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_run_start_v1",
            "validation_run_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "requested_at_utc",
            &input.requested_at_utc,
            "validation_run_start_v1",
            "validation_run_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "requested_at_utc",
            &input.requested_at_utc,
            "validation_run_start_v1",
            "validation_run_start",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.requested_at_utc,
        )?;

        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            let error = ValidationWorkflowServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![ValidationWorkflowValidationIssue {
                    field: "candidate_id".to_string(),
                    code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            );
            emit_validation_run_telemetry(
                "validation_run_start_v1",
                "validation_run_start",
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

        let gate_evaluation = evaluate_training_entry_gates(
            self.gate_orchestrator.as_ref(),
            input.actor_id.clone(),
            input.actor_role.clone(),
            normalized_candidate_id.clone(),
            input.training_entry_observed_metrics.clone(),
            input.correlation_id.clone(),
            input.requested_at_utc.clone(),
        )
        .map_err(map_gate_error)
        .inspect_err(|error| {
            emit_validation_run_telemetry(
                "validation_run_start_v1",
                "validation_run_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                Some("gate_precheck"),
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
        })?;
        if gate_evaluation.outcome != "allow" {
            let error = ValidationWorkflowServiceError::gate_denied(
                "training-entry gate precheck denied validation run progression",
            );
            emit_validation_run_telemetry(
                "validation_run_start_v1",
                "validation_run_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                Some("gate_precheck"),
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }

        let stage_inputs = parse_stage_inputs(&input.stage_inputs).inspect_err(|error| {
            emit_validation_run_telemetry(
                "validation_run_start_v1",
                "validation_run_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
        })?;

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_validation_run_telemetry(
                "validation_run_start_v1",
                "validation_run_start",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
        })?;

        let run_id = compose_validation_run_id(&normalized_candidate_id, &input.requested_at_utc)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_start_v1",
                    "validation_run_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;
        let started_at = parse_validation_utc_timestamp(&input.requested_at_utc)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_start_v1",
                    "validation_run_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;

        let gate_evaluation_json = serde_json::to_value(&gate_evaluation).map_err(|error| {
            ValidationWorkflowServiceError::invalid_payload(
                format!("unable to serialize gate evaluation evidence: {error}"),
                Vec::new(),
            )
        })?;

        let mut run_record = ValidationWorkflowRunRecord {
            run_id: run_id.clone(),
            candidate_id: normalized_candidate_id.clone(),
            run_state: ValidationWorkflowRunState::Running,
            reason_code: ValidationWorkflowReasonCode::RunStarted.code().to_string(),
            gate_evaluation: gate_evaluation_json,
            comparison_ready: false,
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            started_at_utc: input.requested_at_utc.clone(),
            completed_at_utc: None,
        };
        self.repository
            .upsert_run(run_record.clone())
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_start_v1",
                    "validation_run_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;

        let mut artifacts = Vec::new();
        let mut terminal_reason_code = ValidationWorkflowReasonCode::RunCompleted
            .code()
            .to_string();
        let mut terminal_state = ValidationWorkflowRunState::Completed;
        let mut terminal_timestamp = input.requested_at_utc.clone();

        for stage in ValidationWorkflowStage::ordered() {
            let Some(stage_input) = stage_inputs.get(stage.as_str()) else {
                terminal_reason_code = ValidationWorkflowReasonCode::DependencyUnavailable
                    .code()
                    .to_string();
                terminal_state = ValidationWorkflowRunState::Blocked;
                let stage_started_at_utc = stage_timestamp(started_at, stage.stage_index(), 0);
                let stage_completed_at_utc = stage_timestamp(started_at, stage.stage_index(), 1);
                let artifact = ValidationWorkflowArtifactRecord {
                    artifact_id: normalize_research_identifier(&format!(
                        "{run_id}::{}",
                        stage.as_str()
                    )),
                    run_id: run_id.clone(),
                    candidate_id: normalized_candidate_id.clone(),
                    stage,
                    stage_index: stage.stage_index(),
                    stage_outcome: ValidationWorkflowStageOutcome::Blocked,
                    reason_code: ValidationWorkflowReasonCode::DependencyUnavailable
                        .code()
                        .to_string(),
                    diagnostics: blocked_diagnostics(),
                    actor_id: input.actor_id.clone(),
                    correlation_id: input.correlation_id.clone(),
                    stage_started_at_utc: stage_started_at_utc.clone(),
                    stage_completed_at_utc: stage_completed_at_utc.clone(),
                };
                self.repository
                    .upsert_artifact(artifact.clone())
                    .inspect_err(|error| {
                        emit_validation_run_telemetry(
                            "validation_run_start_v1",
                            "validation_run_start",
                            "deny",
                            &input.actor_id,
                            &normalized_candidate_id,
                            Some(stage.as_str()),
                            error.code,
                            &input.correlation_id,
                            &input.requested_at_utc,
                        );
                    })?;
                artifacts.push(artifact);
                terminal_timestamp = stage_completed_at_utc;
                break;
            };

            let execution = self
                .stage_executor
                .execute_stage(stage, &normalized_candidate_id, stage_input)
                .inspect_err(|error| {
                    emit_validation_run_telemetry(
                        "validation_run_start_v1",
                        "validation_run_start",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        Some(stage.as_str()),
                        error.code,
                        &input.correlation_id,
                        &input.requested_at_utc,
                    );
                })?;

            let stage_started_at_utc = stage_timestamp(started_at, stage.stage_index(), 0);
            let stage_completed_at_utc = stage_timestamp(started_at, stage.stage_index(), 1);
            let artifact = ValidationWorkflowArtifactRecord {
                artifact_id: normalize_research_identifier(&format!(
                    "{run_id}::{}",
                    stage.as_str()
                )),
                run_id: run_id.clone(),
                candidate_id: normalized_candidate_id.clone(),
                stage,
                stage_index: stage.stage_index(),
                stage_outcome: execution.stage_outcome,
                reason_code: execution.reason_code.clone(),
                diagnostics: execution.diagnostics,
                actor_id: input.actor_id.clone(),
                correlation_id: input.correlation_id.clone(),
                stage_started_at_utc: stage_started_at_utc.clone(),
                stage_completed_at_utc: stage_completed_at_utc.clone(),
            };
            self.repository
                .upsert_artifact(artifact.clone())
                .inspect_err(|error| {
                    emit_validation_run_telemetry(
                        "validation_run_start_v1",
                        "validation_run_start",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        Some(stage.as_str()),
                        error.code,
                        &input.correlation_id,
                        &input.requested_at_utc,
                    );
                })?;
            artifacts.push(artifact);
            terminal_timestamp = stage_completed_at_utc;

            match execution.stage_outcome {
                ValidationWorkflowStageOutcome::Passed => {}
                ValidationWorkflowStageOutcome::Failed => {
                    terminal_state = ValidationWorkflowRunState::Failed;
                    terminal_reason_code = execution.reason_code;
                    break;
                }
                ValidationWorkflowStageOutcome::Blocked => {
                    terminal_state = ValidationWorkflowRunState::Blocked;
                    terminal_reason_code = execution.reason_code;
                    break;
                }
            }
        }

        run_record.run_state = terminal_state;
        run_record.reason_code = terminal_reason_code.clone();
        run_record.completed_at_utc = Some(terminal_timestamp.clone());
        self.repository
            .upsert_run(run_record.clone())
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_start_v1",
                    "validation_run_start",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
            })?;

        let comparisons = if run_record.run_state == ValidationWorkflowRunState::Completed {
            self.build_comparisons(&run_record, &artifacts)?
        } else {
            Vec::new()
        };
        if !comparisons.is_empty() && !run_record.comparison_ready {
            run_record.comparison_ready = true;
            self.repository
                .upsert_run(run_record.clone())
                .inspect_err(|error| {
                    emit_validation_run_telemetry(
                        "validation_run_start_v1",
                        "validation_run_start",
                        "deny",
                        &input.actor_id,
                        &normalized_candidate_id,
                        None,
                        error.code,
                        &input.correlation_id,
                        &input.requested_at_utc,
                    );
                })?;
        }

        emit_validation_run_telemetry(
            "validation_run_start_v1",
            "validation_run_start",
            if run_record.run_state == ValidationWorkflowRunState::Completed {
                "allow"
            } else {
                "deny"
            },
            &input.actor_id,
            &normalized_candidate_id,
            None,
            &run_record.reason_code,
            &input.correlation_id,
            &terminal_timestamp,
        );

        Ok(ValidationRunDetailEvidence {
            run: run_record,
            artifacts,
            comparisons,
            reason_code: ValidationWorkflowReasonCode::RunStarted.code().to_string(),
        })
    }

    fn read_validation_run(
        &self,
        input: ReadValidationRunInput,
    ) -> Result<ValidationRunDetailEvidence, ValidationWorkflowServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_validation_run_telemetry(
                "validation_run_read_v1",
                "validation_run_read",
                "deny",
                &input.actor_id,
                &input.run_id,
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
            "validation_run_read_v1",
            "validation_run_read",
            &input.actor_id,
            &input.run_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "run_id",
            &input.run_id,
            "validation_run_read_v1",
            "validation_run_read",
            &input.actor_id,
            &input.run_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_run_read_v1",
            "validation_run_read",
            &input.actor_id,
            &input.run_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_run_read_v1",
            "validation_run_read",
            &input.actor_id,
            &input.run_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_run_read_v1",
            "validation_run_read",
            &input.actor_id,
            &input.run_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;

        let normalized_run_id = normalize_research_identifier(&input.run_id);
        let Some(run) = self
            .repository
            .load_run(&normalized_run_id)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_read_v1",
                    "validation_run_read",
                    "deny",
                    &input.actor_id,
                    &normalized_run_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?
        else {
            let error = ValidationWorkflowServiceError::run_not_found(&normalized_run_id);
            emit_validation_run_telemetry(
                "validation_run_read_v1",
                "validation_run_read",
                "deny",
                &input.actor_id,
                &normalized_run_id,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        };

        let artifacts = self
            .repository
            .list_artifacts_by_run(&run.run_id)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_read_v1",
                    "validation_run_read",
                    "deny",
                    &input.actor_id,
                    &run.run_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;
        let comparisons = if run.run_state == ValidationWorkflowRunState::Completed {
            self.build_comparisons(&run, &artifacts)?
        } else {
            Vec::new()
        };

        emit_validation_run_telemetry(
            "validation_run_read_v1",
            "validation_run_read",
            "allow",
            &input.actor_id,
            &run.run_id,
            None,
            ValidationWorkflowReasonCode::RunRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(ValidationRunDetailEvidence {
            run,
            artifacts,
            comparisons,
            reason_code: ValidationWorkflowReasonCode::RunRead.code().to_string(),
        })
    }

    fn list_validation_runs(
        &self,
        input: ListValidationRunsInput,
    ) -> Result<Vec<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_validation_run_telemetry(
                "validation_run_list_v1",
                "validation_run_list",
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
            "validation_run_list_v1",
            "validation_run_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "candidate_id",
            &input.candidate_id,
            "validation_run_list_v1",
            "validation_run_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_run_list_v1",
            "validation_run_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_run_list_v1",
            "validation_run_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_run_list_v1",
            "validation_run_list",
            &input.actor_id,
            &input.candidate_id,
            None,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;

        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            let error = ValidationWorkflowServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![ValidationWorkflowValidationIssue {
                    field: "candidate_id".to_string(),
                    code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            );
            emit_validation_run_telemetry(
                "validation_run_list_v1",
                "validation_run_list",
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
        let limit = input.limit.unwrap_or(25).clamp(1, 200);

        let runs = self
            .repository
            .list_runs_by_candidate(&normalized_candidate_id, limit)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_run_list_v1",
                    "validation_run_list",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;

        emit_validation_run_telemetry(
            "validation_run_list_v1",
            "validation_run_list",
            "allow",
            &input.actor_id,
            &normalized_candidate_id,
            None,
            ValidationWorkflowReasonCode::RunListed.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(runs)
    }

    fn read_validation_artifact(
        &self,
        input: ReadValidationArtifactInput,
    ) -> Result<ValidationArtifactEvidence, ValidationWorkflowServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_validation_run_telemetry(
                "validation_artifact_read_v1",
                "validation_artifact_read",
                "deny",
                &input.actor_id,
                &input.run_id,
                Some(&input.stage),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "validation_artifact_read_v1",
            "validation_artifact_read",
            &input.actor_id,
            &input.run_id,
            Some(&input.stage),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "run_id",
            &input.run_id,
            "validation_artifact_read_v1",
            "validation_artifact_read",
            &input.actor_id,
            &input.run_id,
            Some(&input.stage),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "stage",
            &input.stage,
            "validation_artifact_read_v1",
            "validation_artifact_read",
            &input.actor_id,
            &input.run_id,
            Some(&input.stage),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_artifact_read_v1",
            "validation_artifact_read",
            &input.actor_id,
            &input.run_id,
            Some(&input.stage),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_artifact_read_v1",
            "validation_artifact_read",
            &input.actor_id,
            &input.run_id,
            Some(&input.stage),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_artifact_read_v1",
            "validation_artifact_read",
            &input.actor_id,
            &input.run_id,
            Some(&input.stage),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        let stage = ValidationWorkflowStage::parse(&input.stage)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_artifact_read_v1",
                    "validation_artifact_read",
                    "deny",
                    &input.actor_id,
                    &input.run_id,
                    Some(&input.stage),
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;
        let normalized_run_id = normalize_research_identifier(&input.run_id);
        let Some(artifact) = self
            .repository
            .load_artifact_by_stage(&normalized_run_id, stage)
            .inspect_err(|error| {
                emit_validation_run_telemetry(
                    "validation_artifact_read_v1",
                    "validation_artifact_read",
                    "deny",
                    &input.actor_id,
                    &normalized_run_id,
                    Some(stage.as_str()),
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?
        else {
            let error =
                ValidationWorkflowServiceError::artifact_not_found(&normalized_run_id, stage);
            emit_validation_run_telemetry(
                "validation_artifact_read_v1",
                "validation_artifact_read",
                "deny",
                &input.actor_id,
                &normalized_run_id,
                Some(stage.as_str()),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        };

        emit_validation_run_telemetry(
            "validation_artifact_read_v1",
            "validation_artifact_read",
            "allow",
            &input.actor_id,
            &normalized_run_id,
            Some(stage.as_str()),
            ValidationWorkflowReasonCode::ArtifactRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(ValidationArtifactEvidence {
            artifact,
            reason_code: ValidationWorkflowReasonCode::ArtifactRead
                .code()
                .to_string(),
            actor_id: input.actor_id,
            correlation_id: input.correlation_id,
            queried_at_utc: input.queried_at_utc,
        })
    }
}

impl ValidationWorkflowRunService {
    fn build_comparisons(
        &self,
        run: &ValidationWorkflowRunRecord,
        current_artifacts: &[ValidationWorkflowArtifactRecord],
    ) -> Result<Vec<ValidationStageComparison>, ValidationWorkflowServiceError> {
        let current_started_at =
            parse_validation_utc_timestamp(&run.started_at_utc).map_err(map_contract_error)?;
        let mut previous_completed_run = None;
        for candidate_run in self
            .repository
            .list_runs_by_candidate(&run.candidate_id, 200)?
        {
            if candidate_run.run_id == run.run_id
                || candidate_run.run_state != ValidationWorkflowRunState::Completed
            {
                continue;
            }

            let candidate_started_at =
                parse_validation_utc_timestamp(&candidate_run.started_at_utc)
                    .map_err(map_contract_error)?;
            if candidate_started_at < current_started_at {
                previous_completed_run = Some(candidate_run);
                break;
            }
        }

        let Some(previous_run) = previous_completed_run else {
            return Ok(Vec::new());
        };

        let previous_artifacts = self
            .repository
            .list_artifacts_by_run(&previous_run.run_id)?;
        let previous_by_stage = previous_artifacts
            .into_iter()
            .map(|artifact| (artifact.stage, artifact))
            .collect::<BTreeMap<_, _>>();

        let mut comparisons = Vec::new();
        for current_artifact in current_artifacts
            .iter()
            .filter(|artifact| artifact.stage_outcome == ValidationWorkflowStageOutcome::Passed)
        {
            let Some(previous_artifact) = previous_by_stage.get(&current_artifact.stage) else {
                continue;
            };
            if previous_artifact.stage_outcome != ValidationWorkflowStageOutcome::Passed {
                continue;
            }

            let comparison = build_validation_stage_comparison(
                current_artifact.stage,
                run.run_id.clone(),
                previous_run.run_id.clone(),
                &current_artifact.diagnostics,
                &previous_artifact.diagnostics,
            )
            .map_err(map_contract_error)?;
            comparisons.push(comparison);
        }
        comparisons.sort_by(|left, right| left.stage.stage_index().cmp(&right.stage.stage_index()));
        Ok(comparisons)
    }
}

#[derive(Debug, Default)]
pub struct DeterministicValidationStageExecutor;

impl ValidationStageExecutorPort for DeterministicValidationStageExecutor {
    fn execute_stage(
        &self,
        stage: ValidationWorkflowStage,
        _candidate_id: &str,
        stage_input: &Value,
    ) -> Result<ValidationStageExecutionResult, ValidationWorkflowServiceError> {
        let Value::Object(input) = stage_input else {
            return Err(ValidationWorkflowServiceError::invalid_payload(
                format!("stage `{}` input must be a JSON object", stage.as_str()),
                vec![ValidationWorkflowValidationIssue {
                    field: format!("stage_inputs.{}", stage.as_str()),
                    code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                    message: "stage input must be a JSON object".to_string(),
                }],
            ));
        };

        if let Some(state) = input.get("dependency_state").and_then(Value::as_str)
            && normalize_research_identifier(state) == "dependency_unavailable"
        {
            return Ok(ValidationStageExecutionResult {
                stage_outcome: ValidationWorkflowStageOutcome::Blocked,
                reason_code: ValidationWorkflowReasonCode::DependencyUnavailable
                    .code()
                    .to_string(),
                diagnostics: blocked_diagnostics(),
            });
        }
        if let Some(state) = input.get("dependency_state").and_then(Value::as_str)
            && normalize_research_identifier(state) == "state_unavailable"
        {
            return Ok(ValidationStageExecutionResult {
                stage_outcome: ValidationWorkflowStageOutcome::Blocked,
                reason_code: ValidationWorkflowReasonCode::StateUnavailable
                    .code()
                    .to_string(),
                diagnostics: blocked_diagnostics(),
            });
        }

        let out_of_sample_sharpe = parse_stage_metric(input, "out_of_sample_sharpe", stage)?;
        let max_drawdown = parse_stage_metric(input, "max_drawdown", stage)?;
        let overfit_indicator = parse_stage_metric(input, "overfit_indicator", stage)?;
        let brier_score = parse_optional_stage_metric(input, "brier_score", stage)?;
        let expected_calibration_error =
            parse_optional_stage_metric(input, "expected_calibration_error", stage)?;
        if brier_score.is_none() && expected_calibration_error.is_none() {
            return Ok(ValidationStageExecutionResult {
                stage_outcome: ValidationWorkflowStageOutcome::Blocked,
                reason_code: ValidationWorkflowReasonCode::DependencyUnavailable
                    .code()
                    .to_string(),
                diagnostics: blocked_diagnostics(),
            });
        }
        let overfit_flag = input
            .get("overfit_flag")
            .and_then(Value::as_bool)
            .unwrap_or(overfit_indicator >= 0.8);
        let diagnostics = ValidationDiagnosticsPayload {
            out_of_sample_sharpe,
            max_drawdown,
            brier_score,
            expected_calibration_error,
            overfit_indicator,
            overfit_flag,
        };
        validate_validation_diagnostics_payload(&diagnostics).map_err(map_contract_error)?;

        let force_fail = input
            .get("force_fail")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(ValidationStageExecutionResult {
            stage_outcome: if force_fail {
                ValidationWorkflowStageOutcome::Failed
            } else {
                ValidationWorkflowStageOutcome::Passed
            },
            reason_code: if force_fail {
                ValidationWorkflowReasonCode::StageFailed.code().to_string()
            } else {
                ValidationWorkflowReasonCode::StagePassed.code().to_string()
            },
            diagnostics,
        })
    }
}

fn parse_stage_metric(
    input: &serde_json::Map<String, Value>,
    metric_key: &str,
    stage: ValidationWorkflowStage,
) -> Result<f64, ValidationWorkflowServiceError> {
    match input.get(metric_key) {
        None => Err(ValidationWorkflowServiceError::dependency_unavailable(
            stage,
            format!("stage `{}` requires `{metric_key}`", stage.as_str()),
        )),
        Some(Value::Null) => Err(ValidationWorkflowServiceError::state_unavailable(
            stage,
            format!("stage `{}` metric `{metric_key}` is null", stage.as_str()),
        )),
        Some(Value::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                ValidationWorkflowServiceError::invalid_payload(
                    format!(
                        "stage `{}` metric `{metric_key}` must be a finite number",
                        stage.as_str()
                    ),
                    vec![ValidationWorkflowValidationIssue {
                        field: format!("stage_inputs.{}.{}", stage.as_str(), metric_key),
                        code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                        message: "metric must be a finite number".to_string(),
                    }],
                )
            }),
        _ => Err(ValidationWorkflowServiceError::invalid_payload(
            format!(
                "stage `{}` metric `{metric_key}` must be numeric",
                stage.as_str()
            ),
            vec![ValidationWorkflowValidationIssue {
                field: format!("stage_inputs.{}.{}", stage.as_str(), metric_key),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: "metric must be numeric".to_string(),
            }],
        )),
    }
}

fn parse_optional_stage_metric(
    input: &serde_json::Map<String, Value>,
    metric_key: &str,
    stage: ValidationWorkflowStage,
) -> Result<Option<f64>, ValidationWorkflowServiceError> {
    match input.get(metric_key) {
        None => Ok(None),
        Some(Value::Null) => Err(ValidationWorkflowServiceError::state_unavailable(
            stage,
            format!("stage `{}` metric `{metric_key}` is null", stage.as_str()),
        )),
        Some(Value::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Some)
            .ok_or_else(|| {
                ValidationWorkflowServiceError::invalid_payload(
                    format!(
                        "stage `{}` metric `{metric_key}` must be a finite number",
                        stage.as_str()
                    ),
                    vec![ValidationWorkflowValidationIssue {
                        field: format!("stage_inputs.{}.{}", stage.as_str(), metric_key),
                        code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                        message: "metric must be a finite number".to_string(),
                    }],
                )
            }),
        _ => Err(ValidationWorkflowServiceError::invalid_payload(
            format!(
                "stage `{}` metric `{metric_key}` must be numeric",
                stage.as_str()
            ),
            vec![ValidationWorkflowValidationIssue {
                field: format!("stage_inputs.{}.{}", stage.as_str(), metric_key),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: "metric must be numeric".to_string(),
            }],
        )),
    }
}

fn blocked_diagnostics() -> ValidationDiagnosticsPayload {
    ValidationDiagnosticsPayload {
        out_of_sample_sharpe: 0.0,
        max_drawdown: 0.0,
        brier_score: Some(1.0),
        expected_calibration_error: None,
        overfit_indicator: 1.0,
        overfit_flag: true,
    }
}

fn parse_stage_inputs(
    stage_inputs: &Value,
) -> Result<serde_json::Map<String, Value>, ValidationWorkflowServiceError> {
    let Value::Object(stage_inputs_map) = stage_inputs else {
        return Err(ValidationWorkflowServiceError::invalid_payload(
            "stage_inputs must be a JSON object keyed by stage name",
            vec![ValidationWorkflowValidationIssue {
                field: "stage_inputs".to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message: "stage_inputs must be a JSON object keyed by stage name".to_string(),
            }],
        ));
    };
    Ok(stage_inputs_map.clone())
}

fn stage_timestamp(base: OffsetDateTime, stage_index: i16, additional_seconds: i64) -> String {
    let shifted = base + Duration::seconds(i64::from(stage_index - 1) * 2 + additional_seconds);
    shifted
        .format(&Rfc3339)
        .expect("validation stage timestamp should always format")
}

fn map_gate_error(error: ValidationGatePolicyServiceError) -> ValidationWorkflowServiceError {
    match error.code {
        code if code == ValidationWorkflowReasonCode::UnauthorizedRole.code() => {
            ValidationWorkflowServiceError::unauthorized_mutation_role()
        }
        "validation_gate_missing_mandatory_policy"
        | "validation_gate_policy_unresolved"
        | "validation_gate_failed" => ValidationWorkflowServiceError::gate_denied(error.message),
        "validation_gate_dependency_unavailable" => {
            ValidationWorkflowServiceError::dependency_unavailable(
                ValidationWorkflowStage::Quality,
                error.message,
            )
        }
        "validation_gate_state_unavailable" => ValidationWorkflowServiceError::state_unavailable(
            ValidationWorkflowStage::Quality,
            error.message,
        ),
        "validation_gate_persistence_unavailable"
        | "validation_gate_policy_query_failed"
        | "validation_gate_policy_row_decode_failed" => {
            ValidationWorkflowServiceError::persistence_unavailable(error.message)
        }
        _ => ValidationWorkflowServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| ValidationWorkflowValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn map_contract_error(error: ValidationWorkflowContractError) -> ValidationWorkflowServiceError {
    ValidationWorkflowServiceError::invalid_payload(error.message, error.field_errors)
}

fn map_run_persistence_error(
    error: ValidationRunPersistenceError,
) -> ValidationWorkflowServiceError {
    match error.code {
        "validation_run_query_failed" | "validation_run_row_decode_failed" => {
            ValidationWorkflowServiceError::persistence_unavailable(error.message)
        }
        _ => ValidationWorkflowServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
            failed_stages: Vec::new(),
        },
    }
}

fn map_artifact_persistence_error(
    error: ValidationArtifactPersistenceError,
) -> ValidationWorkflowServiceError {
    match error.code {
        "validation_artifact_query_failed" | "validation_artifact_row_decode_failed" => {
            ValidationWorkflowServiceError::persistence_unavailable(error.message)
        }
        _ => ValidationWorkflowServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
            failed_stages: Vec::new(),
        },
    }
}

fn validate_mutation_role(role: &str) -> Result<(), ValidationWorkflowServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ValidationWorkflowServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), ValidationWorkflowServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ValidationWorkflowServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ValidationWorkflowServiceError> {
    if value.trim().is_empty() {
        return Err(ValidationWorkflowServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![ValidationWorkflowValidationIssue {
                field: field.to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
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
    subject_id: &str,
    stage: Option<&str>,
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), ValidationWorkflowServiceError> {
    validate_non_empty(field, value).inspect_err(|error| {
        emit_validation_run_telemetry(
            event_name,
            action,
            "deny",
            actor_id,
            subject_id,
            stage,
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
    subject_id: &str,
    stage: Option<&str>,
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), ValidationWorkflowServiceError> {
    parse_validation_utc_timestamp(value)
        .map_err(|error| {
            ValidationWorkflowServiceError::invalid_payload(
                error.message,
                vec![ValidationWorkflowValidationIssue {
                    field: field.to_string(),
                    code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                    message: format!("{field} must be RFC3339 UTC"),
                }],
            )
        })
        .inspect_err(|error| {
            emit_validation_run_telemetry(
                event_name,
                action,
                "deny",
                actor_id,
                subject_id,
                stage,
                error.code,
                correlation_id,
                timestamp_utc,
            );
        })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_validation_run_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    subject_id: &str,
    stage: Option<&str>,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = ValidationRunTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        subject_id,
        stage,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(ValidationRunSecuritySignal {
                name: "validation_run_denied_v1",
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
        serde_json::to_string(&event).expect("validation-run telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct ValidationRunTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    subject_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<&'a str>,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<ValidationRunSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct ValidationRunSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresValidationWorkflowRepository {
    pool: PgPool,
}

impl PostgresValidationWorkflowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_run_future<F, T>(&self, future: F) -> Result<T, ValidationWorkflowServiceError>
    where
        F: Future<Output = Result<T, ValidationRunPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_run_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ValidationWorkflowServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_run_persistence_error),
        }
    }

    fn run_artifact_future<F, T>(&self, future: F) -> Result<T, ValidationWorkflowServiceError>
    where
        F: Future<Output = Result<T, ValidationArtifactPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_artifact_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ValidationWorkflowServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_artifact_persistence_error),
        }
    }
}

impl ValidationWorkflowRepositoryPort for PostgresValidationWorkflowRepository {
    fn upsert_run(
        &self,
        run: ValidationWorkflowRunRecord,
    ) -> Result<(), ValidationWorkflowServiceError> {
        self.run_run_future(pg_upsert_validation_run(&self.pool, &run))
    }

    fn load_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError> {
        self.run_run_future(pg_load_validation_run(&self.pool, run_id))
    }

    fn list_runs_by_candidate(
        &self,
        candidate_id: &str,
        limit: i64,
    ) -> Result<Vec<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError> {
        self.run_run_future(pg_list_validation_runs_by_candidate(
            &self.pool,
            candidate_id,
            limit,
        ))
    }

    fn upsert_artifact(
        &self,
        artifact: ValidationWorkflowArtifactRecord,
    ) -> Result<(), ValidationWorkflowServiceError> {
        self.run_artifact_future(pg_upsert_validation_artifact(&self.pool, &artifact))
    }

    fn load_artifact_by_stage(
        &self,
        run_id: &str,
        stage: ValidationWorkflowStage,
    ) -> Result<Option<ValidationWorkflowArtifactRecord>, ValidationWorkflowServiceError> {
        self.run_artifact_future(pg_load_validation_artifact_by_stage(
            &self.pool, run_id, stage,
        ))
    }

    fn list_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ValidationWorkflowServiceError> {
        self.run_artifact_future(pg_list_validation_artifacts_by_run(&self.pool, run_id))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryValidationWorkflowRepository {
    runs: Mutex<BTreeMap<String, ValidationWorkflowRunRecord>>,
    artifacts: Mutex<BTreeMap<String, ValidationWorkflowArtifactRecord>>,
}

impl ValidationWorkflowRepositoryPort for InMemoryValidationWorkflowRepository {
    fn upsert_run(
        &self,
        run: ValidationWorkflowRunRecord,
    ) -> Result<(), ValidationWorkflowServiceError> {
        self.runs
            .lock()
            .expect("in-memory validation workflow run lock should not be poisoned")
            .insert(run.run_id.clone(), run);
        Ok(())
    }

    fn load_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        Ok(self
            .runs
            .lock()
            .expect("in-memory validation workflow run lock should not be poisoned")
            .get(&normalized_run_id)
            .cloned())
    }

    fn list_runs_by_candidate(
        &self,
        candidate_id: &str,
        limit: i64,
    ) -> Result<Vec<ValidationWorkflowRunRecord>, ValidationWorkflowServiceError> {
        let normalized_candidate_id = normalize_research_identifier(candidate_id);
        let mut runs = self
            .runs
            .lock()
            .expect("in-memory validation workflow run lock should not be poisoned")
            .values()
            .filter(|run| run.candidate_id == normalized_candidate_id)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            right
                .started_at_utc
                .cmp(&left.started_at_utc)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        runs.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(runs)
    }

    fn upsert_artifact(
        &self,
        artifact: ValidationWorkflowArtifactRecord,
    ) -> Result<(), ValidationWorkflowServiceError> {
        self.artifacts
            .lock()
            .expect("in-memory validation workflow artifact lock should not be poisoned")
            .insert(artifact.artifact_id.clone(), artifact);
        Ok(())
    }

    fn load_artifact_by_stage(
        &self,
        run_id: &str,
        stage: ValidationWorkflowStage,
    ) -> Result<Option<ValidationWorkflowArtifactRecord>, ValidationWorkflowServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        Ok(self
            .artifacts
            .lock()
            .expect("in-memory validation workflow artifact lock should not be poisoned")
            .values()
            .find(|artifact| artifact.run_id == normalized_run_id && artifact.stage == stage)
            .cloned())
    }

    fn list_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, ValidationWorkflowServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        let mut artifacts = self
            .artifacts
            .lock()
            .expect("in-memory validation workflow artifact lock should not be poisoned")
            .values()
            .filter(|artifact| artifact.run_id == normalized_run_id)
            .cloned()
            .collect::<Vec<_>>();
        artifacts.sort_by(|left, right| {
            left.stage_index.cmp(&right.stage_index).then_with(|| {
                left.stage_completed_at_utc
                    .cmp(&right.stage_completed_at_utc)
                    .then_with(|| left.artifact_id.cmp(&right.artifact_id))
            })
        });
        Ok(artifacts)
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
    use domain::research::ValidationGateReasonCode;
    use serde_json::json;

    #[derive(Debug, Default)]
    struct StubValidationGateOrchestrator {
        deny: bool,
        dependency_unavailable: bool,
    }

    impl ValidationGatePolicyOrchestrator for StubValidationGateOrchestrator {
        fn upsert_validation_gate_policy(
            &self,
            _input: UpsertValidationGatePolicyInput,
        ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError> {
            unreachable!("not used in workflow runs tests")
        }

        fn read_validation_gate_policy(
            &self,
            _input: ReadValidationGatePolicyInput,
        ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError> {
            unreachable!("not used in workflow runs tests")
        }

        fn list_validation_gate_policies(
            &self,
            _input: ListValidationGatePoliciesInput,
        ) -> Result<Vec<ValidationGatePolicyEvidence>, ValidationGatePolicyServiceError> {
            unreachable!("not used in workflow runs tests")
        }

        fn evaluate_validation_gates(
            &self,
            input: EvaluateValidationGatePoliciesInput,
        ) -> Result<ValidationGateEvaluationEvidence, ValidationGatePolicyServiceError> {
            if self.dependency_unavailable {
                return Err(ValidationGatePolicyServiceError {
                    code: ValidationGateReasonCode::DependencyUnavailable.code(),
                    message: "gate dependency unavailable".to_string(),
                    field_errors: Vec::new(),
                    failed_gate_ids: vec!["fr43::forward-bias::primary".to_string()],
                });
            }
            if self.deny {
                return Err(ValidationGatePolicyServiceError {
                    code: ValidationGateReasonCode::GateFailed.code(),
                    message: "gate threshold comparison denied progression".to_string(),
                    field_errors: Vec::new(),
                    failed_gate_ids: vec!["fr43::data-leakage::primary".to_string()],
                });
            }
            Ok(ValidationGateEvaluationEvidence {
                candidate_id: input.candidate_id,
                stage: "training".to_string(),
                outcome: "allow".to_string(),
                reason_code: ValidationGateReasonCode::EvaluationAllowed
                    .code()
                    .to_string(),
                failed_gate_ids: Vec::new(),
                gate_results: vec![ValidationGateEvaluationResultEvidence {
                    policy_key: "fr43::forward-bias::primary".to_string(),
                    gate_type: "forward_bias".to_string(),
                    metric_key: "forward_bias_score".to_string(),
                    comparator: "lte".to_string(),
                    threshold_value: 0.12,
                    observed_value: Some(0.1),
                    passed: true,
                    reason_code: ValidationGateReasonCode::EvaluationAllowed
                        .code()
                        .to_string(),
                }],
                actor_id: input.actor_id,
                correlation_id: input.correlation_id,
                evaluated_at_utc: input.evaluated_at_utc,
            })
        }
    }

    fn service_with_gate_orchestrator(
        gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>,
    ) -> ValidationWorkflowRunService {
        ValidationWorkflowRunService::new(
            Arc::new(InMemoryValidationWorkflowRepository::default()),
            gate_orchestrator,
            Arc::new(DeterministicValidationStageExecutor),
        )
    }

    fn sample_stage_inputs(force_fail_stage: Option<&str>) -> Value {
        json!({
            "quality": {
                "out_of_sample_sharpe": 1.2,
                "max_drawdown": -0.15,
                "brier_score": 0.11,
                "overfit_indicator": 0.2,
                "force_fail": force_fail_stage == Some("quality")
            },
            "labeling": {
                "out_of_sample_sharpe": 1.25,
                "max_drawdown": -0.14,
                "brier_score": 0.10,
                "overfit_indicator": 0.21,
                "force_fail": force_fail_stage == Some("labeling")
            },
            "purged_cv": {
                "out_of_sample_sharpe": 1.30,
                "max_drawdown": -0.13,
                "brier_score": 0.09,
                "overfit_indicator": 0.22,
                "force_fail": force_fail_stage == Some("purged_cv")
            },
            "cpcv": {
                "out_of_sample_sharpe": 1.35,
                "max_drawdown": -0.12,
                "brier_score": 0.08,
                "overfit_indicator": 0.23,
                "force_fail": force_fail_stage == Some("cpcv")
            },
            "overfit_diagnostics": {
                "out_of_sample_sharpe": 1.40,
                "max_drawdown": -0.11,
                "brier_score": 0.07,
                "overfit_indicator": 0.24,
                "force_fail": force_fail_stage == Some("overfit_diagnostics")
            }
        })
    }

    fn sample_start_input(candidate_id: &str) -> StartValidationRunInput {
        StartValidationRunInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            candidate_id: candidate_id.to_string(),
            training_entry_observed_metrics: json!({
                "forward_bias_score": 0.10,
                "data_leakage_score": 0.19,
                "regime_survivability_score": 0.85,
                "data_quality_score": 0.97
            }),
            stage_inputs: sample_stage_inputs(None),
            correlation_id: "corr-validation-run-001".to_string(),
            requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn validation_run_start_enforces_deterministic_stage_order_and_persists_artifacts() {
        let service =
            service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator::default()));

        let result = service
            .start_validation_run(sample_start_input("candidate::alpha-1"))
            .expect("validation run should complete");
        assert_eq!(result.run.run_state, ValidationWorkflowRunState::Completed);
        assert_eq!(result.artifacts.len(), 5);
        assert_eq!(result.artifacts[0].stage, ValidationWorkflowStage::Quality);
        assert_eq!(
            result.artifacts[4].stage,
            ValidationWorkflowStage::OverfitDiagnostics
        );
    }

    #[test]
    fn validation_run_start_fails_closed_when_training_gate_denies_entry() {
        let service = service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator {
            deny: true,
            dependency_unavailable: false,
        }));

        let error = service
            .start_validation_run(sample_start_input("candidate::alpha-2"))
            .expect_err("gate denial should fail closed");
        assert_eq!(error.code, ValidationWorkflowReasonCode::GateDenied.code());
    }

    #[test]
    fn validation_run_start_blocks_downstream_stages_on_stage_failure() {
        let service =
            service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator::default()));
        let mut input = sample_start_input("candidate::alpha-3");
        input.stage_inputs = sample_stage_inputs(Some("purged_cv"));

        let result = service
            .start_validation_run(input)
            .expect("stage failure should still persist run evidence");
        assert_eq!(result.run.run_state, ValidationWorkflowRunState::Failed);
        assert_eq!(
            result.run.reason_code,
            ValidationWorkflowReasonCode::StageFailed.code()
        );
        assert_eq!(result.artifacts.len(), 3);
        assert_eq!(
            result
                .artifacts
                .last()
                .expect("failed stage artifact should exist")
                .stage_outcome,
            ValidationWorkflowStageOutcome::Failed
        );
    }

    #[test]
    fn validation_run_start_marks_missing_stage_input_as_dependency_unavailable() {
        let service =
            service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator::default()));
        let mut input = sample_start_input("candidate::alpha-4");
        input.stage_inputs = json!({
            "quality": {
                "out_of_sample_sharpe": 1.2,
                "max_drawdown": -0.15,
                "brier_score": 0.11,
                "overfit_indicator": 0.2
            }
        });

        let result = service
            .start_validation_run(input)
            .expect("missing stage inputs should produce blocked run evidence");
        assert_eq!(result.run.run_state, ValidationWorkflowRunState::Blocked);
        assert_eq!(
            result.run.reason_code,
            ValidationWorkflowReasonCode::DependencyUnavailable.code()
        );
    }

    #[test]
    fn validation_run_read_and_artifact_lookup_return_persisted_evidence() {
        let service =
            service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator::default()));
        let started = service
            .start_validation_run(sample_start_input("candidate::alpha-5"))
            .expect("seed run should complete");

        let read_result = service
            .read_validation_run(ReadValidationRunInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                run_id: started.run.run_id.clone(),
                correlation_id: "corr-validation-run-read-001".to_string(),
                queried_at_utc: "2026-04-07T00:10:00Z".to_string(),
            })
            .expect("read should succeed");
        assert_eq!(
            read_result.reason_code,
            ValidationWorkflowReasonCode::RunRead.code()
        );
        assert_eq!(read_result.artifacts.len(), 5);

        let artifact = service
            .read_validation_artifact(ReadValidationArtifactInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                run_id: started.run.run_id,
                stage: "quality".to_string(),
                correlation_id: "corr-validation-artifact-read-001".to_string(),
                queried_at_utc: "2026-04-07T00:10:10Z".to_string(),
            })
            .expect("artifact lookup should succeed");
        assert_eq!(artifact.artifact.stage, ValidationWorkflowStage::Quality);
        assert_eq!(
            artifact.reason_code,
            ValidationWorkflowReasonCode::ArtifactRead.code()
        );
    }

    #[test]
    fn validation_run_read_builds_deterministic_comparison_against_previous_completed_run() {
        let service =
            service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator::default()));

        service
            .start_validation_run(sample_start_input("candidate::alpha-6"))
            .expect("first run should complete");
        let second = service
            .start_validation_run(StartValidationRunInput {
                correlation_id: "corr-validation-run-002".to_string(),
                requested_at_utc: "2026-04-07T00:20:00Z".to_string(),
                ..sample_start_input("candidate::alpha-6")
            })
            .expect("second run should complete");

        let read = service
            .read_validation_run(ReadValidationRunInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                run_id: second.run.run_id,
                correlation_id: "corr-validation-run-read-002".to_string(),
                queried_at_utc: "2026-04-07T00:25:00Z".to_string(),
            })
            .expect("comparison-capable read should succeed");
        assert!(!read.comparisons.is_empty());
        assert_eq!(
            read.comparisons[0].metric_deltas[0].metric_key,
            "out_of_sample_sharpe"
        );
        assert_eq!(
            read.comparisons[0].reason_code,
            ValidationWorkflowReasonCode::ComparisonReady.code()
        );
    }

    #[test]
    fn validation_run_read_does_not_compare_against_newer_runs() {
        let service =
            service_with_gate_orchestrator(Arc::new(StubValidationGateOrchestrator::default()));

        let first = service
            .start_validation_run(sample_start_input("candidate::alpha-7"))
            .expect("first run should complete");
        service
            .start_validation_run(StartValidationRunInput {
                correlation_id: "corr-validation-run-003".to_string(),
                requested_at_utc: "2026-04-07T00:20:00Z".to_string(),
                ..sample_start_input("candidate::alpha-7")
            })
            .expect("second run should complete");

        let read_first = service
            .read_validation_run(ReadValidationRunInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                run_id: first.run.run_id,
                correlation_id: "corr-validation-run-read-003".to_string(),
                queried_at_utc: "2026-04-07T00:25:00Z".to_string(),
            })
            .expect("read should succeed");
        assert!(
            read_first.comparisons.is_empty(),
            "older runs must not compare against newer baselines"
        );
    }
}
