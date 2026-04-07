use domain::research::{
    FR43_MANDATORY_GATE_TYPES, ValidationGateComparator, ValidationGateContractError,
    ValidationGatePolicyDefinition, ValidationGateReasonCode, ValidationGateStageScope,
    ValidationGateThreshold, ValidationGateType, ValidationGateValidationIssue,
    ValidationGateWorkflowStage, canonicalize_validation_gate_policy_definition,
    evaluate_validation_gate_threshold, normalize_research_identifier, parse_utc_timestamp,
};
use persistence::postgres::validation_gate_policies::{
    ValidationGatePolicyPersistenceError,
    list_validation_gate_policies as pg_list_validation_gate_policies,
    load_validation_gate_policy as pg_load_validation_gate_policy,
    upsert_validation_gate_policy as pg_upsert_validation_gate_policy,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValidationGatePolicyServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ValidationGateValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_gate_ids: Vec<String>,
}

impl ValidationGatePolicyServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ValidationGateValidationIssue>,
    ) -> Self {
        Self {
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
            failed_gate_ids: Vec::new(),
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: ValidationGateReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for validation gate policy mutations"
                .to_string(),
            field_errors: Vec::new(),
            failed_gate_ids: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: ValidationGateReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for validation gate policy reads".to_string(),
            field_errors: Vec::new(),
            failed_gate_ids: Vec::new(),
        }
    }

    fn policy_not_found(policy_key: &str) -> Self {
        Self {
            code: ValidationGateReasonCode::PolicyNotFound.code(),
            message: format!("validation gate policy `{policy_key}` was not found"),
            field_errors: Vec::new(),
            failed_gate_ids: Vec::new(),
        }
    }

    fn policy_unresolved(stage: ValidationGateWorkflowStage) -> Self {
        Self {
            code: ValidationGateReasonCode::PolicyUnresolved.code(),
            message: format!(
                "validation gate policy catalog is unresolved for workflow stage `{}`",
                stage.as_str()
            ),
            field_errors: vec![ValidationGateValidationIssue {
                field: "stage".to_string(),
                code: ValidationGateReasonCode::PolicyUnresolved.code(),
                message: format!(
                    "no validation gate policies are configured for `{}`",
                    stage.as_str()
                ),
            }],
            failed_gate_ids: Vec::new(),
        }
    }

    fn missing_mandatory_policies(missing_gate_types: Vec<String>) -> Self {
        let message = format!(
            "mandatory FR43 gate policies missing for: {}",
            missing_gate_types.join(", ")
        );
        let failed_gate_ids = missing_gate_types.clone();
        let field_errors = missing_gate_types
            .iter()
            .map(|gate_type| ValidationGateValidationIssue {
                field: format!("mandatory.{gate_type}"),
                code: ValidationGateReasonCode::MissingMandatoryPolicy.code(),
                message: format!("missing mandatory policy for `{gate_type}`"),
            })
            .collect();
        Self {
            code: ValidationGateReasonCode::MissingMandatoryPolicy.code(),
            message,
            field_errors,
            failed_gate_ids,
        }
    }

    fn gate_failed(failed_gate_ids: Vec<String>) -> Self {
        let field_errors = failed_gate_ids
            .iter()
            .map(|policy_key| ValidationGateValidationIssue {
                field: format!("gate.{policy_key}"),
                code: ValidationGateReasonCode::GateFailed.code(),
                message: "gate threshold comparison denied progression".to_string(),
            })
            .collect();
        Self {
            code: ValidationGateReasonCode::GateFailed.code(),
            message: "candidate failed one or more mandatory validation gates".to_string(),
            field_errors,
            failed_gate_ids,
        }
    }

    fn dependency_unavailable(failed_gate_ids: Vec<String>) -> Self {
        let field_errors = failed_gate_ids
            .iter()
            .map(|policy_key| ValidationGateValidationIssue {
                field: format!("gate.{policy_key}"),
                code: ValidationGateReasonCode::DependencyUnavailable.code(),
                message: "required gate input dependency is unavailable".to_string(),
            })
            .collect();
        Self {
            code: ValidationGateReasonCode::DependencyUnavailable.code(),
            message: "required gate input dependency is unavailable".to_string(),
            field_errors,
            failed_gate_ids,
        }
    }

    fn state_unavailable(failed_gate_ids: Vec<String>) -> Self {
        let field_errors = failed_gate_ids
            .iter()
            .map(|policy_key| ValidationGateValidationIssue {
                field: format!("gate.{policy_key}"),
                code: ValidationGateReasonCode::StateUnavailable.code(),
                message: "gate input state is unavailable or ambiguous".to_string(),
            })
            .collect();
        Self {
            code: ValidationGateReasonCode::StateUnavailable.code(),
            message: "gate input state is unavailable or ambiguous".to_string(),
            field_errors,
            failed_gate_ids,
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ValidationGateReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
            failed_gate_ids: Vec::new(),
        }
    }
}

impl Display for ValidationGatePolicyServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ValidationGatePolicyServiceError {}

fn map_contract_error(error: ValidationGateContractError) -> ValidationGatePolicyServiceError {
    ValidationGatePolicyServiceError::invalid_payload(error.message, error.field_errors)
}

fn map_persistence_error(
    error: ValidationGatePolicyPersistenceError,
) -> ValidationGatePolicyServiceError {
    match error.code {
        "validation_gate_policy_query_failed" | "validation_gate_policy_row_decode_failed" => {
            ValidationGatePolicyServiceError::persistence_unavailable(error.message)
        }
        _ => ValidationGatePolicyServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
            failed_gate_ids: Vec::new(),
        },
    }
}

#[derive(Debug, Clone)]
pub struct UpsertValidationGatePolicyInput {
    pub actor_id: String,
    pub actor_role: String,
    pub policy_key: String,
    pub gate_type: String,
    pub stage_scope: String,
    pub metric_key: String,
    pub comparator: String,
    pub threshold_value: f64,
    pub mandatory: bool,
    pub diagnostics: Value,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadValidationGatePolicyInput {
    pub actor_id: String,
    pub actor_role: String,
    pub policy_key: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListValidationGatePoliciesInput {
    pub actor_id: String,
    pub actor_role: String,
    pub stage_filter: Option<String>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct EvaluateValidationGatePoliciesInput {
    pub actor_id: String,
    pub actor_role: String,
    pub candidate_id: String,
    pub stage: String,
    pub observed_metrics: Value,
    pub correlation_id: String,
    pub evaluated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationGatePolicyEvidence {
    pub policy_key: String,
    pub gate_type: String,
    pub stage_scope: String,
    pub metric_key: String,
    pub comparator: String,
    pub threshold_value: f64,
    pub mandatory: bool,
    pub diagnostics: Value,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationGateEvaluationResultEvidence {
    pub policy_key: String,
    pub gate_type: String,
    pub metric_key: String,
    pub comparator: String,
    pub threshold_value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_value: Option<f64>,
    pub passed: bool,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationGateEvaluationEvidence {
    pub candidate_id: String,
    pub stage: String,
    pub outcome: String,
    pub reason_code: String,
    pub failed_gate_ids: Vec<String>,
    pub gate_results: Vec<ValidationGateEvaluationResultEvidence>,
    pub actor_id: String,
    pub correlation_id: String,
    pub evaluated_at_utc: String,
}

pub trait ValidationGatePolicyRepositoryPort: Send + Sync {
    fn upsert(
        &self,
        policy: ValidationGatePolicyDefinition,
    ) -> Result<(), ValidationGatePolicyServiceError>;

    fn load(
        &self,
        policy_key: &str,
    ) -> Result<Option<ValidationGatePolicyDefinition>, ValidationGatePolicyServiceError>;

    fn list(&self)
    -> Result<Vec<ValidationGatePolicyDefinition>, ValidationGatePolicyServiceError>;
}

pub trait ValidationGatePolicyOrchestrator: Send + Sync {
    fn upsert_validation_gate_policy(
        &self,
        input: UpsertValidationGatePolicyInput,
    ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError>;

    fn read_validation_gate_policy(
        &self,
        input: ReadValidationGatePolicyInput,
    ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError>;

    fn list_validation_gate_policies(
        &self,
        input: ListValidationGatePoliciesInput,
    ) -> Result<Vec<ValidationGatePolicyEvidence>, ValidationGatePolicyServiceError>;

    fn evaluate_validation_gates(
        &self,
        input: EvaluateValidationGatePoliciesInput,
    ) -> Result<ValidationGateEvaluationEvidence, ValidationGatePolicyServiceError>;
}

#[derive(Clone)]
pub struct ValidationGatePolicyService {
    repository: Arc<dyn ValidationGatePolicyRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl ValidationGatePolicyService {
    pub fn new(repository: Arc<dyn ValidationGatePolicyRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryValidationGatePolicyRepository::default()))
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresValidationGatePolicyRepository::new(pool)))
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, ValidationGatePolicyServiceError> {
        self.operation_lock.lock().map_err(|_| {
            ValidationGatePolicyServiceError::persistence_unavailable(
                "validation gate policy operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for ValidationGatePolicyService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ValidationGatePolicyOrchestrator for ValidationGatePolicyService {
    fn upsert_validation_gate_policy(
        &self,
        input: UpsertValidationGatePolicyInput,
    ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_validation_gate_telemetry(
                "validation_gate_policy_upsert_v1",
                "validation_gate_policy_upsert",
                "deny",
                &input.actor_id,
                &input.policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }

        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "validation_gate_policy_upsert_v1",
            "validation_gate_policy_upsert",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "policy_key",
            &input.policy_key,
            "validation_gate_policy_upsert_v1",
            "validation_gate_policy_upsert",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_gate_policy_upsert_v1",
            "validation_gate_policy_upsert",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "updated_at_utc",
            &input.updated_at_utc,
            "validation_gate_policy_upsert_v1",
            "validation_gate_policy_upsert",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "updated_at_utc",
            &input.updated_at_utc,
            "validation_gate_policy_upsert_v1",
            "validation_gate_policy_upsert",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;

        let gate_type = ValidationGateType::parse(&input.gate_type)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_upsert_v1",
                    "validation_gate_policy_upsert",
                    "deny",
                    &input.actor_id,
                    &input.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;
        let stage_scope = ValidationGateStageScope::parse(&input.stage_scope)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_upsert_v1",
                    "validation_gate_policy_upsert",
                    "deny",
                    &input.actor_id,
                    &input.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;
        let comparator = ValidationGateComparator::parse(&input.comparator)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_upsert_v1",
                    "validation_gate_policy_upsert",
                    "deny",
                    &input.actor_id,
                    &input.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        let canonical_policy =
            canonicalize_validation_gate_policy_definition(&ValidationGatePolicyDefinition {
                policy_key: input.policy_key.clone(),
                gate_type,
                stage_scope,
                metric_key: input.metric_key.clone(),
                threshold: ValidationGateThreshold {
                    comparator,
                    value: input.threshold_value,
                },
                mandatory: input.mandatory,
                diagnostics: input.diagnostics.clone(),
                actor_id: input.actor_id.clone(),
                correlation_id: input.correlation_id.clone(),
                updated_at_utc: input.updated_at_utc.clone(),
            })
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_upsert_v1",
                    "validation_gate_policy_upsert",
                    "deny",
                    &input.actor_id,
                    &input.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_validation_gate_telemetry(
                "validation_gate_policy_upsert_v1",
                "validation_gate_policy_upsert",
                "deny",
                &input.actor_id,
                &canonical_policy.policy_key,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
        })?;
        let existed = self
            .repository
            .load(&canonical_policy.policy_key)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_upsert_v1",
                    "validation_gate_policy_upsert",
                    "deny",
                    &input.actor_id,
                    &canonical_policy.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?
            .is_some();
        self.repository
            .upsert(canonical_policy.clone())
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_upsert_v1",
                    "validation_gate_policy_upsert",
                    "deny",
                    &input.actor_id,
                    &canonical_policy.policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        let reason_code = if existed {
            ValidationGateReasonCode::PolicyUpdated.code()
        } else {
            ValidationGateReasonCode::PolicyRegistered.code()
        };

        emit_validation_gate_telemetry(
            "validation_gate_policy_upsert_v1",
            "validation_gate_policy_upsert",
            "allow",
            &input.actor_id,
            &canonical_policy.policy_key,
            reason_code,
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(policy_to_evidence(canonical_policy, reason_code))
    }

    fn read_validation_gate_policy(
        &self,
        input: ReadValidationGatePolicyInput,
    ) -> Result<ValidationGatePolicyEvidence, ValidationGatePolicyServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_validation_gate_telemetry(
                "validation_gate_policy_read_v1",
                "validation_gate_policy_read",
                "deny",
                &input.actor_id,
                &input.policy_key,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "validation_gate_policy_read_v1",
            "validation_gate_policy_read",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "policy_key",
            &input.policy_key,
            "validation_gate_policy_read_v1",
            "validation_gate_policy_read",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_gate_policy_read_v1",
            "validation_gate_policy_read",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_gate_policy_read_v1",
            "validation_gate_policy_read",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_gate_policy_read_v1",
            "validation_gate_policy_read",
            &input.actor_id,
            &input.policy_key,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;

        let normalized_policy_key = normalize_research_identifier(&input.policy_key);
        let Some(policy) = self
            .repository
            .load(&normalized_policy_key)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_read_v1",
                    "validation_gate_policy_read",
                    "deny",
                    &input.actor_id,
                    &normalized_policy_key,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?
        else {
            let error = ValidationGatePolicyServiceError::policy_not_found(&normalized_policy_key);
            emit_validation_gate_telemetry(
                "validation_gate_policy_read_v1",
                "validation_gate_policy_read",
                "deny",
                &input.actor_id,
                &normalized_policy_key,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        };

        emit_validation_gate_telemetry(
            "validation_gate_policy_read_v1",
            "validation_gate_policy_read",
            "allow",
            &input.actor_id,
            &policy.policy_key,
            ValidationGateReasonCode::PolicyRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(policy_to_evidence(
            ValidationGatePolicyDefinition {
                actor_id: input.actor_id,
                correlation_id: input.correlation_id,
                updated_at_utc: input.queried_at_utc,
                ..policy
            },
            ValidationGateReasonCode::PolicyRead.code(),
        ))
    }

    fn list_validation_gate_policies(
        &self,
        input: ListValidationGatePoliciesInput,
    ) -> Result<Vec<ValidationGatePolicyEvidence>, ValidationGatePolicyServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_validation_gate_telemetry(
                "validation_gate_policy_list_v1",
                "validation_gate_policy_list",
                "deny",
                &input.actor_id,
                input
                    .stage_filter
                    .as_deref()
                    .unwrap_or("all_validation_gate_policies"),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "validation_gate_policy_list_v1",
            "validation_gate_policy_list",
            &input.actor_id,
            input
                .stage_filter
                .as_deref()
                .unwrap_or("all_validation_gate_policies"),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_gate_policy_list_v1",
            "validation_gate_policy_list",
            &input.actor_id,
            input
                .stage_filter
                .as_deref()
                .unwrap_or("all_validation_gate_policies"),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_gate_policy_list_v1",
            "validation_gate_policy_list",
            &input.actor_id,
            input
                .stage_filter
                .as_deref()
                .unwrap_or("all_validation_gate_policies"),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "validation_gate_policy_list_v1",
            "validation_gate_policy_list",
            &input.actor_id,
            input
                .stage_filter
                .as_deref()
                .unwrap_or("all_validation_gate_policies"),
            &input.correlation_id,
            &input.queried_at_utc,
        )?;

        let stage_filter = input
            .stage_filter
            .as_deref()
            .map(str::trim)
            .filter(|stage| !stage.is_empty())
            .map(ValidationGateWorkflowStage::parse)
            .transpose()
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_policy_list_v1",
                    "validation_gate_policy_list",
                    "deny",
                    &input.actor_id,
                    "all_validation_gate_policies",
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?;

        let policies = self.repository.list().inspect_err(|error| {
            emit_validation_gate_telemetry(
                "validation_gate_policy_list_v1",
                "validation_gate_policy_list",
                "deny",
                &input.actor_id,
                "all_validation_gate_policies",
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
        })?;

        let mut evidence = policies
            .into_iter()
            .filter(|policy| {
                stage_filter
                    .map(|stage| policy.stage_scope.applies_to(stage))
                    .unwrap_or(true)
            })
            .map(|policy| {
                policy_to_evidence(
                    ValidationGatePolicyDefinition {
                        actor_id: input.actor_id.clone(),
                        correlation_id: input.correlation_id.clone(),
                        updated_at_utc: input.queried_at_utc.clone(),
                        ..policy
                    },
                    ValidationGateReasonCode::PolicyListed.code(),
                )
            })
            .collect::<Vec<_>>();
        evidence.sort_by(|left, right| left.policy_key.cmp(&right.policy_key));

        emit_validation_gate_telemetry(
            "validation_gate_policy_list_v1",
            "validation_gate_policy_list",
            "allow",
            &input.actor_id,
            stage_filter.map(|stage| stage.as_str()).unwrap_or("all"),
            ValidationGateReasonCode::PolicyListed.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(evidence)
    }

    fn evaluate_validation_gates(
        &self,
        input: EvaluateValidationGatePoliciesInput,
    ) -> Result<ValidationGateEvaluationEvidence, ValidationGatePolicyServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            &input.actor_id,
            &input.candidate_id,
            &input.correlation_id,
            &input.evaluated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "candidate_id",
            &input.candidate_id,
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            &input.actor_id,
            &input.candidate_id,
            &input.correlation_id,
            &input.evaluated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "stage",
            &input.stage,
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            &input.actor_id,
            &input.candidate_id,
            &input.correlation_id,
            &input.evaluated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            &input.actor_id,
            &input.candidate_id,
            &input.correlation_id,
            &input.evaluated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "evaluated_at_utc",
            &input.evaluated_at_utc,
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            &input.actor_id,
            &input.candidate_id,
            &input.correlation_id,
            &input.evaluated_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "evaluated_at_utc",
            &input.evaluated_at_utc,
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            &input.actor_id,
            &input.candidate_id,
            &input.correlation_id,
            &input.evaluated_at_utc,
        )?;

        let stage = ValidationGateWorkflowStage::parse(&input.stage)
            .map_err(map_contract_error)
            .inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_evaluate_v1",
                    "validation_gate_evaluate",
                    "deny",
                    &input.actor_id,
                    &input.candidate_id,
                    error.code,
                    &input.correlation_id,
                    &input.evaluated_at_utc,
                );
            })?;
        let normalized_candidate_id = normalize_research_identifier(&input.candidate_id);
        if normalized_candidate_id.is_empty() {
            let error = ValidationGatePolicyServiceError::invalid_payload(
                "candidate_id cannot be blank",
                vec![ValidationGateValidationIssue {
                    field: "candidate_id".to_string(),
                    code: ValidationGateReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            );
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &input.candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }

        let observed_metrics =
            normalize_observed_metrics(&input.observed_metrics).inspect_err(|error| {
                emit_validation_gate_telemetry(
                    "validation_gate_evaluate_v1",
                    "validation_gate_evaluate",
                    "deny",
                    &input.actor_id,
                    &normalized_candidate_id,
                    error.code,
                    &input.correlation_id,
                    &input.evaluated_at_utc,
                );
            })?;

        let policies = self.repository.list().inspect_err(|error| {
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
        })?;
        let applicable_policies = policies
            .into_iter()
            .filter(|policy| policy.stage_scope.applies_to(stage))
            .collect::<Vec<_>>();
        if applicable_policies.is_empty() {
            let error = ValidationGatePolicyServiceError::policy_unresolved(stage);
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }

        let mandatory_policies = applicable_policies
            .iter()
            .filter(|policy| policy.mandatory)
            .collect::<Vec<_>>();
        let mut missing_gate_types = FR43_MANDATORY_GATE_TYPES
            .iter()
            .filter(|gate_type| {
                !mandatory_policies
                    .iter()
                    .any(|policy| policy.gate_type == **gate_type)
            })
            .map(|gate_type| gate_type.as_str().to_string())
            .collect::<Vec<_>>();
        if !mandatory_policies
            .iter()
            .any(|policy| policy.gate_type == ValidationGateType::DataQuality)
        {
            missing_gate_types.push(ValidationGateType::DataQuality.as_str().to_string());
        }
        if !missing_gate_types.is_empty() {
            let error =
                ValidationGatePolicyServiceError::missing_mandatory_policies(missing_gate_types);
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }

        let mut gate_results = Vec::new();
        let mut dependency_missing_gate_ids = Vec::new();
        let mut state_unavailable_gate_ids = Vec::new();
        let mut failed_gate_ids = Vec::new();

        let mut ordered_policies = mandatory_policies
            .into_iter()
            .cloned()
            .collect::<Vec<ValidationGatePolicyDefinition>>();
        ordered_policies.sort_by(|left, right| left.policy_key.cmp(&right.policy_key));

        for policy in ordered_policies {
            let metric_key = normalize_research_identifier(&policy.metric_key);
            let observed_value = observed_metrics.get(&metric_key).copied().flatten();

            let (passed, reason_code) = match observed_metrics.get(&metric_key) {
                None => {
                    dependency_missing_gate_ids.push(policy.policy_key.clone());
                    (
                        false,
                        ValidationGateReasonCode::DependencyUnavailable
                            .code()
                            .to_string(),
                    )
                }
                Some(None) => {
                    state_unavailable_gate_ids.push(policy.policy_key.clone());
                    (
                        false,
                        ValidationGateReasonCode::StateUnavailable
                            .code()
                            .to_string(),
                    )
                }
                Some(Some(value)) => {
                    let passed = evaluate_validation_gate_threshold(&policy.threshold, *value)
                        .map_err(map_contract_error)
                        .inspect_err(|error| {
                            emit_validation_gate_telemetry(
                                "validation_gate_evaluate_v1",
                                "validation_gate_evaluate",
                                "deny",
                                &input.actor_id,
                                &normalized_candidate_id,
                                error.code,
                                &input.correlation_id,
                                &input.evaluated_at_utc,
                            );
                        })?;
                    if !passed {
                        failed_gate_ids.push(policy.policy_key.clone());
                    }
                    (
                        passed,
                        if passed {
                            ValidationGateReasonCode::EvaluationAllowed
                                .code()
                                .to_string()
                        } else {
                            ValidationGateReasonCode::GateFailed.code().to_string()
                        },
                    )
                }
            };

            gate_results.push(ValidationGateEvaluationResultEvidence {
                policy_key: policy.policy_key.clone(),
                gate_type: policy.gate_type.as_str().to_string(),
                metric_key,
                comparator: policy.threshold.comparator.as_str().to_string(),
                threshold_value: policy.threshold.value,
                observed_value,
                passed,
                reason_code,
            });
        }

        if !dependency_missing_gate_ids.is_empty() {
            let error = ValidationGatePolicyServiceError::dependency_unavailable(
                dependency_missing_gate_ids,
            );
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }
        if !state_unavailable_gate_ids.is_empty() {
            let error =
                ValidationGatePolicyServiceError::state_unavailable(state_unavailable_gate_ids);
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }
        if !failed_gate_ids.is_empty() {
            let error = ValidationGatePolicyServiceError::gate_failed(failed_gate_ids);
            emit_validation_gate_telemetry(
                "validation_gate_evaluate_v1",
                "validation_gate_evaluate",
                "deny",
                &input.actor_id,
                &normalized_candidate_id,
                error.code,
                &input.correlation_id,
                &input.evaluated_at_utc,
            );
            return Err(error);
        }

        emit_validation_gate_telemetry(
            "validation_gate_evaluate_v1",
            "validation_gate_evaluate",
            "allow",
            &input.actor_id,
            &normalized_candidate_id,
            ValidationGateReasonCode::EvaluationAllowed.code(),
            &input.correlation_id,
            &input.evaluated_at_utc,
        );

        Ok(ValidationGateEvaluationEvidence {
            candidate_id: normalized_candidate_id,
            stage: stage.as_str().to_string(),
            outcome: "allow".to_string(),
            reason_code: ValidationGateReasonCode::EvaluationAllowed
                .code()
                .to_string(),
            failed_gate_ids: Vec::new(),
            gate_results,
            actor_id: input.actor_id,
            correlation_id: input.correlation_id,
            evaluated_at_utc: input.evaluated_at_utc,
        })
    }
}

fn policy_to_evidence(
    policy: ValidationGatePolicyDefinition,
    reason_code: &str,
) -> ValidationGatePolicyEvidence {
    ValidationGatePolicyEvidence {
        policy_key: policy.policy_key,
        gate_type: policy.gate_type.as_str().to_string(),
        stage_scope: policy.stage_scope.as_str().to_string(),
        metric_key: policy.metric_key,
        comparator: policy.threshold.comparator.as_str().to_string(),
        threshold_value: policy.threshold.value,
        mandatory: policy.mandatory,
        diagnostics: policy.diagnostics,
        actor_id: policy.actor_id,
        reason_code: reason_code.to_string(),
        correlation_id: policy.correlation_id,
        updated_at_utc: policy.updated_at_utc,
    }
}

fn validate_mutation_role(role: &str) -> Result<(), ValidationGatePolicyServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ValidationGatePolicyServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), ValidationGatePolicyServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ValidationGatePolicyServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ValidationGatePolicyServiceError> {
    if value.trim().is_empty() {
        return Err(ValidationGatePolicyServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![ValidationGateValidationIssue {
                field: field.to_string(),
                code: ValidationGateReasonCode::InvalidPayload.code(),
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
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), ValidationGatePolicyServiceError> {
    validate_non_empty(field, value).inspect_err(|error| {
        emit_validation_gate_telemetry(
            event_name,
            action,
            "deny",
            actor_id,
            subject_id,
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
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), ValidationGatePolicyServiceError> {
    parse_utc_timestamp(value)
        .map_err(|error| {
            ValidationGatePolicyServiceError::invalid_payload(
                error.message,
                vec![ValidationGateValidationIssue {
                    field: field.to_string(),
                    code: ValidationGateReasonCode::InvalidPayload.code(),
                    message: format!("{field} must be RFC3339 UTC"),
                }],
            )
        })
        .inspect_err(|error| {
            emit_validation_gate_telemetry(
                event_name,
                action,
                "deny",
                actor_id,
                subject_id,
                error.code,
                correlation_id,
                timestamp_utc,
            );
        })?;
    Ok(())
}

fn normalize_observed_metrics(
    observed_metrics: &Value,
) -> Result<BTreeMap<String, Option<f64>>, ValidationGatePolicyServiceError> {
    let Value::Object(entries) = observed_metrics else {
        return Err(ValidationGatePolicyServiceError::invalid_payload(
            "observed_metrics must be a JSON object",
            vec![ValidationGateValidationIssue {
                field: "observed_metrics".to_string(),
                code: ValidationGateReasonCode::InvalidPayload.code(),
                message: "observed_metrics must be a JSON object".to_string(),
            }],
        ));
    };

    let mut normalized = BTreeMap::new();
    for (key, value) in entries {
        let metric_key = normalize_research_identifier(key);
        if metric_key.is_empty() {
            return Err(ValidationGatePolicyServiceError::invalid_payload(
                "observed_metrics contains a blank metric key",
                vec![ValidationGateValidationIssue {
                    field: "observed_metrics".to_string(),
                    code: ValidationGateReasonCode::InvalidPayload.code(),
                    message: "observed metric keys cannot be blank".to_string(),
                }],
            ));
        }
        if normalized.contains_key(&metric_key) {
            return Err(ValidationGatePolicyServiceError::invalid_payload(
                format!("observed_metrics contains duplicate canonical metric key `{metric_key}`"),
                vec![ValidationGateValidationIssue {
                    field: format!("observed_metrics.{metric_key}"),
                    code: ValidationGateReasonCode::InvalidPayload.code(),
                    message: "duplicate observed metric key after canonical normalization"
                        .to_string(),
                }],
            ));
        }
        let metric_field = format!("observed_metrics.{metric_key}");
        let invalid_metric_error = || {
            ValidationGatePolicyServiceError::invalid_payload(
                format!("{metric_field} must be a finite number or null"),
                vec![ValidationGateValidationIssue {
                    field: metric_field.clone(),
                    code: ValidationGateReasonCode::InvalidPayload.code(),
                    message: format!("{metric_field} must be a finite number or null"),
                }],
            )
        };
        let metric_value = match value {
            Value::Number(number) => Some(
                number
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .ok_or_else(invalid_metric_error)?,
            ),
            Value::Null => None,
            _ => return Err(invalid_metric_error()),
        };
        normalized.insert(metric_key, metric_value);
    }
    Ok(normalized)
}

#[allow(clippy::too_many_arguments)]
fn emit_validation_gate_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    subject_id: &str,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = ValidationGateTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        subject_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(ValidationGateSecuritySignal {
                name: "validation_gate_policy_denied_v1",
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
        serde_json::to_string(&event).expect("validation gate telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct ValidationGateTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    subject_id: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<ValidationGateSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct ValidationGateSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresValidationGatePolicyRepository {
    pool: PgPool,
}

impl PostgresValidationGatePolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, ValidationGatePolicyServiceError>
    where
        F: Future<Output = Result<T, ValidationGatePolicyPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ValidationGatePolicyServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl ValidationGatePolicyRepositoryPort for PostgresValidationGatePolicyRepository {
    fn upsert(
        &self,
        policy: ValidationGatePolicyDefinition,
    ) -> Result<(), ValidationGatePolicyServiceError> {
        self.run_with_runtime(pg_upsert_validation_gate_policy(&self.pool, &policy))
    }

    fn load(
        &self,
        policy_key: &str,
    ) -> Result<Option<ValidationGatePolicyDefinition>, ValidationGatePolicyServiceError> {
        self.run_with_runtime(pg_load_validation_gate_policy(&self.pool, policy_key))
    }

    fn list(
        &self,
    ) -> Result<Vec<ValidationGatePolicyDefinition>, ValidationGatePolicyServiceError> {
        self.run_with_runtime(pg_list_validation_gate_policies(&self.pool))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryValidationGatePolicyRepository {
    records: Mutex<BTreeMap<String, ValidationGatePolicyDefinition>>,
}

impl ValidationGatePolicyRepositoryPort for InMemoryValidationGatePolicyRepository {
    fn upsert(
        &self,
        policy: ValidationGatePolicyDefinition,
    ) -> Result<(), ValidationGatePolicyServiceError> {
        self.records
            .lock()
            .expect("in-memory validation gate repository lock should not be poisoned")
            .insert(policy.policy_key.clone(), policy);
        Ok(())
    }

    fn load(
        &self,
        policy_key: &str,
    ) -> Result<Option<ValidationGatePolicyDefinition>, ValidationGatePolicyServiceError> {
        let normalized = normalize_research_identifier(policy_key);
        Ok(self
            .records
            .lock()
            .expect("in-memory validation gate repository lock should not be poisoned")
            .get(&normalized)
            .cloned())
    }

    fn list(
        &self,
    ) -> Result<Vec<ValidationGatePolicyDefinition>, ValidationGatePolicyServiceError> {
        Ok(self
            .records
            .lock()
            .expect("in-memory validation gate repository lock should not be poisoned")
            .values()
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_upsert_input() -> UpsertValidationGatePolicyInput {
        UpsertValidationGatePolicyInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            policy_key: "fr43::forward-bias::primary".to_string(),
            gate_type: "forward_bias".to_string(),
            stage_scope: "training_and_promotion".to_string(),
            metric_key: "forward_bias_score".to_string(),
            comparator: "lte".to_string(),
            threshold_value: 0.12,
            mandatory: true,
            diagnostics: json!({
                "failure_reason": "forward_bias_above_limit",
                "operator_action": "review feature windows"
            }),
            correlation_id: "corr-validation-gate-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    fn seed_mandatory_catalog(
        service: &ValidationGatePolicyService,
    ) -> Result<(), ValidationGatePolicyServiceError> {
        let mut gate = sample_upsert_input();
        gate.policy_key = "fr43::forward-bias::primary".to_string();
        gate.gate_type = "forward_bias".to_string();
        gate.metric_key = "forward_bias_score".to_string();
        gate.comparator = "lte".to_string();
        gate.threshold_value = 0.12;
        gate.mandatory = true;
        service.upsert_validation_gate_policy(gate)?;

        let mut gate = sample_upsert_input();
        gate.policy_key = "fr43::data-leakage::primary".to_string();
        gate.gate_type = "data_leakage".to_string();
        gate.metric_key = "data_leakage_score".to_string();
        gate.comparator = "lt".to_string();
        gate.threshold_value = 0.2;
        gate.mandatory = true;
        service.upsert_validation_gate_policy(gate)?;

        let mut gate = sample_upsert_input();
        gate.policy_key = "fr43::regime-survivability::primary".to_string();
        gate.gate_type = "regime_survivability".to_string();
        gate.metric_key = "regime_survivability_score".to_string();
        gate.comparator = "gte".to_string();
        gate.threshold_value = 0.8;
        gate.mandatory = true;
        service.upsert_validation_gate_policy(gate)?;

        let mut gate = sample_upsert_input();
        gate.policy_key = "fr43::data-quality::training-core".to_string();
        gate.gate_type = "data_quality".to_string();
        gate.stage_scope = "training".to_string();
        gate.metric_key = "data_quality_score".to_string();
        gate.comparator = "gte".to_string();
        gate.threshold_value = 0.95;
        gate.mandatory = true;
        service.upsert_validation_gate_policy(gate)?;

        Ok(())
    }

    #[test]
    fn upsert_rejects_unauthorized_mutation_role() {
        let service = ValidationGatePolicyService::default();
        let mut input = sample_upsert_input();
        input.actor_role = "read_only_analytics".to_string();

        let error = service
            .upsert_validation_gate_policy(input)
            .expect_err("unauthorized mutation role should fail closed");
        assert_eq!(
            error.code,
            ValidationGateReasonCode::UnauthorizedRole.code()
        );
    }

    #[test]
    fn upsert_returns_registered_then_updated_reason_codes() {
        let service = ValidationGatePolicyService::default();

        let first = service
            .upsert_validation_gate_policy(sample_upsert_input())
            .expect("first upsert should register");
        assert_eq!(
            first.reason_code,
            ValidationGateReasonCode::PolicyRegistered.code()
        );

        let second = service
            .upsert_validation_gate_policy(sample_upsert_input())
            .expect("second upsert should update");
        assert_eq!(
            second.reason_code,
            ValidationGateReasonCode::PolicyUpdated.code()
        );
    }

    #[test]
    fn evaluate_denies_when_mandatory_catalog_is_incomplete() {
        let service = ValidationGatePolicyService::default();
        service
            .upsert_validation_gate_policy(sample_upsert_input())
            .expect("seed policy should succeed");

        let error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "forward_bias_score": 0.10
                }),
                correlation_id: "corr-evaluate-001".to_string(),
                evaluated_at_utc: "2026-04-07T00:05:00Z".to_string(),
            })
            .expect_err("incomplete mandatory catalog must fail closed");
        assert_eq!(
            error.code,
            ValidationGateReasonCode::MissingMandatoryPolicy.code()
        );
        assert!(
            error
                .failed_gate_ids
                .iter()
                .any(|id| id == ValidationGateType::DataLeakage.as_str())
        );
        assert!(
            error
                .failed_gate_ids
                .iter()
                .any(|id| id == ValidationGateType::RegimeSurvivability.as_str())
        );
        assert!(
            error
                .failed_gate_ids
                .iter()
                .any(|id| id == ValidationGateType::DataQuality.as_str())
        );
    }

    #[test]
    fn evaluate_denies_when_policy_catalog_is_unresolved_for_stage() {
        let service = ValidationGatePolicyService::default();

        let error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({}),
                correlation_id: "corr-evaluate-001b".to_string(),
                evaluated_at_utc: "2026-04-07T00:05:30Z".to_string(),
            })
            .expect_err("missing policy catalog should fail closed as unresolved");
        assert_eq!(
            error.code,
            ValidationGateReasonCode::PolicyUnresolved.code()
        );
    }

    #[test]
    fn evaluate_denies_when_required_metric_dependency_is_unavailable() {
        let service = ValidationGatePolicyService::default();
        seed_mandatory_catalog(&service).expect("mandatory catalog should seed");

        let error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "forward_bias_score": 0.10,
                    "regime_survivability_score": 0.81,
                    "data_quality_score": 0.97
                }),
                correlation_id: "corr-evaluate-002".to_string(),
                evaluated_at_utc: "2026-04-07T00:06:00Z".to_string(),
            })
            .expect_err("missing data_leakage metric should fail closed");
        assert_eq!(
            error.code,
            ValidationGateReasonCode::DependencyUnavailable.code()
        );
        assert!(
            error
                .failed_gate_ids
                .iter()
                .any(|id| id == "fr43::data-leakage::primary")
        );
    }

    #[test]
    fn evaluate_denies_when_metric_state_is_unavailable() {
        let service = ValidationGatePolicyService::default();
        seed_mandatory_catalog(&service).expect("mandatory catalog should seed");

        let error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "forward_bias_score": 0.10,
                    "data_leakage_score": null,
                    "regime_survivability_score": 0.81,
                    "data_quality_score": 0.97
                }),
                correlation_id: "corr-evaluate-002a".to_string(),
                evaluated_at_utc: "2026-04-07T00:06:15Z".to_string(),
            })
            .expect_err("null metric state should fail closed as unavailable");
        assert_eq!(
            error.code,
            ValidationGateReasonCode::StateUnavailable.code()
        );
        assert!(
            error
                .failed_gate_ids
                .iter()
                .any(|id| id == "fr43::data-leakage::primary")
        );
    }

    #[test]
    fn evaluate_rejects_non_numeric_metric_payload_as_invalid_payload() {
        let service = ValidationGatePolicyService::default();
        seed_mandatory_catalog(&service).expect("mandatory catalog should seed");

        let error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "forward_bias_score": 0.10,
                    "data_leakage_score": "not_a_number",
                    "regime_survivability_score": 0.81,
                    "data_quality_score": 0.97
                }),
                correlation_id: "corr-evaluate-002b".to_string(),
                evaluated_at_utc: "2026-04-07T00:06:30Z".to_string(),
            })
            .expect_err("non-numeric metric payloads must be rejected as invalid payload");
        assert_eq!(error.code, ValidationGateReasonCode::InvalidPayload.code());
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "observed_metrics.data_leakage_score"
                && issue.code == ValidationGateReasonCode::InvalidPayload.code()
        }));
    }

    #[test]
    fn evaluate_rejects_duplicate_canonical_metric_keys_as_invalid_payload() {
        let service = ValidationGatePolicyService::default();
        seed_mandatory_catalog(&service).expect("mandatory catalog should seed");

        let error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "Data_Leakage_Score": 0.19,
                    " data_leakage_score ": 0.10,
                    "forward_bias_score": 0.10,
                    "regime_survivability_score": 0.81,
                    "data_quality_score": 0.97
                }),
                correlation_id: "corr-evaluate-002c".to_string(),
                evaluated_at_utc: "2026-04-07T00:06:45Z".to_string(),
            })
            .expect_err("duplicate canonical metric keys must fail as invalid payload");
        assert_eq!(error.code, ValidationGateReasonCode::InvalidPayload.code());
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "observed_metrics.data_leakage_score"
                && issue.code == ValidationGateReasonCode::InvalidPayload.code()
        }));
    }

    #[test]
    fn evaluate_applies_threshold_boundary_semantics_deterministically() {
        let service = ValidationGatePolicyService::default();
        seed_mandatory_catalog(&service).expect("mandatory catalog should seed");

        let deny_error = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "forward_bias_score": 0.12,
                    "data_leakage_score": 0.2,
                    "regime_survivability_score": 0.80,
                    "data_quality_score": 0.95
                }),
                correlation_id: "corr-evaluate-003".to_string(),
                evaluated_at_utc: "2026-04-07T00:07:00Z".to_string(),
            })
            .expect_err("lt comparator must fail at equality boundary");
        assert_eq!(deny_error.code, ValidationGateReasonCode::GateFailed.code());
        assert!(
            deny_error
                .failed_gate_ids
                .iter()
                .any(|id| id == "fr43::data-leakage::primary")
        );

        let allow = service
            .evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                candidate_id: "candidate::alpha-1".to_string(),
                stage: "training".to_string(),
                observed_metrics: json!({
                    "forward_bias_score": 0.12,
                    "data_leakage_score": 0.19,
                    "regime_survivability_score": 0.80,
                    "data_quality_score": 0.95
                }),
                correlation_id: "corr-evaluate-004".to_string(),
                evaluated_at_utc: "2026-04-07T00:08:00Z".to_string(),
            })
            .expect("adjusted leakage score should pass");
        assert_eq!(allow.outcome, "allow");
        assert_eq!(
            allow.reason_code,
            ValidationGateReasonCode::EvaluationAllowed.code()
        );
    }

    #[test]
    fn list_filters_by_stage_scope() {
        let service = ValidationGatePolicyService::default();
        seed_mandatory_catalog(&service).expect("mandatory catalog should seed");

        let policies = service
            .list_validation_gate_policies(ListValidationGatePoliciesInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                stage_filter: Some("promotion".to_string()),
                correlation_id: "corr-list-001".to_string(),
                queried_at_utc: "2026-04-07T00:09:00Z".to_string(),
            })
            .expect("stage-filtered list should succeed");
        assert!(!policies.is_empty());
        assert!(
            !policies
                .iter()
                .any(|policy| policy.stage_scope == ValidationGateStageScope::Training.as_str())
        );
    }
}
