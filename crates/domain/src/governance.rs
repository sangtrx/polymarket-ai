use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceRole {
    ReadOnlyAnalytics,
    OperationalControl,
    AdministrativeActions,
}

impl GovernanceRole {
    pub const ALL: [Self; 3] = [
        Self::ReadOnlyAnalytics,
        Self::OperationalControl,
        Self::AdministrativeActions,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnlyAnalytics => "read_only_analytics",
            Self::OperationalControl => "operational_control",
            Self::AdministrativeActions => "administrative_actions",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AuthorizationError> {
        match value {
            "read_only_analytics" => Ok(Self::ReadOnlyAnalytics),
            "operational_control" => Ok(Self::OperationalControl),
            "administrative_actions" => Ok(Self::AdministrativeActions),
            _ => Err(AuthorizationError::unknown_role(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GovernancePermission {
    ReadAnalytics,
    ExecuteControlAction,
    ManageRoleAssignments,
}

impl GovernancePermission {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadAnalytics => "read_analytics",
            Self::ExecuteControlAction => "execute_control_action",
            Self::ManageRoleAssignments => "manage_role_assignments",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AuthorizationError> {
        match value {
            "read_analytics" => Ok(Self::ReadAnalytics),
            "execute_control_action" => Ok(Self::ExecuteControlAction),
            "manage_role_assignments" => Ok(Self::ManageRoleAssignments),
            _ => Err(AuthorizationError::unknown_permission(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    ReadAnalyticsDashboard,
    ExecuteControlPlaneAction,
    ManageRoleAssignments,
}

impl ControlAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadAnalyticsDashboard => "read_analytics_dashboard",
            Self::ExecuteControlPlaneAction => "execute_control_plane_action",
            Self::ManageRoleAssignments => "manage_role_assignments",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AuthorizationError> {
        match value {
            "read_analytics_dashboard" => Ok(Self::ReadAnalyticsDashboard),
            "execute_control_plane_action" => Ok(Self::ExecuteControlPlaneAction),
            "manage_role_assignments" => Ok(Self::ManageRoleAssignments),
            _ => Err(AuthorizationError::unknown_action(value)),
        }
    }

    pub const fn required_permission(self) -> GovernancePermission {
        match self {
            Self::ReadAnalyticsDashboard => GovernancePermission::ReadAnalytics,
            Self::ExecuteControlPlaneAction => GovernancePermission::ExecuteControlAction,
            Self::ManageRoleAssignments => GovernancePermission::ManageRoleAssignments,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolePermissionMatrix {
    role_permissions: BTreeMap<GovernanceRole, BTreeSet<GovernancePermission>>,
}

impl RolePermissionMatrix {
    pub fn canonical() -> Self {
        let mut role_permissions = BTreeMap::new();
        role_permissions.insert(
            GovernanceRole::ReadOnlyAnalytics,
            BTreeSet::from([GovernancePermission::ReadAnalytics]),
        );
        role_permissions.insert(
            GovernanceRole::OperationalControl,
            BTreeSet::from([
                GovernancePermission::ReadAnalytics,
                GovernancePermission::ExecuteControlAction,
            ]),
        );
        role_permissions.insert(
            GovernanceRole::AdministrativeActions,
            BTreeSet::from([
                GovernancePermission::ReadAnalytics,
                GovernancePermission::ExecuteControlAction,
                GovernancePermission::ManageRoleAssignments,
            ]),
        );

        Self { role_permissions }
    }

    pub fn from_raw_assignments(raw: &[(String, String)]) -> Result<Self, AuthorizationError> {
        let mut role_permissions =
            BTreeMap::<GovernanceRole, BTreeSet<GovernancePermission>>::new();

        for (role_key, permission_key) in raw {
            let role = GovernanceRole::parse(role_key)?;
            let permission = GovernancePermission::parse(permission_key)?;
            let role_set = role_permissions.entry(role).or_default();
            if !role_set.insert(permission) {
                return Err(AuthorizationError::duplicate_role_permission_mapping(
                    role_key,
                    permission_key,
                ));
            }
        }

        let matrix = Self { role_permissions };
        matrix.validate_canonical_boundaries()?;
        Ok(matrix)
    }

    pub fn validate_canonical_boundaries(&self) -> Result<(), AuthorizationError> {
        for role in GovernanceRole::ALL {
            let expected = expected_permissions(role);
            let Some(actual) = self.role_permissions.get(&role) else {
                return Err(AuthorizationError::invalid_role_assignment(format!(
                    "missing mapping for role `{}`",
                    role.as_str()
                )));
            };

            if actual != &expected {
                return Err(AuthorizationError::invalid_role_assignment(format!(
                    "invalid permission set for role `{}`; expected {:?}, got {:?}",
                    role.as_str(),
                    to_permission_keys(&expected),
                    to_permission_keys(actual)
                )));
            }
        }

        if self.role_permissions.len() != GovernanceRole::ALL.len() {
            return Err(AuthorizationError::invalid_role_assignment(
                "role map contains unsupported role entries".to_string(),
            ));
        }

        Ok(())
    }

    pub fn has_permission(
        &self,
        role: GovernanceRole,
        permission: GovernancePermission,
    ) -> Result<bool, AuthorizationError> {
        let role_permissions = self
            .role_permissions
            .get(&role)
            .ok_or_else(|| AuthorizationError::invalid_role_assignment("role not configured"))?;
        Ok(role_permissions.contains(&permission))
    }

    pub fn permissions_for_role(
        &self,
        role: GovernanceRole,
    ) -> Result<&BTreeSet<GovernancePermission>, AuthorizationError> {
        self.role_permissions
            .get(&role)
            .ok_or_else(|| AuthorizationError::invalid_role_assignment("role not configured"))
    }
}

fn expected_permissions(role: GovernanceRole) -> BTreeSet<GovernancePermission> {
    match role {
        GovernanceRole::ReadOnlyAnalytics => BTreeSet::from([GovernancePermission::ReadAnalytics]),
        GovernanceRole::OperationalControl => BTreeSet::from([
            GovernancePermission::ReadAnalytics,
            GovernancePermission::ExecuteControlAction,
        ]),
        GovernanceRole::AdministrativeActions => BTreeSet::from([
            GovernancePermission::ReadAnalytics,
            GovernancePermission::ExecuteControlAction,
            GovernancePermission::ManageRoleAssignments,
        ]),
    }
}

fn to_permission_keys(set: &BTreeSet<GovernancePermission>) -> Vec<&'static str> {
    set.iter().map(|permission| permission.as_str()).collect()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationOutcome {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationReason {
    RolePermissionGranted,
    InsufficientRole,
    UnknownRole,
    UnknownPermission,
    UnknownAction,
    InvalidRoleAssignment,
    DuplicateRolePermissionMapping,
    InvalidActorContext,
}

impl AuthorizationReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RolePermissionGranted => "authorization_allowed",
            Self::InsufficientRole => "authorization_denied",
            Self::UnknownRole => "unknown_role",
            Self::UnknownPermission => "unknown_permission",
            Self::UnknownAction => "unknown_action",
            Self::InvalidRoleAssignment => "invalid_role_assignment",
            Self::DuplicateRolePermissionMapping => "duplicate_role_permission_mapping",
            Self::InvalidActorContext => "invalid_actor_context",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizationError {
    pub code: &'static str,
    pub reason: AuthorizationReason,
    pub message: String,
}

impl AuthorizationError {
    pub fn unknown_role(role: &str) -> Self {
        Self {
            code: AuthorizationReason::UnknownRole.code(),
            reason: AuthorizationReason::UnknownRole,
            message: format!("unknown role `{role}`"),
        }
    }

    pub fn unknown_permission(permission: &str) -> Self {
        Self {
            code: AuthorizationReason::UnknownPermission.code(),
            reason: AuthorizationReason::UnknownPermission,
            message: format!("unknown permission `{permission}`"),
        }
    }

    pub fn unknown_action(action: &str) -> Self {
        Self {
            code: AuthorizationReason::UnknownAction.code(),
            reason: AuthorizationReason::UnknownAction,
            message: format!("unknown action `{action}`"),
        }
    }

    pub fn invalid_role_assignment(message: impl Into<String>) -> Self {
        Self {
            code: AuthorizationReason::InvalidRoleAssignment.code(),
            reason: AuthorizationReason::InvalidRoleAssignment,
            message: message.into(),
        }
    }

    pub fn duplicate_role_permission_mapping(role: &str, permission: &str) -> Self {
        Self {
            code: AuthorizationReason::DuplicateRolePermissionMapping.code(),
            reason: AuthorizationReason::DuplicateRolePermissionMapping,
            message: format!("duplicate role_permission mapping for `{role}` / `{permission}`"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizationDecision {
    pub actor_id: String,
    pub role: String,
    pub action: String,
    pub outcome: AuthorizationOutcome,
    pub reason: AuthorizationReason,
    pub timestamp_utc: String,
    pub correlation_id: String,
}

impl AuthorizationDecision {
    pub fn machine_error(&self) -> Option<AuthorizationError> {
        if self.outcome == AuthorizationOutcome::Allow {
            return None;
        }

        Some(AuthorizationError {
            code: self.reason.code(),
            reason: self.reason,
            message: format!(
                "authorization denied for actor `{}` on action `{}` ({})",
                self.actor_id, self.action, self.role
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegedAuditOutcome {
    Allow,
    AuthorizationDenied,
    AuthenticationDenied,
}

impl PrivilegedAuditOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::AuthorizationDenied => "authorization_denied",
            Self::AuthenticationDenied => "authentication_denied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrivilegedAuditRecord {
    pub actor_id: String,
    pub role: String,
    pub action_type: String,
    pub parameters: Value,
    pub approval_reference: Option<String>,
    pub timestamp: String,
    pub outcome: PrivilegedAuditOutcome,
    pub reason_code: String,
    pub authentication_outcome: String,
    pub correlation_id: String,
}

impl PrivilegedAuditRecord {
    pub fn from_authorization_decision(
        decision: &AuthorizationDecision,
        authentication_outcome: &str,
        parameters: Value,
    ) -> Self {
        let outcome = match decision.outcome {
            AuthorizationOutcome::Allow => PrivilegedAuditOutcome::Allow,
            AuthorizationOutcome::Deny => PrivilegedAuditOutcome::AuthorizationDenied,
        };

        Self {
            actor_id: decision.actor_id.clone(),
            role: decision.role.clone(),
            action_type: decision.action.clone(),
            parameters,
            approval_reference: None,
            timestamp: decision.timestamp_utc.clone(),
            outcome,
            reason_code: decision.reason.code().to_string(),
            authentication_outcome: authentication_outcome.to_string(),
            correlation_id: decision.correlation_id.clone(),
        }
    }

    pub fn from_authentication_denial(
        actor_id: Option<&str>,
        role: Option<&str>,
        action_type: &str,
        reason_code: &str,
        correlation_id: &str,
        timestamp: &str,
        parameters: Value,
    ) -> Self {
        Self {
            actor_id: actor_id
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("unauthenticated")
                .to_string(),
            role: role
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("unknown_role")
                .to_string(),
            action_type: action_type.to_string(),
            parameters,
            approval_reference: None,
            timestamp: timestamp.to_string(),
            outcome: PrivilegedAuditOutcome::AuthenticationDenied,
            reason_code: reason_code.to_string(),
            authentication_outcome: "denied".to_string(),
            correlation_id: correlation_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizationRequest {
    pub actor_id: String,
    pub role: String,
    pub action: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone)]
pub struct AuthorizationEvaluator {
    matrix: RolePermissionMatrix,
}

impl Default for AuthorizationEvaluator {
    fn default() -> Self {
        Self {
            matrix: RolePermissionMatrix::canonical(),
        }
    }
}

impl AuthorizationEvaluator {
    pub fn with_matrix(matrix: RolePermissionMatrix) -> Result<Self, AuthorizationError> {
        matrix.validate_canonical_boundaries()?;
        Ok(Self { matrix })
    }

    pub fn evaluate(&self, request: &AuthorizationRequest) -> AuthorizationDecision {
        if request.actor_id.trim().is_empty() || request.correlation_id.trim().is_empty() {
            return denied_decision(request, AuthorizationReason::InvalidActorContext);
        }

        let role = match GovernanceRole::parse(&request.role) {
            Ok(role) => role,
            Err(_) => return denied_decision(request, AuthorizationReason::UnknownRole),
        };

        let action = match ControlAction::parse(&request.action) {
            Ok(action) => action,
            Err(_) => return denied_decision(request, AuthorizationReason::UnknownAction),
        };

        let required_permission = action.required_permission();
        match self.matrix.has_permission(role, required_permission) {
            Ok(true) => allowed_decision(request),
            Ok(false) => denied_decision(request, AuthorizationReason::InsufficientRole),
            Err(_) => denied_decision(request, AuthorizationReason::InvalidRoleAssignment),
        }
    }

    pub fn matrix(&self) -> &RolePermissionMatrix {
        &self.matrix
    }
}

fn allowed_decision(request: &AuthorizationRequest) -> AuthorizationDecision {
    AuthorizationDecision {
        actor_id: request.actor_id.clone(),
        role: request.role.clone(),
        action: request.action.clone(),
        outcome: AuthorizationOutcome::Allow,
        reason: AuthorizationReason::RolePermissionGranted,
        timestamp_utc: timestamp_utc(),
        correlation_id: request.correlation_id.clone(),
    }
}

fn denied_decision(
    request: &AuthorizationRequest,
    reason: AuthorizationReason,
) -> AuthorizationDecision {
    AuthorizationDecision {
        actor_id: request.actor_id.clone(),
        role: request.role.clone(),
        action: request.action.clone(),
        outcome: AuthorizationOutcome::Deny,
        reason,
        timestamp_utc: timestamp_utc(),
        correlation_id: request.correlation_id.clone(),
    }
}

fn timestamp_utc() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC3339 formatting for authorization telemetry should succeed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_matrix_has_expected_permissions_for_all_roles() {
        let matrix = RolePermissionMatrix::canonical();

        let read_only = matrix
            .permissions_for_role(GovernanceRole::ReadOnlyAnalytics)
            .expect("read-only role should exist");
        assert_eq!(
            read_only,
            &BTreeSet::from([GovernancePermission::ReadAnalytics])
        );

        let operational = matrix
            .permissions_for_role(GovernanceRole::OperationalControl)
            .expect("operational role should exist");
        assert_eq!(
            operational,
            &BTreeSet::from([
                GovernancePermission::ReadAnalytics,
                GovernancePermission::ExecuteControlAction,
            ])
        );

        let admin = matrix
            .permissions_for_role(GovernanceRole::AdministrativeActions)
            .expect("admin role should exist");
        assert_eq!(
            admin,
            &BTreeSet::from([
                GovernancePermission::ReadAnalytics,
                GovernancePermission::ExecuteControlAction,
                GovernancePermission::ManageRoleAssignments,
            ])
        );
    }

    #[test]
    fn raw_assignments_reject_unknown_roles() {
        let result = RolePermissionMatrix::from_raw_assignments(&[(
            "unknown_role".to_string(),
            "read_analytics".to_string(),
        )]);
        assert_eq!(
            result.err().map(|err| err.code),
            Some(AuthorizationReason::UnknownRole.code())
        );
    }

    #[test]
    fn raw_assignments_reject_unknown_permissions() {
        let result = RolePermissionMatrix::from_raw_assignments(&[(
            "read_only_analytics".to_string(),
            "not_a_permission".to_string(),
        )]);
        assert_eq!(
            result.err().map(|err| err.code),
            Some(AuthorizationReason::UnknownPermission.code())
        );
    }

    #[test]
    fn raw_assignments_reject_duplicate_role_permission_entries() {
        let result = RolePermissionMatrix::from_raw_assignments(&[
            (
                "read_only_analytics".to_string(),
                "read_analytics".to_string(),
            ),
            (
                "read_only_analytics".to_string(),
                "read_analytics".to_string(),
            ),
        ]);
        assert_eq!(
            result.err().map(|err| err.code),
            Some(AuthorizationReason::DuplicateRolePermissionMapping.code())
        );
    }

    #[test]
    fn raw_assignments_reject_invalid_boundary_combination() {
        let result = RolePermissionMatrix::from_raw_assignments(&[
            (
                "read_only_analytics".to_string(),
                "read_analytics".to_string(),
            ),
            (
                "read_only_analytics".to_string(),
                "execute_control_action".to_string(),
            ),
            (
                "operational_control".to_string(),
                "read_analytics".to_string(),
            ),
            (
                "operational_control".to_string(),
                "execute_control_action".to_string(),
            ),
            (
                "administrative_actions".to_string(),
                "read_analytics".to_string(),
            ),
            (
                "administrative_actions".to_string(),
                "execute_control_action".to_string(),
            ),
            (
                "administrative_actions".to_string(),
                "manage_role_assignments".to_string(),
            ),
        ]);

        assert_eq!(
            result.err().map(|err| err.code),
            Some(AuthorizationReason::InvalidRoleAssignment.code())
        );
    }

    #[test]
    fn evaluator_enforces_allow_and_deny_boundaries() {
        let evaluator = AuthorizationEvaluator::default();

        let allowed = evaluator.evaluate(&AuthorizationRequest {
            actor_id: "ops-user".to_string(),
            role: "operational_control".to_string(),
            action: "execute_control_plane_action".to_string(),
            correlation_id: "corr-ops-1".to_string(),
        });
        assert_eq!(allowed.outcome, AuthorizationOutcome::Allow);
        assert_eq!(allowed.reason, AuthorizationReason::RolePermissionGranted);
        assert_eq!(allowed.machine_error(), None);

        let denied = evaluator.evaluate(&AuthorizationRequest {
            actor_id: "readonly-user".to_string(),
            role: "read_only_analytics".to_string(),
            action: "execute_control_plane_action".to_string(),
            correlation_id: "corr-readonly-1".to_string(),
        });
        assert_eq!(denied.outcome, AuthorizationOutcome::Deny);
        assert_eq!(denied.reason, AuthorizationReason::InsufficientRole);
        assert_eq!(
            denied.machine_error().map(|error| error.code),
            Some(AuthorizationReason::InsufficientRole.code())
        );
    }

    #[test]
    fn evaluator_denies_unknown_role_and_action() {
        let evaluator = AuthorizationEvaluator::default();

        let unknown_role = evaluator.evaluate(&AuthorizationRequest {
            actor_id: "actor-1".to_string(),
            role: "invalid".to_string(),
            action: "execute_control_plane_action".to_string(),
            correlation_id: "corr-role-1".to_string(),
        });
        assert_eq!(unknown_role.reason, AuthorizationReason::UnknownRole);
        assert_eq!(
            unknown_role.machine_error().map(|error| error.code),
            Some(AuthorizationReason::UnknownRole.code())
        );

        let unknown_action = evaluator.evaluate(&AuthorizationRequest {
            actor_id: "actor-2".to_string(),
            role: "operational_control".to_string(),
            action: "not_a_real_action".to_string(),
            correlation_id: "corr-action-1".to_string(),
        });
        assert_eq!(unknown_action.reason, AuthorizationReason::UnknownAction);
        assert_eq!(
            unknown_action.machine_error().map(|error| error.code),
            Some(AuthorizationReason::UnknownAction.code())
        );
    }

    #[test]
    fn evaluator_emits_rfc3339_timestamps_for_traceability() {
        let evaluator = AuthorizationEvaluator::default();
        let decision = evaluator.evaluate(&AuthorizationRequest {
            actor_id: "ops-user".to_string(),
            role: "operational_control".to_string(),
            action: "execute_control_plane_action".to_string(),
            correlation_id: "corr-ts-1".to_string(),
        });

        let parsed = OffsetDateTime::parse(&decision.timestamp_utc, &Rfc3339);
        assert!(parsed.is_ok(), "timestamp should be RFC3339 UTC");
    }

    #[test]
    fn privileged_audit_record_maps_authorization_decision_context() {
        let evaluator = AuthorizationEvaluator::default();
        let decision = evaluator.evaluate(&AuthorizationRequest {
            actor_id: "ops-user".to_string(),
            role: "operational_control".to_string(),
            action: "execute_control_plane_action".to_string(),
            correlation_id: "corr-audit-map-001".to_string(),
        });

        let record = PrivilegedAuditRecord::from_authorization_decision(
            &decision,
            "authenticated",
            json!({
                "endpoint": "/control/rebalance",
                "http_method": "POST",
            }),
        );

        assert_eq!(record.actor_id, "ops-user");
        assert_eq!(record.role, "operational_control");
        assert_eq!(record.action_type, "execute_control_plane_action");
        assert_eq!(record.approval_reference, None);
        assert_eq!(record.authentication_outcome, "authenticated");
        assert_eq!(record.reason_code, "authorization_allowed");
        assert_eq!(record.outcome, PrivilegedAuditOutcome::Allow);
        assert!(record.parameters.is_object());
        let parsed = OffsetDateTime::parse(&record.timestamp, &Rfc3339);
        assert!(parsed.is_ok(), "audit timestamp should be RFC3339 UTC");
    }

    #[test]
    fn authentication_denial_audit_record_uses_safe_identity_defaults() {
        let record = PrivilegedAuditRecord::from_authentication_denial(
            None,
            None,
            "execute_control_plane_action",
            "auth_missing_credentials",
            "corr-auth-deny-audit-001",
            "2026-04-04T23:59:59Z",
            json!({
                "endpoint": "/control/rebalance",
                "http_method": "POST",
            }),
        );

        assert_eq!(record.actor_id, "unauthenticated");
        assert_eq!(record.role, "unknown_role");
        assert_eq!(record.action_type, "execute_control_plane_action");
        assert_eq!(record.reason_code, "auth_missing_credentials");
        assert_eq!(record.timestamp, "2026-04-04T23:59:59Z");
        assert_eq!(record.authentication_outcome, "denied");
        assert_eq!(record.outcome, PrivilegedAuditOutcome::AuthenticationDenied);
    }
}
