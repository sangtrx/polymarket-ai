use domain::governance::{
    AuthorizationDecision, AuthorizationEvaluator, AuthorizationOutcome, AuthorizationReason,
    AuthorizationRequest,
};
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct GovernanceAuthorizationService {
    evaluator: AuthorizationEvaluator,
}

impl GovernanceAuthorizationService {
    pub fn evaluate(
        &self,
        actor_id: &str,
        role: &str,
        action: &str,
        correlation_id: &str,
    ) -> AuthorizationDecision {
        self.evaluator.evaluate(&AuthorizationRequest {
            actor_id: actor_id.to_string(),
            role: role.to_string(),
            action: action.to_string(),
            correlation_id: correlation_id.to_string(),
        })
    }

    pub fn telemetry_evidence(
        &self,
        decision: &AuthorizationDecision,
    ) -> AuthorizationTelemetryEvidence {
        AuthorizationTelemetryEvidence {
            event_name: "authorization_decision_v1".to_string(),
            actor_id: decision.actor_id.clone(),
            role: decision.role.clone(),
            action: decision.action.clone(),
            outcome: outcome_key(decision.outcome).to_string(),
            reason: reason_key(decision.reason).to_string(),
            correlation_id: decision.correlation_id.clone(),
            timestamp_utc: decision.timestamp_utc.clone(),
        }
    }
}

fn outcome_key(outcome: AuthorizationOutcome) -> &'static str {
    match outcome {
        AuthorizationOutcome::Allow => "allow",
        AuthorizationOutcome::Deny => "deny",
    }
}

fn reason_key(reason: AuthorizationReason) -> &'static str {
    match reason {
        AuthorizationReason::RolePermissionGranted => "role_permission_granted",
        AuthorizationReason::InsufficientRole => "insufficient_role",
        AuthorizationReason::UnknownRole => "unknown_role",
        AuthorizationReason::UnknownPermission => "unknown_permission",
        AuthorizationReason::UnknownAction => "unknown_action",
        AuthorizationReason::InvalidRoleAssignment => "invalid_role_assignment",
        AuthorizationReason::DuplicateRolePermissionMapping => "duplicate_role_permission_mapping",
        AuthorizationReason::InvalidActorContext => "invalid_actor_context",
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AuthorizationTelemetryEvidence {
    pub event_name: String,
    pub actor_id: String,
    pub role: String,
    pub action: String,
    pub outcome: String,
    pub reason: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::governance::{AuthorizationOutcome, AuthorizationReason};

    #[test]
    fn telemetry_evidence_contains_traceability_fields() {
        let service = GovernanceAuthorizationService::default();
        let decision = service.evaluate(
            "governance-admin",
            "administrative_actions",
            "manage_role_assignments",
            "corr-governance-01",
        );
        let evidence = service.telemetry_evidence(&decision);

        assert_eq!(evidence.event_name, "authorization_decision_v1");
        assert_eq!(evidence.actor_id, "governance-admin");
        assert_eq!(evidence.role, "administrative_actions");
        assert_eq!(evidence.action, "manage_role_assignments");
        assert_eq!(evidence.outcome, "allow");
        assert_eq!(evidence.reason, "role_permission_granted");
        assert_eq!(evidence.correlation_id, "corr-governance-01");
        assert!(!evidence.timestamp_utc.is_empty());
    }

    #[test]
    fn evaluate_returns_machine_readable_denial_reasons() {
        let service = GovernanceAuthorizationService::default();
        let denied = service.evaluate(
            "analytics-reader",
            "read_only_analytics",
            "manage_role_assignments",
            "corr-governance-02",
        );

        assert_eq!(denied.outcome, AuthorizationOutcome::Deny);
        assert_eq!(denied.reason, AuthorizationReason::InsufficientRole);
        assert_eq!(
            denied.machine_error().map(|error| error.code),
            Some("authorization_denied")
        );
    }
}
