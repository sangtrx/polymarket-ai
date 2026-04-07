#[cfg(test)]
use domain::research::FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS;
use domain::research::{
    CounterfactualReplayContractError, CounterfactualReplayGateOutcome,
    CounterfactualReplayReasonCode, CounterfactualReplayRunRecord, CounterfactualReplayRunState,
    CounterfactualReplayScenarioKind, CounterfactualReplayScenarioResult,
    CounterfactualReplaySummary, CounterfactualReplayValidationIssue, ShadowEvaluationState,
    FR46_DEGRADATION_DENY_THRESHOLD_PCT, FR46_DELAYED_EXIT_SECONDS,
    FR46_STRESSED_SLIPPAGE_MULTIPLIER, ShadowEvaluationRecord, ShadowSimulationDecisionSide,
    ShadowSimulationOutcome, ValidationWorkflowArtifactRecord, ValidationWorkflowRunRecord,
    ValidationWorkflowRunState, canonicalize_counterfactual_replay_run_record,
    compose_counterfactual_replay_run_id, evaluate_counterfactual_replay_gate,
    fr46_scenario_parameters, normalize_research_identifier,
    parse_counterfactual_replay_utc_timestamp,
};
use persistence::postgres::counterfactual_replay_runs::{
    CounterfactualReplayPersistenceError,
    list_counterfactual_replay_runs_by_candidate as pg_list_counterfactual_replay_runs_by_candidate,
    load_counterfactual_replay_run as pg_load_counterfactual_replay_run,
    upsert_counterfactual_replay_run as pg_upsert_counterfactual_replay_run,
};
use persistence::postgres::shadow_evaluations::{
    ShadowEvaluationPersistenceError,
    load_latest_shadow_evaluation_by_candidate_and_run as pg_load_latest_shadow_evaluation_by_candidate_and_run,
};
use persistence::postgres::validation_artifacts::{
    ValidationArtifactPersistenceError,
    list_validation_artifacts_by_run as pg_list_validation_artifacts_by_run,
};
use persistence::postgres::validation_runs::{
    ValidationRunPersistenceError, load_validation_run as pg_load_validation_run,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

const DEFAULT_LIST_LIMIT: i64 = 25;
const MAX_LIST_LIMIT: i64 = 200;
const STRESS_FILL_RATE_PENALTY_COEFFICIENT: f64 = 0.08;
const DELAY_EXIT_PENALTY_COEFFICIENT: f64 = 0.02;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CounterfactualReplayServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<CounterfactualReplayValidationIssue>,
}

impl CounterfactualReplayServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<CounterfactualReplayValidationIssue>,
    ) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: CounterfactualReplayReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for counterfactual replay mutations".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: CounterfactualReplayReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for counterfactual replay reads".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn run_not_found(run_id: &str) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::RunNotFound.code(),
            message: format!("counterfactual replay run `{run_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for CounterfactualReplayServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for CounterfactualReplayServiceError {}

#[derive(Debug, Clone)]
pub struct StartCounterfactualReplayInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub validation_run_id: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadCounterfactualReplayInput {
    pub actor_id: String,
    pub actor_role: String,
    pub replay_run_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListCounterfactualReplayRunsInput {
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
pub struct CounterfactualReplayEvidence {
    pub replay_run: CounterfactualReplayRunRecord,
    pub reason_code: String,
}

pub trait CounterfactualReplayOrchestrator: Send + Sync {
    fn start_counterfactual_replay(
        &self,
        input: StartCounterfactualReplayInput,
    ) -> Result<CounterfactualReplayEvidence, CounterfactualReplayServiceError>;

    fn read_counterfactual_replay(
        &self,
        input: ReadCounterfactualReplayInput,
    ) -> Result<CounterfactualReplayEvidence, CounterfactualReplayServiceError>;

    fn list_counterfactual_replay_runs(
        &self,
        input: ListCounterfactualReplayRunsInput,
    ) -> Result<Vec<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError>;
}

pub trait CounterfactualReplayRepositoryPort: Send + Sync {
    fn upsert(
        &self,
        record: CounterfactualReplayRunRecord,
    ) -> Result<(), CounterfactualReplayServiceError>;
    fn load(
        &self,
        run_id: &str,
    ) -> Result<Option<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError>;
    fn list_by_candidate(
        &self,
        candidate_id: &str,
        started_after_utc: Option<&str>,
        started_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError>;
}

pub trait ValidationEvidencePort: Send + Sync {
    fn load_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, CounterfactualReplayServiceError>;
    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, CounterfactualReplayServiceError>;
}

pub trait ShadowEvidencePort: Send + Sync {
    fn read_latest_shadow_evaluation(
        &self,
        candidate_id: &str,
        validation_run_id: &str,
    ) -> Result<Option<ShadowEvaluationRecord>, CounterfactualReplayServiceError>;
}

#[derive(Clone)]
pub struct CounterfactualReplayService {
    repository: Arc<dyn CounterfactualReplayRepositoryPort>,
    validation_evidence: Arc<dyn ValidationEvidencePort>,
    shadow_evidence: Arc<dyn ShadowEvidencePort>,
    operation_lock: Arc<Mutex<()>>,
}

impl CounterfactualReplayService {
    pub fn new(
        repository: Arc<dyn CounterfactualReplayRepositoryPort>,
        validation_evidence: Arc<dyn ValidationEvidencePort>,
        shadow_evidence: Arc<dyn ShadowEvidencePort>,
    ) -> Self {
        Self {
            repository,
            validation_evidence,
            shadow_evidence,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryCounterfactualReplayRepository::default()),
            Arc::new(StaticValidationEvidencePort::default()),
            Arc::new(StaticShadowEvidencePort::default()),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresCounterfactualReplayRepository::new(pool.clone())),
            Arc::new(PostgresValidationEvidencePort::new(pool.clone())),
            Arc::new(PostgresShadowEvidencePort::new(pool)),
        )
    }

    pub fn with_validation_evidence_port(
        mut self,
        validation_evidence: Arc<dyn ValidationEvidencePort>,
    ) -> Self {
        self.validation_evidence = validation_evidence;
        self
    }

    pub fn with_shadow_evidence_port(
        mut self,
        shadow_evidence: Arc<dyn ShadowEvidencePort>,
    ) -> Self {
        self.shadow_evidence = shadow_evidence;
        self
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, CounterfactualReplayServiceError> {
        self.operation_lock.lock().map_err(|_| {
            CounterfactualReplayServiceError::persistence_unavailable(
                "counterfactual replay operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for CounterfactualReplayService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl CounterfactualReplayOrchestrator for CounterfactualReplayService {
    fn start_counterfactual_replay(
        &self,
        input: StartCounterfactualReplayInput,
    ) -> Result<CounterfactualReplayEvidence, CounterfactualReplayServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_counterfactual_replay_telemetry(
                "counterfactual_replay_start_v1",
                "counterfactual_replay_start",
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
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("candidate_id", &input.candidate_id)?;
        validate_non_empty("validation_run_id", &input.validation_run_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("requested_at_utc", &input.requested_at_utc)?;
        validate_utc_timestamp("requested_at_utc", &input.requested_at_utc)?;

        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            return Err(CounterfactualReplayServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![CounterfactualReplayValidationIssue {
                    field: "candidate_id".to_string(),
                    code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            ));
        }
        let normalized_validation_run_id = normalize_research_identifier(&input.validation_run_id);
        let replay_run_id =
            compose_counterfactual_replay_run_id(&normalized_candidate_id, &input.requested_at_utc)
                .map_err(map_contract_error)?;
        let _requested_at = parse_counterfactual_replay_utc_timestamp(&input.requested_at_utc)
            .map_err(map_contract_error)?;
        let _lock = self.lock_operations()?;

        let validation_run = self
            .validation_evidence
            .load_validation_run(&normalized_validation_run_id)?;
        let Some(validation_run) = validation_run else {
            return Err(CounterfactualReplayServiceError::state_unavailable(
                "validation_run_id must reference a completed Story 6.3 validation run",
            ));
        };
        if validation_run.run_state != ValidationWorkflowRunState::Completed
            || validation_run.candidate_id != normalized_candidate_id
        {
            return Err(CounterfactualReplayServiceError::state_unavailable(
                "validation_run_id must match candidate_id and be completed",
            ));
        }

        let validation_artifacts = self
            .validation_evidence
            .list_validation_artifacts_by_run(&normalized_validation_run_id)?;
        if validation_artifacts.is_empty() {
            return Err(CounterfactualReplayServiceError::state_unavailable(
                "validation artifacts are required for counterfactual replay",
            ));
        }

        let shadow_evaluation = self.shadow_evidence.read_latest_shadow_evaluation(
            &normalized_candidate_id,
            &normalized_validation_run_id,
        )?;
        let Some(shadow_evaluation) = shadow_evaluation else {
            return Err(CounterfactualReplayServiceError::state_unavailable(
                "shadow evaluation evidence is required for counterfactual replay",
            ));
        };
        if shadow_evaluation.evaluation_state != ShadowEvaluationState::Completed {
            return Err(CounterfactualReplayServiceError::state_unavailable(
                "shadow evaluation must be completed before counterfactual replay",
            ));
        }

        let baseline_net_pnl = compute_baseline_net_pnl(&shadow_evaluation.simulation_outcomes)?;
        let average_slippage_bps =
            compute_average_slippage_bps(&shadow_evaluation.simulation_outcomes)?;
        let stress_penalty_pct = ((FR46_STRESSED_SLIPPAGE_MULTIPLIER - 1.0)
            * (average_slippage_bps / 10_000.0))
            + ((1.0 - domain::research::FR46_STRESSED_FILL_RATE_MULTIPLIER)
                * STRESS_FILL_RATE_PENALTY_COEFFICIENT);
        let delayed_exit_penalty_pct =
            (FR46_DELAYED_EXIT_SECONDS as f64 / 60.0) * DELAY_EXIT_PENALTY_COEFFICIENT;
        let stressed_net_pnl = baseline_net_pnl * (1.0 - stress_penalty_pct);
        let delayed_exit_net_pnl = baseline_net_pnl * (1.0 - delayed_exit_penalty_pct);
        let gate_evaluation =
            evaluate_counterfactual_replay_gate(baseline_net_pnl, stressed_net_pnl)
                .map_err(map_contract_error)?;
        let delayed_exit_degradation_pct =
            ((delayed_exit_net_pnl - baseline_net_pnl) / baseline_net_pnl) * 100.0;

        let scenarios = vec![
            CounterfactualReplayScenarioResult {
                scenario: CounterfactualReplayScenarioKind::Baseline,
                net_pnl: baseline_net_pnl,
                degradation_pct: Some(0.0),
                gate_outcome: CounterfactualReplayGateOutcome::Allow,
                reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                    .code()
                    .to_string(),
                parameters: fr46_scenario_parameters(CounterfactualReplayScenarioKind::Baseline),
            },
            CounterfactualReplayScenarioResult {
                scenario: CounterfactualReplayScenarioKind::StressedExecution,
                net_pnl: stressed_net_pnl,
                degradation_pct: Some(gate_evaluation.degradation_pct),
                gate_outcome: gate_evaluation.gate_outcome,
                reason_code: gate_evaluation.reason_code.clone(),
                parameters: fr46_scenario_parameters(
                    CounterfactualReplayScenarioKind::StressedExecution,
                ),
            },
            CounterfactualReplayScenarioResult {
                scenario: CounterfactualReplayScenarioKind::DelayedExit,
                net_pnl: delayed_exit_net_pnl,
                degradation_pct: Some(delayed_exit_degradation_pct),
                gate_outcome: CounterfactualReplayGateOutcome::Allow,
                reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                    .code()
                    .to_string(),
                parameters: fr46_scenario_parameters(CounterfactualReplayScenarioKind::DelayedExit),
            },
        ];

        let run_state = if gate_evaluation.gate_outcome == CounterfactualReplayGateOutcome::Deny {
            CounterfactualReplayRunState::Denied
        } else {
            CounterfactualReplayRunState::Completed
        };
        let reason_code = gate_evaluation.reason_code.clone();
        let replay_summary = CounterfactualReplaySummary {
            run_id: replay_run_id.clone(),
            gate_outcome: gate_evaluation.gate_outcome,
            reason_code: reason_code.clone(),
            baseline_net_pnl,
            stressed_net_pnl,
            delayed_exit_net_pnl,
            degradation_pct: gate_evaluation.degradation_pct,
            tolerance_threshold_pct: FR46_DEGRADATION_DENY_THRESHOLD_PCT,
            scenarios: scenarios.clone(),
        };
        let replay_run =
            canonicalize_counterfactual_replay_run_record(&CounterfactualReplayRunRecord {
                run_id: replay_run_id.clone(),
                candidate_id: normalized_candidate_id.clone(),
                validation_run_id: normalized_validation_run_id,
                run_state,
                reason_code: reason_code.clone(),
                scenario_results: scenarios,
                replay_summary,
                actor_id: input.actor_id.clone(),
                correlation_id: input.correlation_id.clone(),
                started_at_utc: input.requested_at_utc.clone(),
                completed_at_utc: Some(input.requested_at_utc.clone()),
            })
            .map_err(map_contract_error)?;

        self.repository.upsert(replay_run.clone())?;
        emit_counterfactual_replay_telemetry(
            "counterfactual_replay_start_v1",
            "counterfactual_replay_start",
            if replay_run.run_state == CounterfactualReplayRunState::Completed {
                "allow"
            } else {
                "deny"
            },
            &input.actor_id,
            &replay_run.candidate_id,
            Some(&replay_run.run_id),
            &replay_run.reason_code,
            &replay_run.correlation_id,
            &replay_run.started_at_utc,
        );

        Ok(CounterfactualReplayEvidence {
            replay_run,
            reason_code: CounterfactualReplayReasonCode::RunStarted
                .code()
                .to_string(),
        })
    }

    fn read_counterfactual_replay(
        &self,
        input: ReadCounterfactualReplayInput,
    ) -> Result<CounterfactualReplayEvidence, CounterfactualReplayServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_counterfactual_replay_telemetry(
                "counterfactual_replay_read_v1",
                "counterfactual_replay_read",
                "deny",
                &input.actor_id,
                "unknown_candidate_id",
                Some(&input.replay_run_id),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("replay_run_id", &input.replay_run_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        let normalized_run_id = normalize_research_identifier(&input.replay_run_id);
        let Some(replay_run) = self.repository.load(&normalized_run_id)? else {
            return Err(CounterfactualReplayServiceError::run_not_found(
                &normalized_run_id,
            ));
        };

        emit_counterfactual_replay_telemetry(
            "counterfactual_replay_read_v1",
            "counterfactual_replay_read",
            "allow",
            &input.actor_id,
            &replay_run.candidate_id,
            Some(&replay_run.run_id),
            CounterfactualReplayReasonCode::RunRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(CounterfactualReplayEvidence {
            replay_run,
            reason_code: CounterfactualReplayReasonCode::RunRead.code().to_string(),
        })
    }

    fn list_counterfactual_replay_runs(
        &self,
        input: ListCounterfactualReplayRunsInput,
    ) -> Result<Vec<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_counterfactual_replay_telemetry(
                "counterfactual_replay_list_v1",
                "counterfactual_replay_list",
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
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("candidate_id", &input.candidate_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        if let Some(limit) = input.limit
            && limit <= 0
        {
            return Err(CounterfactualReplayServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![CounterfactualReplayValidationIssue {
                    field: "limit".to_string(),
                    code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_started_after =
            normalize_optional_timestamp("started_after_utc", input.started_after_utc.as_deref())?;
        let normalized_started_before = normalize_optional_timestamp(
            "started_before_utc",
            input.started_before_utc.as_deref(),
        )?;
        if let (Some(started_after), Some(started_before)) = (
            normalized_started_after.as_deref(),
            normalized_started_before.as_deref(),
        ) {
            let started_after_ts = parse_counterfactual_replay_utc_timestamp(started_after)
                .map_err(map_contract_error)?;
            let started_before_ts = parse_counterfactual_replay_utc_timestamp(started_before)
                .map_err(map_contract_error)?;
            if started_before_ts <= started_after_ts {
                return Err(CounterfactualReplayServiceError::invalid_payload(
                    "started_before_utc must be greater than started_after_utc",
                    vec![CounterfactualReplayValidationIssue {
                        field: "started_before_utc".to_string(),
                        code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                        message: "started_before_utc must be greater than started_after_utc"
                            .to_string(),
                    }],
                ));
            }
        }

        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            return Err(CounterfactualReplayServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![CounterfactualReplayValidationIssue {
                    field: "candidate_id".to_string(),
                    code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            ));
        }
        let limit = input
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT);
        let replay_runs = self.repository.list_by_candidate(
            &normalized_candidate_id,
            normalized_started_after.as_deref(),
            normalized_started_before.as_deref(),
            limit,
        )?;
        emit_counterfactual_replay_telemetry(
            "counterfactual_replay_list_v1",
            "counterfactual_replay_list",
            "allow",
            &input.actor_id,
            &normalized_candidate_id,
            None,
            CounterfactualReplayReasonCode::RunListed.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(replay_runs)
    }
}

fn compute_baseline_net_pnl(
    outcomes: &[ShadowSimulationOutcome],
) -> Result<f64, CounterfactualReplayServiceError> {
    if outcomes.is_empty() {
        return Err(CounterfactualReplayServiceError::state_unavailable(
            "shadow simulation outcomes are required to compute baseline net pnl",
        ));
    }
    let baseline_net_pnl = outcomes
        .iter()
        .map(|outcome| {
            let side_multiplier = match outcome.decision_side {
                ShadowSimulationDecisionSide::Hold => 0.0,
                ShadowSimulationDecisionSide::Buy | ShadowSimulationDecisionSide::Sell => 1.0,
            };
            let gross_edge = (1.0 - outcome.simulated_fill_price) * outcome.simulated_fill_size;
            let slippage_penalty =
                outcome.simulated_fill_size * (outcome.simulated_slippage_bps / 10_000.0);
            side_multiplier * (gross_edge - slippage_penalty)
        })
        .sum::<f64>();
    if !baseline_net_pnl.is_finite() {
        return Err(CounterfactualReplayServiceError::state_unavailable(
            "baseline net pnl is non-finite",
        ));
    }
    Ok(baseline_net_pnl)
}

fn compute_average_slippage_bps(
    outcomes: &[ShadowSimulationOutcome],
) -> Result<f64, CounterfactualReplayServiceError> {
    if outcomes.is_empty() {
        return Err(CounterfactualReplayServiceError::state_unavailable(
            "shadow simulation outcomes are required to compute stressed scenario",
        ));
    }
    let count = outcomes.len() as f64;
    let avg = outcomes
        .iter()
        .map(|outcome| outcome.simulated_slippage_bps)
        .sum::<f64>()
        / count;
    if !avg.is_finite() {
        return Err(CounterfactualReplayServiceError::state_unavailable(
            "average slippage is non-finite",
        ));
    }
    Ok(avg)
}

fn map_contract_error(
    error: CounterfactualReplayContractError,
) -> CounterfactualReplayServiceError {
    CounterfactualReplayServiceError::invalid_payload(error.message, error.field_errors)
}

fn validate_mutation_role(role: &str) -> Result<(), CounterfactualReplayServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(CounterfactualReplayServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), CounterfactualReplayServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(CounterfactualReplayServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), CounterfactualReplayServiceError> {
    if value.trim().is_empty() {
        return Err(CounterfactualReplayServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![CounterfactualReplayValidationIssue {
                field: field.to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn validate_utc_timestamp(
    field: &str,
    value: &str,
) -> Result<(), CounterfactualReplayServiceError> {
    parse_counterfactual_replay_utc_timestamp(value).map_err(|_| {
        CounterfactualReplayServiceError::invalid_payload(
            format!("{field} must be RFC3339 UTC"),
            vec![CounterfactualReplayValidationIssue {
                field: field.to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: format!("{field} must be RFC3339 UTC"),
            }],
        )
    })?;
    Ok(())
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, CounterfactualReplayServiceError> {
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
fn emit_counterfactual_replay_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    candidate_id: &str,
    run_id: Option<&str>,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = CounterfactualReplayTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        candidate_id,
        run_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(CounterfactualReplaySecuritySignal {
                name: "counterfactual_replay_denied_v1",
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
        serde_json::to_string(&event).expect("counterfactual replay telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct CounterfactualReplayTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    candidate_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<&'a str>,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<CounterfactualReplaySecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct CounterfactualReplaySecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresCounterfactualReplayRepository {
    pool: PgPool,
}

impl PostgresCounterfactualReplayRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, CounterfactualReplayServiceError>
    where
        F: Future<Output = Result<T, CounterfactualReplayPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_counterfactual_replay_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    CounterfactualReplayServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_counterfactual_replay_persistence_error),
        }
    }
}

impl CounterfactualReplayRepositoryPort for PostgresCounterfactualReplayRepository {
    fn upsert(
        &self,
        record: CounterfactualReplayRunRecord,
    ) -> Result<(), CounterfactualReplayServiceError> {
        self.run_future(pg_upsert_counterfactual_replay_run(&self.pool, &record))
    }

    fn load(
        &self,
        run_id: &str,
    ) -> Result<Option<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError> {
        self.run_future(pg_load_counterfactual_replay_run(&self.pool, run_id))
    }

    fn list_by_candidate(
        &self,
        candidate_id: &str,
        started_after_utc: Option<&str>,
        started_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError> {
        self.run_future(pg_list_counterfactual_replay_runs_by_candidate(
            &self.pool,
            candidate_id,
            started_after_utc,
            started_before_utc,
            limit,
        ))
    }
}

fn map_counterfactual_replay_persistence_error(
    error: CounterfactualReplayPersistenceError,
) -> CounterfactualReplayServiceError {
    match error.code {
        "counterfactual_replay_run_query_failed"
        | "counterfactual_replay_run_row_decode_failed" => {
            CounterfactualReplayServiceError::persistence_unavailable(error.message)
        }
        _ => CounterfactualReplayServiceError {
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

    fn run_validation_run_future<F, T>(
        &self,
        future: F,
    ) -> Result<T, CounterfactualReplayServiceError>
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
                    CounterfactualReplayServiceError::dependency_unavailable(format!(
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
    ) -> Result<T, CounterfactualReplayServiceError>
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
                    CounterfactualReplayServiceError::dependency_unavailable(format!(
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
    ) -> Result<Option<ValidationWorkflowRunRecord>, CounterfactualReplayServiceError> {
        self.run_validation_run_future(pg_load_validation_run(&self.pool, run_id))
    }

    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, CounterfactualReplayServiceError> {
        self.run_validation_artifact_future(pg_list_validation_artifacts_by_run(&self.pool, run_id))
    }
}

fn map_validation_run_error(
    error: ValidationRunPersistenceError,
) -> CounterfactualReplayServiceError {
    match error.code {
        "validation_run_query_failed" => {
            CounterfactualReplayServiceError::dependency_unavailable(error.message)
        }
        "validation_run_row_decode_failed" => {
            CounterfactualReplayServiceError::state_unavailable(error.message)
        }
        _ => CounterfactualReplayServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| CounterfactualReplayValidationIssue {
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
) -> CounterfactualReplayServiceError {
    match error.code {
        "validation_artifact_query_failed" => {
            CounterfactualReplayServiceError::dependency_unavailable(error.message)
        }
        "validation_artifact_row_decode_failed" => {
            CounterfactualReplayServiceError::state_unavailable(error.message)
        }
        _ => CounterfactualReplayServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| CounterfactualReplayValidationIssue {
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

    fn run_shadow_future<F, T>(&self, future: F) -> Result<T, CounterfactualReplayServiceError>
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
                    CounterfactualReplayServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_shadow_persistence_error),
        }
    }
}

impl ShadowEvidencePort for PostgresShadowEvidencePort {
    fn read_latest_shadow_evaluation(
        &self,
        candidate_id: &str,
        validation_run_id: &str,
    ) -> Result<Option<ShadowEvaluationRecord>, CounterfactualReplayServiceError> {
        self.run_shadow_future(pg_load_latest_shadow_evaluation_by_candidate_and_run(
            &self.pool,
            candidate_id,
            validation_run_id,
        ))
    }
}

fn map_shadow_persistence_error(
    error: ShadowEvaluationPersistenceError,
) -> CounterfactualReplayServiceError {
    match error.code {
        "shadow_evaluation_query_failed" => {
            CounterfactualReplayServiceError::dependency_unavailable(error.message)
        }
        "shadow_evaluation_row_decode_failed" => {
            CounterfactualReplayServiceError::state_unavailable(error.message)
        }
        _ => CounterfactualReplayServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| CounterfactualReplayValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

#[derive(Debug, Default)]
pub struct InMemoryCounterfactualReplayRepository {
    records: Mutex<BTreeMap<String, CounterfactualReplayRunRecord>>,
}

impl CounterfactualReplayRepositoryPort for InMemoryCounterfactualReplayRepository {
    fn upsert(
        &self,
        record: CounterfactualReplayRunRecord,
    ) -> Result<(), CounterfactualReplayServiceError> {
        let canonical =
            canonicalize_counterfactual_replay_run_record(&record).map_err(map_contract_error)?;
        let mut records = self.records.lock().map_err(|_| {
            CounterfactualReplayServiceError::persistence_unavailable(
                "in-memory replay run store lock poisoned",
            )
        })?;
        records.insert(canonical.run_id.clone(), canonical);
        Ok(())
    }

    fn load(
        &self,
        run_id: &str,
    ) -> Result<Option<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        let records = self.records.lock().map_err(|_| {
            CounterfactualReplayServiceError::persistence_unavailable(
                "in-memory replay run store lock poisoned",
            )
        })?;
        records
            .get(&normalized_run_id)
            .cloned()
            .map(|record| {
                canonicalize_counterfactual_replay_run_record(&record).map_err(map_contract_error)
            })
            .transpose()
    }

    fn list_by_candidate(
        &self,
        candidate_id: &str,
        started_after_utc: Option<&str>,
        started_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CounterfactualReplayRunRecord>, CounterfactualReplayServiceError> {
        if limit <= 0 {
            return Err(CounterfactualReplayServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![CounterfactualReplayValidationIssue {
                    field: "limit".to_string(),
                    code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_candidate_id = normalize_research_identifier(candidate_id);
        let started_after = started_after_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                parse_counterfactual_replay_utc_timestamp(value).map_err(map_contract_error)
            })
            .transpose()?;
        let started_before = started_before_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                parse_counterfactual_replay_utc_timestamp(value).map_err(map_contract_error)
            })
            .transpose()?;
        let records = self.records.lock().map_err(|_| {
            CounterfactualReplayServiceError::persistence_unavailable(
                "in-memory replay run store lock poisoned",
            )
        })?;
        let mut replay_runs = records
            .values()
            .filter(|record| record.candidate_id == normalized_candidate_id)
            .cloned()
            .collect::<Vec<_>>();
        replay_runs.sort_by(|left, right| {
            right
                .started_at_utc
                .cmp(&left.started_at_utc)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        let filtered = replay_runs
            .into_iter()
            .filter(|record| {
                let started_at = parse_counterfactual_replay_utc_timestamp(&record.started_at_utc);
                if let Ok(started_at) = started_at {
                    let after_ok = started_after
                        .map(|after| started_at >= after)
                        .unwrap_or(true);
                    let before_ok = started_before
                        .map(|before| started_at < before)
                        .unwrap_or(true);
                    after_ok && before_ok
                } else {
                    false
                }
            })
            .take(limit as usize)
            .map(|record| {
                canonicalize_counterfactual_replay_run_record(&record).map_err(map_contract_error)
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
            gate_evaluation: serde_json::json!({
                "reason_code": "validation_gate_evaluation_allowed",
                "outcome": "allow",
            }),
            comparison_ready: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-validation-001".to_string(),
            started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:05:00Z".to_string()),
        };
        let artifacts = vec![ValidationWorkflowArtifactRecord {
            artifact_id: format!("{}::quality", run.run_id),
            run_id: run.run_id.clone(),
            candidate_id: run.candidate_id.clone(),
            stage: domain::research::ValidationWorkflowStage::Quality,
            stage_index: 1,
            stage_outcome: domain::research::ValidationWorkflowStageOutcome::Passed,
            reason_code: "validation_run_stage_passed".to_string(),
            diagnostics: domain::research::ValidationDiagnosticsPayload {
                out_of_sample_sharpe: 1.22,
                max_drawdown: -0.18,
                brier_score: Some(0.12),
                expected_calibration_error: None,
                overfit_indicator: 0.19,
                overfit_flag: false,
            },
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-validation-001".to_string(),
            stage_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            stage_completed_at_utc: "2026-04-07T00:01:00Z".to_string(),
        }];
        Self { run, artifacts }
    }
}

impl ValidationEvidencePort for StaticValidationEvidencePort {
    fn load_validation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ValidationWorkflowRunRecord>, CounterfactualReplayServiceError> {
        let normalized_run_id = normalize_research_identifier(run_id);
        Ok((self.run.run_id == normalized_run_id).then_some(self.run.clone()))
    }

    fn list_validation_artifacts_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<ValidationWorkflowArtifactRecord>, CounterfactualReplayServiceError> {
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
pub struct StaticShadowEvidencePort {
    pub slippage_bps: f64,
    pub simulated_fill_price: f64,
}

impl Default for StaticShadowEvidencePort {
    fn default() -> Self {
        Self {
            slippage_bps: 5.0,
            simulated_fill_price: 0.43,
        }
    }
}

impl ShadowEvidencePort for StaticShadowEvidencePort {
    fn read_latest_shadow_evaluation(
        &self,
        candidate_id: &str,
        validation_run_id: &str,
    ) -> Result<Option<ShadowEvaluationRecord>, CounterfactualReplayServiceError> {
        Ok(Some(ShadowEvaluationRecord {
            evaluation_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: candidate_id.to_string(),
            validation_run_id: validation_run_id.to_string(),
            evaluation_state: domain::research::ShadowEvaluationState::Completed,
            reason_code: "shadow_evaluation_completed".to_string(),
            market_context: serde_json::json!({
                "best_bid": 0.42,
                "best_ask": 0.44
            }),
            signal_decisions: serde_json::json!({
                "signals": [
                    {
                        "decision_side": "buy",
                        "intended_size": 10.0,
                        "decision_timestamp_utc": "2026-04-07T00:00:00Z"
                    }
                ]
            }),
            simulation_outcomes: vec![ShadowSimulationOutcome {
                decision_side: ShadowSimulationDecisionSide::Buy,
                intended_size: 10.0,
                simulated_fill_size: 10.0,
                simulated_fill_price: self.simulated_fill_price,
                simulated_slippage_bps: self.slippage_bps,
                simulation_reason_code:
                    domain::research::ShadowSimulationReasonCode::ReadOnlyEnforced
                        .code()
                        .to_string(),
                decision_timestamp_utc: "2026-04-07T00:00:00Z".to_string(),
                simulated_at_utc: "2026-04-07T00:00:01Z".to_string(),
            }],
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-shadow-001".to_string(),
            started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:00:01Z".to_string()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_start_input() -> StartCounterfactualReplayInput {
        StartCounterfactualReplayInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            correlation_id: "corr-replay-001".to_string(),
            requested_at_utc: "2026-04-07T00:20:00Z".to_string(),
        }
    }

    fn service_with_shadow_port(
        shadow_port: Arc<dyn ShadowEvidencePort>,
    ) -> CounterfactualReplayService {
        CounterfactualReplayService::new(
            Arc::new(InMemoryCounterfactualReplayRepository::default()),
            Arc::new(StaticValidationEvidencePort::default()),
            shadow_port,
        )
    }

    #[test]
    fn counterfactual_replay_start_persists_required_scenarios_and_reason_codes() {
        let service = service_with_shadow_port(Arc::new(StaticShadowEvidencePort::default()));
        let evidence = service
            .start_counterfactual_replay(sample_start_input())
            .expect("replay run should succeed");
        assert_eq!(
            evidence.reason_code,
            CounterfactualReplayReasonCode::RunStarted.code()
        );
        assert_eq!(evidence.replay_run.replay_summary.scenarios.len(), 3);
        let mut scenario_names = evidence
            .replay_run
            .replay_summary
            .scenarios
            .iter()
            .map(|scenario| scenario.scenario.as_str())
            .collect::<Vec<_>>();
        scenario_names.sort();
        let mut expected_scenarios = FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS
            .iter()
            .map(|scenario| scenario.as_str())
            .collect::<Vec<_>>();
        expected_scenarios.sort();
        assert_eq!(scenario_names, expected_scenarios);
    }

    #[test]
    fn counterfactual_replay_start_boundary_equal_to_negative_five_is_allow_path() {
        let service = service_with_shadow_port(Arc::new(StaticShadowEvidencePort {
            slippage_bps: 100.0,
            simulated_fill_price: 0.43,
        }));
        let evidence = service
            .start_counterfactual_replay(sample_start_input())
            .expect("boundary replay run should succeed");
        assert!((evidence.replay_run.replay_summary.degradation_pct + 5.0).abs() < 1e-9);
        assert_eq!(
            evidence.replay_run.replay_summary.gate_outcome,
            CounterfactualReplayGateOutcome::Allow
        );
        assert_eq!(
            evidence.replay_run.run_state,
            CounterfactualReplayRunState::Completed
        );
    }

    #[test]
    fn counterfactual_replay_start_denies_when_degradation_is_below_tolerance() {
        let service = service_with_shadow_port(Arc::new(StaticShadowEvidencePort {
            slippage_bps: 120.0,
            simulated_fill_price: 0.43,
        }));
        let evidence = service
            .start_counterfactual_replay(sample_start_input())
            .expect("replay run should persist as denied");
        assert!(evidence.replay_run.replay_summary.degradation_pct < -5.0);
        assert_eq!(
            evidence.replay_run.replay_summary.gate_outcome,
            CounterfactualReplayGateOutcome::Deny
        );
        assert_eq!(
            evidence.replay_run.run_state,
            CounterfactualReplayRunState::Denied
        );
        assert_eq!(
            evidence.replay_run.reason_code,
            CounterfactualReplayReasonCode::ToleranceBreached.code()
        );
    }

    #[test]
    fn counterfactual_replay_start_fails_closed_for_invalid_baseline_math() {
        let service = service_with_shadow_port(Arc::new(StaticShadowEvidencePort {
            slippage_bps: 5.0,
            simulated_fill_price: 1.2,
        }));
        let error = service
            .start_counterfactual_replay(sample_start_input())
            .expect_err("invalid baseline denominator should fail closed");
        assert_eq!(
            error.code,
            CounterfactualReplayReasonCode::InvalidPayload.code()
        );
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "baseline_net_pnl"
                && issue.code == CounterfactualReplayReasonCode::BaselineUnavailable.code()
        }));
    }

    #[test]
    fn counterfactual_replay_list_orders_results_deterministically() {
        let service = service_with_shadow_port(Arc::new(StaticShadowEvidencePort::default()));
        let mut first = sample_start_input();
        first.requested_at_utc = "2026-04-07T00:20:00Z".to_string();
        service
            .start_counterfactual_replay(first)
            .expect("first replay run should persist");

        let mut second = sample_start_input();
        second.requested_at_utc = "2026-04-07T00:22:00Z".to_string();
        second.correlation_id = "corr-replay-002".to_string();
        service
            .start_counterfactual_replay(second)
            .expect("second replay run should persist");

        let listed = service
            .list_counterfactual_replay_runs(ListCounterfactualReplayRunsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                limit: Some(10),
                started_after_utc: None,
                started_before_utc: None,
                correlation_id: "corr-replay-list-001".to_string(),
                queried_at_utc: "2026-04-07T00:23:00Z".to_string(),
            })
            .expect("list should succeed");
        assert_eq!(listed.len(), 2);
        assert!(listed[0].started_at_utc >= listed[1].started_at_utc);
    }
}
