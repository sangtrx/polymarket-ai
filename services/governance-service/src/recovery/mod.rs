use domain::reconciliation::ReconciliationRunSummary;
use domain::recovery::{
    RecoveryContractError, RecoveryGateName, RecoveryGateOutcome, RecoveryGateRunEvidence,
    RecoveryOperatorSignoff, RecoveryReadinessRequest, RecoveryReadinessStatus, RecoveryReasonCode,
    RecoveryResumeVerificationEnvelope, RecoveryValidationIssue, build_operator_signoff,
    build_recovery_resume_verification_envelope, compute_risk_limit_bundle_checksum,
    evaluate_checksum_gate, evaluate_freshness_readiness, evaluate_reconciliation_readiness,
    validate_checksum_digest, validate_recovery_gate_run_evidence,
};
use domain::risk::{FreshnessGateEvent, normalize_risk_limit_identifier};
use persistence::postgres::freshness_gate::{
    FreshnessGatePersistenceError,
    load_latest_freshness_gate_event as pg_load_latest_freshness_gate_event,
};
use persistence::postgres::reconciliation::{
    ReconciliationPersistenceError, load_reconciliation_run as pg_load_reconciliation_run,
};
use persistence::postgres::recovery_gate_runs::{
    RecoveryGateRunPersistenceError, insert_recovery_gate_run as pg_insert_recovery_gate_run,
    load_latest_recovery_gate_run_by_correlation as pg_load_latest_run_by_correlation,
    load_recovery_gate_run_by_run_id as pg_load_run_by_run_id,
};
use persistence::postgres::risk_limits::{
    RiskLimitPersistenceError, RiskLimitProfileBundle,
    load_active_risk_limit_profile_bundle as pg_load_active_risk_limit_profile_bundle,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RecoveryServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<RecoveryValidationIssue>,
}

impl RecoveryServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RecoveryValidationIssue>,
    ) -> Self {
        Self {
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: RecoveryReasonCode::Unauthorized.code(),
            message: "actor role is not authorized for controlled recovery".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RecoveryServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RecoveryServiceError {}

#[derive(Debug, Clone)]
pub struct EvaluateRecoveryReadinessInput {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub profile_key: String,
    pub reconciliation_run_id: String,
    pub approved_checksum: String,
    pub signoff_intent: String,
    pub audit_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecuteRecoveryResumeInput {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub resumed_at_utc: String,
    pub run_id: String,
}

#[derive(Debug, Clone)]
pub struct QueryRecoveryGateRunInput {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
    pub run_id: Option<String>,
    pub query_correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecoveryResumeExecutionEvidence {
    pub verification: RecoveryResumeVerificationEnvelope,
    pub run: RecoveryGateRunEvidence,
}

pub trait RecoveryGateRunRepositoryPort: Send + Sync {
    fn insert_run(&self, run: RecoveryGateRunEvidence) -> Result<(), RecoveryServiceError>;
    fn load_run_by_run_id(
        &self,
        run_id: &str,
    ) -> Result<Option<RecoveryGateRunEvidence>, RecoveryServiceError>;
    fn load_latest_run_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<RecoveryGateRunEvidence>, RecoveryServiceError>;
}

pub trait RecoveryDependencyPort: Send + Sync {
    fn load_latest_freshness_event(
        &self,
    ) -> Result<Option<FreshnessGateEvent>, RecoveryServiceError>;
    fn load_reconciliation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ReconciliationRunSummary>, RecoveryServiceError>;
    fn load_active_risk_limit_bundle(
        &self,
        profile_key: &str,
    ) -> Result<Option<RiskLimitProfileBundle>, RecoveryServiceError>;
}

pub trait RecoveryOrchestrator: Send + Sync {
    fn evaluate_recovery_readiness(
        &self,
        input: EvaluateRecoveryReadinessInput,
    ) -> Result<RecoveryGateRunEvidence, RecoveryServiceError>;
    fn execute_recovery_resume(
        &self,
        input: ExecuteRecoveryResumeInput,
    ) -> Result<RecoveryResumeExecutionEvidence, RecoveryServiceError>;
    fn query_recovery_gate_run(
        &self,
        input: QueryRecoveryGateRunInput,
    ) -> Result<RecoveryGateRunEvidence, RecoveryServiceError>;
}

#[derive(Clone)]
pub struct RecoveryService {
    repository: Arc<dyn RecoveryGateRunRepositoryPort>,
    dependencies: Arc<dyn RecoveryDependencyPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl RecoveryService {
    pub fn new(
        repository: Arc<dyn RecoveryGateRunRepositoryPort>,
        dependencies: Arc<dyn RecoveryDependencyPort>,
    ) -> Self {
        Self {
            repository,
            dependencies,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryRecoveryGateRunRepository::default()),
            Arc::new(InMemoryRecoveryDependencyPort::default()),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresRecoveryGateRunRepository::new(pool.clone())),
            Arc::new(PostgresRecoveryDependencyAdapter::new(pool)),
        )
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, RecoveryServiceError> {
        self.operation_lock.lock().map_err(|_| {
            RecoveryServiceError::persistence_unavailable(
                "controlled recovery operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for RecoveryService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl RecoveryOrchestrator for RecoveryService {
    fn evaluate_recovery_readiness(
        &self,
        input: EvaluateRecoveryReadinessInput,
    ) -> Result<RecoveryGateRunEvidence, RecoveryServiceError> {
        validate_recovery_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("profile_key", &input.profile_key)?;
        validate_non_empty("reconciliation_run_id", &input.reconciliation_run_id)?;
        validate_non_empty("requested_at_utc", &input.requested_at_utc)?;
        let approved_checksum = validate_checksum_digest(&input.approved_checksum)
            .map_err(map_contract_error)?
            .to_string();

        let request = RecoveryReadinessRequest {
            actor_id: input.actor_id.clone(),
            actor_role: input.actor_role.clone(),
            correlation_id: input.correlation_id.clone(),
            requested_at_utc: input.requested_at_utc.clone(),
            profile_key: input.profile_key.clone(),
            reconciliation_run_id: input.reconciliation_run_id.clone(),
            approved_checksum: approved_checksum.clone(),
            signoff_intent: Some(input.signoff_intent.clone()),
            audit_reference: input.audit_reference.clone(),
        };
        let signoff = build_operator_signoff(&request).map_err(map_contract_error)?;

        let _lock = self.lock_operations()?;
        let freshness_event = self.dependencies.load_latest_freshness_event()?;
        let reconciliation_summary = self
            .dependencies
            .load_reconciliation_run(&input.reconciliation_run_id)?;
        let active_risk_bundle = self
            .dependencies
            .load_active_risk_limit_bundle(&input.profile_key)?;

        let (freshness_age_seconds, freshness_observed_at_utc, freshness_outcome) =
            evaluate_freshness_gate_outcome(freshness_event.as_ref())?;
        let (reconciliation_mismatch_rate, reconciliation_outcome) =
            evaluate_reconciliation_gate_outcome(
                reconciliation_summary.as_ref(),
                &input.reconciliation_run_id,
            )?;
        let (computed_checksum, checksum_outcome) =
            evaluate_checksum_gate_outcome(active_risk_bundle.as_ref(), &approved_checksum)?;
        let signoff_outcome = evaluate_signoff_gate_outcome(&signoff);

        let gate_outcomes = vec![
            freshness_outcome,
            reconciliation_outcome,
            checksum_outcome,
            signoff_outcome,
        ];
        let failing_gate_codes = gate_outcomes
            .iter()
            .filter(|outcome| !outcome.passed)
            .map(|outcome| outcome.reason_code.clone())
            .collect::<Vec<_>>();
        let readiness_status = if failing_gate_codes.is_empty() {
            RecoveryReadinessStatus::Approved
        } else {
            RecoveryReadinessStatus::Blocked
        };
        let reason_code = if readiness_status == RecoveryReadinessStatus::Approved {
            RecoveryReasonCode::ResumeApproved.code().to_string()
        } else {
            RecoveryReasonCode::ResumeBlocked.code().to_string()
        };
        let run_id = build_recovery_run_id(
            &input.profile_key,
            &input.correlation_id,
            &input.requested_at_utc,
        );
        let run = RecoveryGateRunEvidence {
            run_id,
            correlation_id: normalize_risk_limit_identifier(&input.correlation_id),
            readiness_status,
            reason_code,
            actor_id: normalize_risk_limit_identifier(&input.actor_id),
            actor_role: normalize_risk_limit_identifier(&input.actor_role),
            profile_key: normalize_risk_limit_identifier(&input.profile_key),
            requested_at_utc: input.requested_at_utc.clone(),
            evaluated_at_utc: input.requested_at_utc.clone(),
            resumed_at_utc: None,
            freshness_age_seconds,
            freshness_observed_at_utc,
            reconciliation_run_id: Some(normalize_risk_limit_identifier(
                &input.reconciliation_run_id,
            )),
            reconciliation_mismatch_rate,
            approved_checksum: Some(approved_checksum),
            computed_checksum,
            signoff: Some(signoff),
            gate_outcomes,
            failing_gate_codes,
            audit_reference: normalize_optional_field(input.audit_reference.as_deref()),
        };
        validate_recovery_gate_run_evidence(&run).map_err(map_contract_error)?;
        self.repository.insert_run(run.clone())?;
        emit_recovery_telemetry("governance_recovery_readiness_evaluate_v1", &run);
        Ok(run)
    }

    fn execute_recovery_resume(
        &self,
        input: ExecuteRecoveryResumeInput,
    ) -> Result<RecoveryResumeExecutionEvidence, RecoveryServiceError> {
        validate_recovery_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("run_id", &input.run_id)?;
        validate_non_empty("resumed_at_utc", &input.resumed_at_utc)?;

        let mut run = self
            .repository
            .load_run_by_run_id(&input.run_id)?
            .ok_or_else(|| {
                RecoveryServiceError::not_found(format!(
                    "recovery gate run `{}` was not found",
                    normalize_risk_limit_identifier(&input.run_id)
                ))
            })?;
        if run.readiness_status != RecoveryReadinessStatus::Approved {
            return Err(RecoveryServiceError::stale_evidence(
                "recovery resume denied because readiness status is blocked",
            ));
        }
        if run.resumed_at_utc.is_some() {
            return Err(RecoveryServiceError::stale_evidence(
                "recovery resume denied because verification evidence is already recorded",
            ));
        }
        run.resumed_at_utc = Some(input.resumed_at_utc.clone());
        run.reason_code = RecoveryReasonCode::ResumeApproved.code().to_string();
        validate_recovery_gate_run_evidence(&run).map_err(map_contract_error)?;
        self.repository.insert_run(run.clone())?;

        let verification = build_recovery_resume_verification_envelope(&run, &input.resumed_at_utc)
            .map_err(map_contract_error)?;
        emit_recovery_resume_telemetry(
            "governance_recovery_resume_execute_v1",
            &run,
            &verification,
        );
        Ok(RecoveryResumeExecutionEvidence { verification, run })
    }

    fn query_recovery_gate_run(
        &self,
        input: QueryRecoveryGateRunInput,
    ) -> Result<RecoveryGateRunEvidence, RecoveryServiceError> {
        validate_recovery_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        let run = if let Some(run_id) = input.run_id.as_deref() {
            self.repository.load_run_by_run_id(run_id)?
        } else if let Some(query_correlation_id) = input.query_correlation_id.as_deref() {
            self.repository
                .load_latest_run_by_correlation(query_correlation_id)?
        } else {
            return Err(RecoveryServiceError::invalid_payload(
                "query_recovery_gate_run requires run_id or query_correlation_id",
                vec![RecoveryValidationIssue {
                    field: "run_id",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "run_id or query_correlation_id must be provided".to_string(),
                }],
            ));
        };
        let run = run.ok_or_else(|| {
            RecoveryServiceError::not_found(
                "recovery gate run query returned no matching evidence".to_string(),
            )
        })?;
        emit_recovery_telemetry("governance_recovery_readiness_query_v1", &run);
        Ok(run)
    }
}

fn evaluate_freshness_gate_outcome(
    freshness_event: Option<&FreshnessGateEvent>,
) -> Result<(Option<f64>, Option<String>, RecoveryGateOutcome), RecoveryServiceError> {
    let Some(event) = freshness_event else {
        return Ok((
            None,
            None,
            RecoveryGateOutcome {
                gate: RecoveryGateName::Freshness,
                passed: false,
                reason_code: RecoveryReasonCode::FreshnessStale.code().to_string(),
                trigger: "freshness telemetry gate requires latest event".to_string(),
                context: "no freshness gate event is available from persistence".to_string(),
                action: "keep containment active and restore freshness telemetry inputs"
                    .to_string(),
                verification: "persist fresh gate event with <=30 second data age".to_string(),
            },
        ));
    };
    let Some(age_seconds) = event
        .market_data_age_seconds
        .into_iter()
        .chain(event.user_data_age_seconds)
        .max_by(f64::total_cmp)
    else {
        return Ok((
            None,
            Some(event.evaluated_at_utc.clone()),
            RecoveryGateOutcome {
                gate: RecoveryGateName::Freshness,
                passed: false,
                reason_code: RecoveryReasonCode::FreshnessStale.code().to_string(),
                trigger: "freshness telemetry gate requires age samples".to_string(),
                context:
                    "freshness event exists but both market_data_age_seconds and user_data_age_seconds are missing"
                        .to_string(),
                action:
                    "keep containment active until freshness age telemetry is restored".to_string(),
                verification: format!(
                    "freshness_event_id={} includes market/user age samples",
                    event.event_id
                ),
            },
        ));
    };
    let passed = evaluate_freshness_readiness(age_seconds).map_err(map_contract_error)?;
    let outcome = RecoveryGateOutcome {
        gate: RecoveryGateName::Freshness,
        passed,
        reason_code: if passed {
            RecoveryReasonCode::FreshnessPass.code().to_string()
        } else {
            RecoveryReasonCode::FreshnessStale.code().to_string()
        },
        trigger: "freshness gate requires max(stream_age_seconds) <= 30".to_string(),
        context: format!(
            "market_data_age_seconds={:?}, user_data_age_seconds={:?}, pause_active={}",
            event.market_data_age_seconds, event.user_data_age_seconds, event.pause_active
        ),
        action: if passed {
            "allow controlled recovery freshness gate".to_string()
        } else {
            "keep containment active until freshness age recovers to <=30 seconds".to_string()
        },
        verification: format!(
            "freshness_event_id={} observed_at_utc={}",
            event.event_id, event.evaluated_at_utc
        ),
    };
    Ok((
        Some(age_seconds),
        Some(event.evaluated_at_utc.clone()),
        outcome,
    ))
}

fn evaluate_reconciliation_gate_outcome(
    reconciliation_summary: Option<&ReconciliationRunSummary>,
    requested_run_id: &str,
) -> Result<(Option<f64>, RecoveryGateOutcome), RecoveryServiceError> {
    let Some(summary) = reconciliation_summary else {
        return Ok((
            None,
            RecoveryGateOutcome {
                gate: RecoveryGateName::Reconciliation,
                passed: false,
                reason_code: RecoveryReasonCode::ReconciliationMismatch
                    .code()
                    .to_string(),
                trigger: "reconciliation gate requires referenced run evidence".to_string(),
                context: format!(
                    "reconciliation run `{}` not found in persistence",
                    normalize_risk_limit_identifier(requested_run_id)
                ),
                action: "rerun reconciliation and persist run summary before attempting resume"
                    .to_string(),
                verification: "load reconciliation run by run_id returns evidence".to_string(),
            },
        ));
    };
    let passed =
        evaluate_reconciliation_readiness(summary.mismatch_rate).map_err(map_contract_error)?;
    let outcome = RecoveryGateOutcome {
        gate: RecoveryGateName::Reconciliation,
        passed,
        reason_code: if passed {
            RecoveryReasonCode::ReconciliationPass.code().to_string()
        } else {
            RecoveryReasonCode::ReconciliationMismatch
                .code()
                .to_string()
        },
        trigger: "reconciliation gate requires mismatch_rate < 0.1%".to_string(),
        context: format!(
            "run_id={}, mismatch_rate={:.6}, mismatch_count={}, compared_records={}",
            summary.run_id, summary.mismatch_rate, summary.mismatch_count, summary.compared_records
        ),
        action: if passed {
            "allow controlled recovery reconciliation gate".to_string()
        } else {
            "keep containment active and investigate reconciliation mismatches".to_string()
        },
        verification: format!(
            "reconciliation summary evaluated_at_utc={} critical_halt={}",
            summary.evaluated_at_utc, summary.critical_halt
        ),
    };
    Ok((Some(summary.mismatch_rate), outcome))
}

fn evaluate_checksum_gate_outcome(
    bundle: Option<&RiskLimitProfileBundle>,
    approved_checksum: &str,
) -> Result<(Option<String>, RecoveryGateOutcome), RecoveryServiceError> {
    let Some(bundle) = bundle else {
        return Ok((
            None,
            RecoveryGateOutcome {
                gate: RecoveryGateName::RiskChecksum,
                passed: false,
                reason_code: RecoveryReasonCode::ChecksumMismatch.code().to_string(),
                trigger: "risk checksum gate requires active risk-limit profile bundle".to_string(),
                context: "no active risk-limit profile bundle available for requested profile"
                    .to_string(),
                action: "activate risk-limit profile bundle before controlled recovery resume"
                    .to_string(),
                verification:
                    "load_active_risk_limit_profile_bundle returns active profile and inventory rules"
                        .to_string(),
            },
        ));
    };
    let computed_checksum =
        compute_risk_limit_bundle_checksum(&bundle.profile, &bundle.inventory_rules)
            .map_err(map_contract_error)?;
    let passed = evaluate_checksum_gate(approved_checksum, &computed_checksum)
        .map_err(map_contract_error)?;
    let outcome = RecoveryGateOutcome {
        gate: RecoveryGateName::RiskChecksum,
        passed,
        reason_code: if passed {
            RecoveryReasonCode::ChecksumMatch.code().to_string()
        } else {
            RecoveryReasonCode::ChecksumMismatch.code().to_string()
        },
        trigger: "risk checksum gate requires exact approved/computed digest equality".to_string(),
        context: format!(
            "profile_key={}, version={}, inventory_rule_count={}",
            bundle.profile.profile_key,
            bundle.profile.version,
            bundle.inventory_rules.len()
        ),
        action: if passed {
            "allow controlled recovery checksum gate".to_string()
        } else {
            "keep containment active and refresh approved risk-limit checksum".to_string()
        },
        verification: format!(
            "approved_checksum={} computed_checksum={computed_checksum}",
            approved_checksum
        ),
    };
    Ok((Some(computed_checksum), outcome))
}

fn evaluate_signoff_gate_outcome(signoff: &RecoveryOperatorSignoff) -> RecoveryGateOutcome {
    RecoveryGateOutcome {
        gate: RecoveryGateName::OperatorSignoff,
        passed: true,
        reason_code: RecoveryReasonCode::SignoffRecorded.code().to_string(),
        trigger: "operator sign-off must be explicitly recorded".to_string(),
        context: format!(
            "actor_id={} actor_role={} signoff_intent={}",
            signoff.actor_id, signoff.actor_role, signoff.signoff_intent
        ),
        action: "allow controlled recovery sign-off gate".to_string(),
        verification: format!(
            "signed_at_utc={} audit_reference={}",
            signoff.signed_at_utc,
            signoff.audit_reference.as_deref().unwrap_or("none")
        ),
    }
}

fn build_recovery_run_id(
    profile_key: &str,
    correlation_id: &str,
    requested_at_utc: &str,
) -> String {
    format!(
        "recovery::gate::{}::{}::{}",
        normalize_risk_limit_identifier(profile_key),
        normalize_risk_limit_identifier(correlation_id),
        compact_utc_timestamp_token(requested_at_utc)
    )
}

fn compact_utc_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>()
}

fn validate_recovery_role(role: &str) -> Result<(), RecoveryServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(RecoveryServiceError::unauthorized_role()),
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), RecoveryServiceError> {
    if value.trim().is_empty() {
        return Err(RecoveryServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![RecoveryValidationIssue {
                field,
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn normalize_optional_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn map_contract_error(error: RecoveryContractError) -> RecoveryServiceError {
    RecoveryServiceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn map_recovery_gate_persistence_error(
    error: RecoveryGateRunPersistenceError,
) -> RecoveryServiceError {
    match error.code {
        "recovery_gate_query_failed"
        | "recovery_gate_row_decode_failed"
        | "recovery_gate_runtime_unavailable" => {
            RecoveryServiceError::persistence_unavailable(error.message)
        }
        _ => RecoveryServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

fn map_freshness_dependency_error(error: FreshnessGatePersistenceError) -> RecoveryServiceError {
    RecoveryServiceError::dependency_unavailable(format!(
        "freshness dependency unavailable: {}",
        error.message
    ))
}

fn map_reconciliation_dependency_error(
    error: ReconciliationPersistenceError,
) -> RecoveryServiceError {
    RecoveryServiceError::dependency_unavailable(format!(
        "reconciliation dependency unavailable: {}",
        error.message
    ))
}

fn map_risk_limit_dependency_error(error: RiskLimitPersistenceError) -> RecoveryServiceError {
    RecoveryServiceError::dependency_unavailable(format!(
        "risk-limit dependency unavailable: {}",
        error.message
    ))
}

fn emit_recovery_telemetry(event_name: &'static str, run: &RecoveryGateRunEvidence) {
    let event = RecoveryTelemetryEvent {
        event_name,
        run_id: &run.run_id,
        correlation_id: &run.correlation_id,
        readiness_status: run.readiness_status.as_str(),
        reason_code: &run.reason_code,
        actor_id: &run.actor_id,
        actor_role: &run.actor_role,
        profile_key: &run.profile_key,
        requested_at_utc: &run.requested_at_utc,
        evaluated_at_utc: &run.evaluated_at_utc,
        resumed_at_utc: run.resumed_at_utc.as_deref(),
        failing_gate_codes: &run.failing_gate_codes,
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("recovery telemetry event should serialize")
    );
}

fn emit_recovery_resume_telemetry(
    event_name: &'static str,
    run: &RecoveryGateRunEvidence,
    verification: &RecoveryResumeVerificationEnvelope,
) {
    let event = RecoveryResumeTelemetryEvent {
        event_name,
        run_id: &run.run_id,
        correlation_id: &run.correlation_id,
        readiness_status: run.readiness_status.as_str(),
        reason_code: &run.reason_code,
        verification_reason_code: &verification.reason_code,
        verified_at_utc: &verification.verified_at_utc,
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("recovery resume telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct RecoveryTelemetryEvent<'a> {
    event_name: &'a str,
    run_id: &'a str,
    correlation_id: &'a str,
    readiness_status: &'a str,
    reason_code: &'a str,
    actor_id: &'a str,
    actor_role: &'a str,
    profile_key: &'a str,
    requested_at_utc: &'a str,
    evaluated_at_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    resumed_at_utc: Option<&'a str>,
    failing_gate_codes: &'a [String],
}

#[derive(Debug, Serialize)]
struct RecoveryResumeTelemetryEvent<'a> {
    event_name: &'a str,
    run_id: &'a str,
    correlation_id: &'a str,
    readiness_status: &'a str,
    reason_code: &'a str,
    verification_reason_code: &'a str,
    verified_at_utc: &'a str,
}

#[derive(Debug, Clone)]
pub struct PostgresRecoveryGateRunRepository {
    pool: PgPool,
}

impl PostgresRecoveryGateRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, RecoveryServiceError>
    where
        F: Future<Output = Result<T, RecoveryGateRunPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_recovery_gate_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    RecoveryServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_recovery_gate_persistence_error),
        }
    }
}

impl RecoveryGateRunRepositoryPort for PostgresRecoveryGateRunRepository {
    fn insert_run(&self, run: RecoveryGateRunEvidence) -> Result<(), RecoveryServiceError> {
        self.run_with_runtime(pg_insert_recovery_gate_run(&self.pool, &run))
    }

    fn load_run_by_run_id(
        &self,
        run_id: &str,
    ) -> Result<Option<RecoveryGateRunEvidence>, RecoveryServiceError> {
        self.run_with_runtime(pg_load_run_by_run_id(&self.pool, run_id))
    }

    fn load_latest_run_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<RecoveryGateRunEvidence>, RecoveryServiceError> {
        self.run_with_runtime(pg_load_latest_run_by_correlation(
            &self.pool,
            correlation_id,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct PostgresRecoveryDependencyAdapter {
    pool: PgPool,
}

impl PostgresRecoveryDependencyAdapter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<E, F, T>(
        &self,
        future: F,
        error_mapper: fn(E) -> RecoveryServiceError,
    ) -> Result<T, RecoveryServiceError>
    where
        F: Future<Output = Result<T, E>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| handle.block_on(future)).map_err(error_mapper)
            }
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    RecoveryServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(error_mapper),
        }
    }
}

impl RecoveryDependencyPort for PostgresRecoveryDependencyAdapter {
    fn load_latest_freshness_event(
        &self,
    ) -> Result<Option<FreshnessGateEvent>, RecoveryServiceError> {
        self.run_with_runtime(
            pg_load_latest_freshness_gate_event(&self.pool),
            map_freshness_dependency_error,
        )
    }

    fn load_reconciliation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ReconciliationRunSummary>, RecoveryServiceError> {
        self.run_with_runtime(
            pg_load_reconciliation_run(&self.pool, run_id),
            map_reconciliation_dependency_error,
        )
    }

    fn load_active_risk_limit_bundle(
        &self,
        profile_key: &str,
    ) -> Result<Option<RiskLimitProfileBundle>, RecoveryServiceError> {
        self.run_with_runtime(
            pg_load_active_risk_limit_profile_bundle(&self.pool, profile_key),
            map_risk_limit_dependency_error,
        )
    }
}

#[derive(Debug, Default)]
pub struct InMemoryRecoveryGateRunRepository {
    runs: Mutex<BTreeMap<String, RecoveryGateRunEvidence>>,
}

impl RecoveryGateRunRepositoryPort for InMemoryRecoveryGateRunRepository {
    fn insert_run(&self, run: RecoveryGateRunEvidence) -> Result<(), RecoveryServiceError> {
        let mut runs = self
            .runs
            .lock()
            .expect("in-memory recovery runs lock should not be poisoned");
        runs.insert(run.run_id.clone(), run);
        Ok(())
    }

    fn load_run_by_run_id(
        &self,
        run_id: &str,
    ) -> Result<Option<RecoveryGateRunEvidence>, RecoveryServiceError> {
        let runs = self
            .runs
            .lock()
            .expect("in-memory recovery runs lock should not be poisoned");
        let normalized = normalize_risk_limit_identifier(run_id);
        Ok(runs.get(&normalized).cloned())
    }

    fn load_latest_run_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<RecoveryGateRunEvidence>, RecoveryServiceError> {
        let runs = self
            .runs
            .lock()
            .expect("in-memory recovery runs lock should not be poisoned");
        let normalized = normalize_risk_limit_identifier(correlation_id);
        Ok(runs
            .values()
            .filter(|run| run.correlation_id == normalized)
            .max_by(|left, right| {
                left.evaluated_at_utc
                    .cmp(&right.evaluated_at_utc)
                    .then(left.run_id.cmp(&right.run_id))
            })
            .cloned())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryRecoveryDependencyPort {
    latest_freshness_event: Mutex<Option<FreshnessGateEvent>>,
    reconciliation_runs: Mutex<BTreeMap<String, ReconciliationRunSummary>>,
    active_risk_limit_bundles: Mutex<BTreeMap<String, RiskLimitProfileBundle>>,
}

impl InMemoryRecoveryDependencyPort {
    pub fn set_latest_freshness_event(&self, event: Option<FreshnessGateEvent>) {
        let mut freshness = self
            .latest_freshness_event
            .lock()
            .expect("in-memory freshness lock should not be poisoned");
        *freshness = event;
    }

    pub fn upsert_reconciliation_run(&self, summary: ReconciliationRunSummary) {
        let mut runs = self
            .reconciliation_runs
            .lock()
            .expect("in-memory reconciliation lock should not be poisoned");
        runs.insert(normalize_risk_limit_identifier(&summary.run_id), summary);
    }

    pub fn upsert_active_risk_limit_bundle(&self, bundle: RiskLimitProfileBundle) {
        let mut bundles = self
            .active_risk_limit_bundles
            .lock()
            .expect("in-memory risk-limit lock should not be poisoned");
        bundles.insert(
            normalize_risk_limit_identifier(&bundle.profile.profile_key),
            bundle,
        );
    }
}

impl RecoveryDependencyPort for InMemoryRecoveryDependencyPort {
    fn load_latest_freshness_event(
        &self,
    ) -> Result<Option<FreshnessGateEvent>, RecoveryServiceError> {
        let freshness = self
            .latest_freshness_event
            .lock()
            .expect("in-memory freshness lock should not be poisoned");
        Ok(freshness.clone())
    }

    fn load_reconciliation_run(
        &self,
        run_id: &str,
    ) -> Result<Option<ReconciliationRunSummary>, RecoveryServiceError> {
        let runs = self
            .reconciliation_runs
            .lock()
            .expect("in-memory reconciliation lock should not be poisoned");
        Ok(runs.get(&normalize_risk_limit_identifier(run_id)).cloned())
    }

    fn load_active_risk_limit_bundle(
        &self,
        profile_key: &str,
    ) -> Result<Option<RiskLimitProfileBundle>, RecoveryServiceError> {
        let bundles = self
            .active_risk_limit_bundles
            .lock()
            .expect("in-memory risk-limit lock should not be poisoned");
        Ok(bundles
            .get(&normalize_risk_limit_identifier(profile_key))
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::reconciliation::{ReconciliationReasonCode, ReconciliationRunStatus};
    use domain::risk::{
        FreshnessGateTransition, InventoryLimitRule, RiskLimitProfileStatus, RiskLimitReasonCode,
        RiskLimitScope, RiskScopeLimit,
    };

    #[derive(Debug)]
    struct TestRecoveryDependencies {
        base: InMemoryRecoveryDependencyPort,
    }

    impl TestRecoveryDependencies {
        fn new_ready_state() -> Self {
            let base = InMemoryRecoveryDependencyPort::default();
            base.set_latest_freshness_event(Some(sample_freshness_event(10.0)));
            base.upsert_reconciliation_run(sample_reconciliation_run("recon-1", 0.0005));
            base.upsert_active_risk_limit_bundle(sample_risk_limit_bundle());
            Self { base }
        }
    }

    impl RecoveryDependencyPort for TestRecoveryDependencies {
        fn load_latest_freshness_event(
            &self,
        ) -> Result<Option<FreshnessGateEvent>, RecoveryServiceError> {
            self.base.load_latest_freshness_event()
        }

        fn load_reconciliation_run(
            &self,
            run_id: &str,
        ) -> Result<Option<ReconciliationRunSummary>, RecoveryServiceError> {
            self.base.load_reconciliation_run(run_id)
        }

        fn load_active_risk_limit_bundle(
            &self,
            profile_key: &str,
        ) -> Result<Option<RiskLimitProfileBundle>, RecoveryServiceError> {
            self.base.load_active_risk_limit_bundle(profile_key)
        }
    }

    fn service_with_dependencies(dependencies: Arc<dyn RecoveryDependencyPort>) -> RecoveryService {
        RecoveryService::new(
            Arc::new(InMemoryRecoveryGateRunRepository::default()),
            dependencies,
        )
    }

    #[test]
    fn evaluate_rejects_unauthorized_role() {
        let service =
            service_with_dependencies(Arc::new(TestRecoveryDependencies::new_ready_state()));
        let error = service
            .evaluate_recovery_readiness(EvaluateRecoveryReadinessInput {
                actor_id: "ops-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                correlation_id: "corr-1".to_string(),
                requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
                profile_key: "default".to_string(),
                reconciliation_run_id: "recon-1".to_string(),
                approved_checksum: "a".repeat(64),
                signoff_intent: "approve".to_string(),
                audit_reference: None,
            })
            .expect_err("unauthorized role must fail closed");
        assert_eq!(error.code, RecoveryReasonCode::Unauthorized.code());
    }

    #[test]
    fn evaluate_blocks_when_reconciliation_threshold_is_not_strict() {
        let dependencies = TestRecoveryDependencies::new_ready_state();
        dependencies
            .base
            .upsert_reconciliation_run(sample_reconciliation_run("recon-1", 0.001));
        let service = service_with_dependencies(Arc::new(dependencies));
        let ready_checksum = compute_risk_limit_bundle_checksum(
            &sample_risk_limit_bundle().profile,
            &sample_risk_limit_bundle().inventory_rules,
        )
        .expect("checksum should compute");
        let run = service
            .evaluate_recovery_readiness(EvaluateRecoveryReadinessInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-2".to_string(),
                requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
                profile_key: "default".to_string(),
                reconciliation_run_id: "recon-1".to_string(),
                approved_checksum: ready_checksum,
                signoff_intent: "approve controlled recovery".to_string(),
                audit_reference: None,
            })
            .expect("evaluation should succeed with blocked status evidence");
        assert_eq!(run.readiness_status, RecoveryReadinessStatus::Blocked);
        assert!(
            run.failing_gate_codes.contains(
                &RecoveryReasonCode::ReconciliationMismatch
                    .code()
                    .to_string()
            )
        );
    }

    #[test]
    fn evaluate_approves_when_all_gates_pass() {
        let dependencies = Arc::new(TestRecoveryDependencies::new_ready_state());
        let service = service_with_dependencies(dependencies);
        let bundle = sample_risk_limit_bundle();
        let checksum = compute_risk_limit_bundle_checksum(&bundle.profile, &bundle.inventory_rules)
            .expect("checksum should compute");
        let run = service
            .evaluate_recovery_readiness(EvaluateRecoveryReadinessInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-3".to_string(),
                requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
                profile_key: "default".to_string(),
                reconciliation_run_id: "recon-1".to_string(),
                approved_checksum: checksum,
                signoff_intent: "approve controlled recovery".to_string(),
                audit_reference: Some("arb-2026-0003".to_string()),
            })
            .expect("evaluation should approve");
        assert_eq!(run.readiness_status, RecoveryReadinessStatus::Approved);
        assert!(run.failing_gate_codes.is_empty());
        assert!(run.resumed_at_utc.is_none());
    }

    #[test]
    fn resume_records_verification_and_rejects_replay() {
        let dependencies = Arc::new(TestRecoveryDependencies::new_ready_state());
        let service = service_with_dependencies(dependencies);
        let bundle = sample_risk_limit_bundle();
        let checksum = compute_risk_limit_bundle_checksum(&bundle.profile, &bundle.inventory_rules)
            .expect("checksum should compute");

        let run = service
            .evaluate_recovery_readiness(EvaluateRecoveryReadinessInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-3-resume".to_string(),
                requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
                profile_key: "default".to_string(),
                reconciliation_run_id: "recon-1".to_string(),
                approved_checksum: checksum,
                signoff_intent: "approve controlled recovery".to_string(),
                audit_reference: Some("arb-2026-0003".to_string()),
            })
            .expect("evaluation should approve");
        assert!(run.resumed_at_utc.is_none());

        let resumed_at_utc = "2026-04-06T12:00:05Z".to_string();
        let resumed = service
            .execute_recovery_resume(ExecuteRecoveryResumeInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-3-resume".to_string(),
                resumed_at_utc: resumed_at_utc.clone(),
                run_id: run.run_id.clone(),
            })
            .expect("resume should record verification evidence");
        assert_eq!(resumed.run.resumed_at_utc, Some(resumed_at_utc.clone()));

        let queried = service
            .query_recovery_gate_run(QueryRecoveryGateRunInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-3-resume-query".to_string(),
                queried_at_utc: "2026-04-06T12:00:06Z".to_string(),
                run_id: Some(run.run_id.clone()),
                query_correlation_id: None,
            })
            .expect("query should return persisted resume evidence");
        assert_eq!(queried.resumed_at_utc, Some(resumed_at_utc));

        let replay = service
            .execute_recovery_resume(ExecuteRecoveryResumeInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-3-resume".to_string(),
                resumed_at_utc: "2026-04-06T12:00:07Z".to_string(),
                run_id: run.run_id,
            })
            .expect_err("resume replay must be rejected");
        assert_eq!(replay.code, RecoveryReasonCode::StaleEvidence.code());
    }

    #[test]
    fn resume_requires_preapproved_run() {
        let dependencies = Arc::new(TestRecoveryDependencies::new_ready_state());
        let repository = Arc::new(InMemoryRecoveryGateRunRepository::default());
        let service = RecoveryService::new(repository.clone(), dependencies);
        let blocked_run = RecoveryGateRunEvidence {
            run_id: "run-blocked".to_string(),
            correlation_id: "corr-4".to_string(),
            readiness_status: RecoveryReadinessStatus::Blocked,
            reason_code: RecoveryReasonCode::ResumeBlocked.code().to_string(),
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            profile_key: "default".to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            evaluated_at_utc: "2026-04-06T12:00:00Z".to_string(),
            resumed_at_utc: None,
            freshness_age_seconds: Some(45.0),
            freshness_observed_at_utc: Some("2026-04-06T11:59:00Z".to_string()),
            reconciliation_run_id: Some("recon-1".to_string()),
            reconciliation_mismatch_rate: Some(0.2),
            approved_checksum: Some("a".repeat(64)),
            computed_checksum: Some("b".repeat(64)),
            signoff: Some(RecoveryOperatorSignoff {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                signoff_intent: "approve".to_string(),
                signed_at_utc: "2026-04-06T12:00:00Z".to_string(),
                audit_reference: None,
            }),
            gate_outcomes: vec![
                RecoveryGateOutcome {
                    gate: RecoveryGateName::Freshness,
                    passed: false,
                    reason_code: RecoveryReasonCode::FreshnessStale.code().to_string(),
                    trigger: "freshness".to_string(),
                    context: "stale".to_string(),
                    action: "pause".to_string(),
                    verification: "verify".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::Reconciliation,
                    passed: false,
                    reason_code: RecoveryReasonCode::ReconciliationMismatch
                        .code()
                        .to_string(),
                    trigger: "recon".to_string(),
                    context: "mismatch".to_string(),
                    action: "pause".to_string(),
                    verification: "verify".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::RiskChecksum,
                    passed: false,
                    reason_code: RecoveryReasonCode::ChecksumMismatch.code().to_string(),
                    trigger: "checksum".to_string(),
                    context: "mismatch".to_string(),
                    action: "pause".to_string(),
                    verification: "verify".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::OperatorSignoff,
                    passed: true,
                    reason_code: RecoveryReasonCode::SignoffRecorded.code().to_string(),
                    trigger: "signoff".to_string(),
                    context: "present".to_string(),
                    action: "allow".to_string(),
                    verification: "verify".to_string(),
                },
            ],
            failing_gate_codes: vec![
                RecoveryReasonCode::FreshnessStale.code().to_string(),
                RecoveryReasonCode::ReconciliationMismatch
                    .code()
                    .to_string(),
                RecoveryReasonCode::ChecksumMismatch.code().to_string(),
            ],
            audit_reference: None,
        };
        repository
            .insert_run(blocked_run)
            .expect("blocked run should insert");

        let error = service
            .execute_recovery_resume(ExecuteRecoveryResumeInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-4".to_string(),
                resumed_at_utc: "2026-04-06T12:01:00Z".to_string(),
                run_id: "run-blocked".to_string(),
            })
            .expect_err("blocked run cannot resume");
        assert_eq!(error.code, RecoveryReasonCode::StaleEvidence.code());
    }

    fn sample_freshness_event(max_age_seconds: f64) -> FreshnessGateEvent {
        FreshnessGateEvent {
            event_id: "freshness-1".to_string(),
            transition: FreshnessGateTransition::RecoveryConfirmed,
            reason_code: "freshness_gate_recovery_confirmed".to_string(),
            pause_active: false,
            block_new_order_creation: false,
            market_data_age_seconds: Some(max_age_seconds),
            user_data_age_seconds: Some(max_age_seconds),
            stale_threshold_seconds: 30.0,
            stability_window_seconds: 10.0,
            max_breach_to_pause_seconds: 5.0,
            stale_breach_detected_at_utc: Some("2026-04-06T11:59:00Z".to_string()),
            pause_activated_at_utc: Some("2026-04-06T11:59:01Z".to_string()),
            recovery_window_started_at_utc: Some("2026-04-06T11:59:10Z".to_string()),
            recovery_confirmed_at_utc: Some("2026-04-06T11:59:20Z".to_string()),
            breach_to_pause_latency_seconds: Some(1.0),
            evaluated_at_utc: "2026-04-06T12:00:00Z".to_string(),
            correlation_id: "corr-freshness-1".to_string(),
        }
    }

    fn sample_reconciliation_run(run_id: &str, mismatch_rate: f64) -> ReconciliationRunSummary {
        ReconciliationRunSummary {
            run_id: run_id.to_string(),
            window_started_at_utc: "2026-04-06T11:59:00Z".to_string(),
            window_ended_at_utc: "2026-04-06T12:00:00Z".to_string(),
            compared_records: 1000,
            mismatch_count: (mismatch_rate * 1000.0) as i64,
            mismatch_rate,
            critical_halt: mismatch_rate >= 0.001,
            status: ReconciliationRunStatus::Succeeded,
            reason_code: if mismatch_rate >= 0.001 {
                ReconciliationReasonCode::CriticalMismatch
                    .code()
                    .to_string()
            } else {
                ReconciliationReasonCode::Matched.code().to_string()
            },
            evaluated_at_utc: "2026-04-06T12:00:00Z".to_string(),
            correlation_id: "corr-recon-1".to_string(),
        }
    }

    fn sample_risk_limit_bundle() -> RiskLimitProfileBundle {
        RiskLimitProfileBundle {
            profile: domain::risk::RiskLimitProfileVersion {
                profile_key: "default".to_string(),
                version: 3,
                portfolio: RiskScopeLimit {
                    scope: RiskLimitScope::Portfolio,
                    scope_id: "portfolio".to_string(),
                    max_notional_usd: 2_000_000.0,
                    max_inventory_units: 30_000.0,
                    max_concentration_pct_nav: 40.0,
                },
                market: RiskScopeLimit {
                    scope: RiskLimitScope::Market,
                    scope_id: "market".to_string(),
                    max_notional_usd: 1_000_000.0,
                    max_inventory_units: 15_000.0,
                    max_concentration_pct_nav: 25.0,
                },
                strategy: RiskScopeLimit {
                    scope: RiskLimitScope::Strategy,
                    scope_id: "strategy".to_string(),
                    max_notional_usd: 500_000.0,
                    max_inventory_units: 8_000.0,
                    max_concentration_pct_nav: 15.0,
                },
                status: RiskLimitProfileStatus::Active,
                approval_reference: Some("arb-2026-0001".to_string()),
                actor_id: "ops-1".to_string(),
                reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
                correlation_id: "corr-risk-1".to_string(),
                updated_at_utc: "2026-04-06T11:58:00Z".to_string(),
            },
            inventory_rules: vec![
                InventoryLimitRule {
                    rule_id: "rule-market-1".to_string(),
                    profile_key: "default".to_string(),
                    profile_version: 3,
                    scope: RiskLimitScope::Market,
                    scope_id: "market".to_string(),
                    max_position_units: 500.0,
                    max_order_size_units: 100.0,
                    max_concentration_pct_nav: 5.0,
                    actor_id: "ops-1".to_string(),
                    correlation_id: "corr-risk-1".to_string(),
                    updated_at_utc: "2026-04-06T11:58:00Z".to_string(),
                },
                InventoryLimitRule {
                    rule_id: "rule-strategy-1".to_string(),
                    profile_key: "default".to_string(),
                    profile_version: 3,
                    scope: RiskLimitScope::Strategy,
                    scope_id: "strategy".to_string(),
                    max_position_units: 300.0,
                    max_order_size_units: 50.0,
                    max_concentration_pct_nav: 3.0,
                    actor_id: "ops-1".to_string(),
                    correlation_id: "corr-risk-1".to_string(),
                    updated_at_utc: "2026-04-06T11:58:00Z".to_string(),
                },
            ],
        }
    }
}
