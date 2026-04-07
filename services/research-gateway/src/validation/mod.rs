pub mod gate_policies;
pub mod hypothesis_registry;

use crate::validation::gate_policies::{
    EvaluateValidationGatePoliciesInput, ValidationGateEvaluationEvidence,
    ValidationGatePolicyOrchestrator, ValidationGatePolicyServiceError,
};
use serde_json::Value;

pub fn evaluate_training_entry_gates(
    orchestrator: &dyn ValidationGatePolicyOrchestrator,
    actor_id: impl Into<String>,
    actor_role: impl Into<String>,
    candidate_id: impl Into<String>,
    observed_metrics: Value,
    correlation_id: impl Into<String>,
    evaluated_at_utc: impl Into<String>,
) -> Result<ValidationGateEvaluationEvidence, ValidationGatePolicyServiceError> {
    orchestrator.evaluate_validation_gates(EvaluateValidationGatePoliciesInput {
        actor_id: actor_id.into(),
        actor_role: actor_role.into(),
        candidate_id: candidate_id.into(),
        stage: "training".to_string(),
        observed_metrics,
        correlation_id: correlation_id.into(),
        evaluated_at_utc: evaluated_at_utc.into(),
    })
}
