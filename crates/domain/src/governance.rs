use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CriticalActionId {
    StrategyPromotionOverride,
    RiskLimitIncrease,
    KillSwitchDisable,
    ProductionConfigChange,
}

impl CriticalActionId {
    pub const ALL: [Self; 4] = [
        Self::StrategyPromotionOverride,
        Self::RiskLimitIncrease,
        Self::KillSwitchDisable,
        Self::ProductionConfigChange,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StrategyPromotionOverride => "strategy_promotion_override",
            Self::RiskLimitIncrease => "risk_limit_increase",
            Self::KillSwitchDisable => "kill_switch_disable",
            Self::ProductionConfigChange => "production_config_change",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApprovalContractError> {
        match value {
            "strategy_promotion_override" => Ok(Self::StrategyPromotionOverride),
            "risk_limit_increase" => Ok(Self::RiskLimitIncrease),
            "kill_switch_disable" => Ok(Self::KillSwitchDisable),
            "production_config_change" => Ok(Self::ProductionConfigChange),
            _ => Err(ApprovalContractError::invalid_action(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl ApprovalState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApprovalContractError> {
        match value {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            _ => Err(ApprovalContractError::invalid_state(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalVoteDecision {
    Approve,
    Reject,
}

impl ApprovalVoteDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApprovalContractError> {
        match value {
            "approve" => Ok(Self::Approve),
            "reject" => Ok(Self::Reject),
            _ => Err(ApprovalContractError::invalid_vote(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecisionOutcome {
    Allow,
    Deny,
    Pending,
}

impl ApprovalDecisionOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Pending => "pending",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalReasonCode {
    ApprovalAllowed,
    ApprovalPending,
    MissingApprovalRequest,
    UnknownRequest,
    MissingSecondApprover,
    SelfApprovalAttempt,
    DuplicateVote,
    ExpiredWindow,
    UnauthorizedRole,
    RateLimitedActor,
    InvalidStateTransition,
    ExplicitlyRejected,
}

impl ApprovalReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ApprovalAllowed => "approval_allowed",
            Self::ApprovalPending => "approval_pending",
            Self::MissingApprovalRequest => "approval_missing_request",
            Self::UnknownRequest => "approval_unknown_request",
            Self::MissingSecondApprover => "approval_missing_second_approver",
            Self::SelfApprovalAttempt => "approval_self_approval_attempt",
            Self::DuplicateVote => "approval_duplicate_vote",
            Self::ExpiredWindow => "approval_expired_window",
            Self::UnauthorizedRole => "approval_unauthorized_role",
            Self::RateLimitedActor => "approval_rate_limited_actor",
            Self::InvalidStateTransition => "approval_invalid_state_transition",
            Self::ExplicitlyRejected => "approval_rejected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub action_id: String,
    pub proposer_actor_id: String,
    pub status: ApprovalState,
    pub reason_code: String,
    pub correlation_id: String,
    pub approval_reference: Option<String>,
    pub created_at_utc: String,
    pub expires_at_utc: String,
}

impl ApprovalRequest {
    pub fn is_expired_at(&self, now_utc: &str) -> Result<bool, ApprovalContractError> {
        approval_window_expired(now_utc, &self.expires_at_utc)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalVoteRecord {
    pub request_id: String,
    pub actor_id: String,
    pub decision: ApprovalVoteDecision,
    pub correlation_id: String,
    pub voted_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalDecisionEvidence {
    pub request_id: Option<String>,
    pub actor_id: String,
    pub action_id: String,
    pub outcome: ApprovalDecisionOutcome,
    pub state: ApprovalState,
    pub reason_code: String,
    pub correlation_id: String,
    pub approval_reference: Option<String>,
    pub timestamp_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalContractError {
    pub code: &'static str,
    pub message: String,
}

impl ApprovalContractError {
    fn invalid_action(value: &str) -> Self {
        Self {
            code: "approval_invalid_action",
            message: format!("unknown critical action `{value}`"),
        }
    }

    fn invalid_state(value: &str) -> Self {
        Self {
            code: "approval_invalid_state",
            message: format!("unknown approval state `{value}`"),
        }
    }

    fn invalid_vote(value: &str) -> Self {
        Self {
            code: "approval_invalid_vote",
            message: format!("unknown approval vote `{value}`"),
        }
    }

    fn invalid_timestamp(field: &str, value: &str) -> Self {
        Self {
            code: "approval_invalid_timestamp",
            message: format!("invalid RFC3339 UTC timestamp for `{field}`: `{value}`"),
        }
    }

    fn invalid_reference_component(field: &str) -> Self {
        Self {
            code: "approval_invalid_reference_component",
            message: format!("approval reference component `{field}` cannot be blank"),
        }
    }
}

pub fn canonical_actor_id(actor_id: &str) -> String {
    actor_id.trim().to_ascii_lowercase()
}

pub fn actors_are_distinct(proposer_actor_id: &str, approver_actor_id: &str) -> bool {
    !canonical_actor_id(proposer_actor_id).is_empty()
        && !canonical_actor_id(approver_actor_id).is_empty()
        && canonical_actor_id(proposer_actor_id) != canonical_actor_id(approver_actor_id)
}

pub fn approval_window_expired(
    now_utc: &str,
    expires_at_utc: &str,
) -> Result<bool, ApprovalContractError> {
    let now = parse_utc_timestamp("now_utc", now_utc)?;
    let expires = parse_utc_timestamp("expires_at_utc", expires_at_utc)?;
    Ok(now >= expires)
}

pub fn generate_approval_reference(
    request_id: &str,
    action_id: CriticalActionId,
    proposer_actor_id: &str,
    approver_actor_id: &str,
    approved_at_utc: &str,
) -> Result<String, ApprovalContractError> {
    let sanitized_request_id = sanitize_reference_component("request_id", request_id)?;
    let sanitized_proposer = sanitize_reference_component("proposer_actor_id", proposer_actor_id)?;
    let sanitized_approver = sanitize_reference_component("approver_actor_id", approver_actor_id)?;
    let approved_at = parse_utc_timestamp("approved_at_utc", approved_at_utc)?;

    Ok(format!(
        "apr_{}_{}_{}_{}_{}",
        action_id.as_str(),
        sanitized_request_id,
        sanitized_proposer,
        sanitized_approver,
        approved_at.unix_timestamp()
    ))
}

fn sanitize_reference_component(
    field: &'static str,
    value: &str,
) -> Result<String, ApprovalContractError> {
    let canonical = canonical_actor_id(value);
    if canonical.is_empty() {
        return Err(ApprovalContractError::invalid_reference_component(field));
    }

    Ok(canonical
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect())
}

fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ApprovalContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| ApprovalContractError::invalid_timestamp(field, value))?;
    if parsed.offset() != time::UtcOffset::UTC {
        return Err(ApprovalContractError::invalid_timestamp(field, value));
    }
    Ok(parsed)
}

const ROTATION_SCHEDULE_DUE_DAYS: i64 = 90;
const ROTATION_EMERGENCY_DEADLINE_MINUTES: i64 = 30;
const MAX_ROTATION_METADATA_DEPTH: usize = 8;
const MAX_ROTATION_METADATA_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRotationTrigger {
    ScheduledCadence,
    EmergencyCompromise,
}

impl CredentialRotationTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScheduledCadence => "scheduled_cadence",
            Self::EmergencyCompromise => "emergency_compromise",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CredentialRotationContractError> {
        match value {
            "scheduled_cadence" => Ok(Self::ScheduledCadence),
            "emergency_compromise" => Ok(Self::EmergencyCompromise),
            _ => Err(CredentialRotationContractError::invalid_trigger(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRotationState {
    Pending,
    InProgress,
    Succeeded,
    Denied,
    Failed,
}

impl CredentialRotationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Succeeded => "succeeded",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CredentialRotationContractError> {
        match value {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "succeeded" => Ok(Self::Succeeded),
            "denied" => Ok(Self::Denied),
            "failed" => Ok(Self::Failed),
            _ => Err(CredentialRotationContractError::invalid_state(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRotationDecisionOutcome {
    Allow,
    Deny,
    Pending,
}

impl CredentialRotationDecisionOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Pending => "pending",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRotationReasonCode {
    RotationAllowed,
    RotationPending,
    ScheduledNotDue,
    EmergencyWindowExpired,
    UnauthorizedRole,
    MissingMetadata,
    InvalidPayload,
    ProviderUnavailable,
    RuntimeUnavailable,
    ReadinessAmbiguous,
    SecretMaterialRejected,
    InvalidStateTransition,
}

impl CredentialRotationReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RotationAllowed => "credential_rotation_allowed",
            Self::RotationPending => "credential_rotation_pending",
            Self::ScheduledNotDue => "credential_rotation_scheduled_not_due",
            Self::EmergencyWindowExpired => "credential_rotation_emergency_window_expired",
            Self::UnauthorizedRole => "credential_rotation_unauthorized_role",
            Self::MissingMetadata => "credential_rotation_missing_metadata",
            Self::InvalidPayload => "credential_rotation_invalid_payload",
            Self::ProviderUnavailable => "credential_rotation_provider_unavailable",
            Self::RuntimeUnavailable => "credential_rotation_runtime_unavailable",
            Self::ReadinessAmbiguous => "credential_rotation_readiness_ambiguous",
            Self::SecretMaterialRejected => "credential_rotation_secret_material_rejected",
            Self::InvalidStateTransition => "credential_rotation_invalid_state_transition",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CredentialRotationContractError> {
        match value {
            "credential_rotation_allowed" => Ok(Self::RotationAllowed),
            "credential_rotation_pending" => Ok(Self::RotationPending),
            "credential_rotation_scheduled_not_due" => Ok(Self::ScheduledNotDue),
            "credential_rotation_emergency_window_expired" => Ok(Self::EmergencyWindowExpired),
            "credential_rotation_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "credential_rotation_missing_metadata" => Ok(Self::MissingMetadata),
            "credential_rotation_invalid_payload" => Ok(Self::InvalidPayload),
            "credential_rotation_provider_unavailable" => Ok(Self::ProviderUnavailable),
            "credential_rotation_runtime_unavailable" => Ok(Self::RuntimeUnavailable),
            "credential_rotation_readiness_ambiguous" => Ok(Self::ReadinessAmbiguous),
            "credential_rotation_secret_material_rejected" => Ok(Self::SecretMaterialRejected),
            "credential_rotation_invalid_state_transition" => Ok(Self::InvalidStateTransition),
            _ => Err(CredentialRotationContractError::invalid_reason(value)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CredentialRotationEvidence {
    pub rotation_id: String,
    pub actor_id: String,
    pub trigger_type: CredentialRotationTrigger,
    pub credential_scope: String,
    pub credential_reference: String,
    pub outcome: CredentialRotationDecisionOutcome,
    pub status: CredentialRotationState,
    pub reason_code: String,
    pub correlation_id: String,
    pub initiated_at_utc: String,
    pub deadline_at_utc: Option<String>,
    pub completed_at_utc: Option<String>,
    pub rotation_reference: Option<String>,
    pub metadata: Value,
}

impl CredentialRotationEvidence {
    pub fn validate_contract(&self) -> Result<(), CredentialRotationContractError> {
        validate_non_blank_rotation_field("rotation_id", &self.rotation_id)?;
        validate_non_blank_rotation_field("actor_id", &self.actor_id)?;
        validate_non_blank_rotation_field("credential_scope", &self.credential_scope)?;
        validate_reference_like_field("credential_reference", &self.credential_reference)?;
        validate_non_blank_rotation_field("correlation_id", &self.correlation_id)?;
        CredentialRotationReasonCode::parse(&self.reason_code)?;
        parse_rotation_utc_timestamp("initiated_at_utc", &self.initiated_at_utc)?;
        if let Some(deadline) = &self.deadline_at_utc {
            parse_rotation_utc_timestamp("deadline_at_utc", deadline)?;
        }
        if let Some(completed) = &self.completed_at_utc {
            parse_rotation_utc_timestamp("completed_at_utc", completed)?;
        }
        if let Some(reference) = &self.rotation_reference {
            validate_reference_like_field("rotation_reference", reference)?;
        }
        validate_rotation_metadata(&self.metadata)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialRotationContractError {
    pub code: &'static str,
    pub message: String,
}

impl CredentialRotationContractError {
    fn invalid_trigger(value: &str) -> Self {
        Self {
            code: "credential_rotation_invalid_trigger",
            message: format!("unknown credential rotation trigger `{value}`"),
        }
    }

    fn invalid_state(value: &str) -> Self {
        Self {
            code: "credential_rotation_invalid_state",
            message: format!("unknown credential rotation state `{value}`"),
        }
    }

    fn invalid_reason(value: &str) -> Self {
        Self {
            code: "credential_rotation_invalid_reason",
            message: format!("unknown credential rotation reason `{value}`"),
        }
    }

    fn invalid_timestamp(field: &str, value: &str) -> Self {
        Self {
            code: CredentialRotationReasonCode::InvalidPayload.code(),
            message: format!("invalid RFC3339 UTC timestamp for `{field}`: `{value}`"),
        }
    }

    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::InvalidPayload.code(),
            message: message.into(),
        }
    }

    fn missing_metadata(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::MissingMetadata.code(),
            message: message.into(),
        }
    }

    fn secret_material(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::SecretMaterialRejected.code(),
            message: message.into(),
        }
    }
}

pub fn scheduled_rotation_due(
    last_rotated_at_utc: &str,
    now_utc: &str,
) -> Result<bool, CredentialRotationContractError> {
    let last_rotated = parse_rotation_utc_timestamp("last_rotated_at_utc", last_rotated_at_utc)?;
    let now = parse_rotation_utc_timestamp("now_utc", now_utc)?;
    if now < last_rotated {
        return Err(CredentialRotationContractError::invalid_payload(
            "`now_utc` cannot be earlier than `last_rotated_at_utc`",
        ));
    }
    Ok(now - last_rotated >= Duration::days(ROTATION_SCHEDULE_DUE_DAYS))
}

pub fn emergency_rotation_within_deadline(
    compromise_triggered_at_utc: &str,
    completed_at_utc: &str,
) -> Result<bool, CredentialRotationContractError> {
    let triggered =
        parse_rotation_utc_timestamp("compromise_triggered_at_utc", compromise_triggered_at_utc)?;
    let completed = parse_rotation_utc_timestamp("completed_at_utc", completed_at_utc)?;
    if completed < triggered {
        return Err(CredentialRotationContractError::invalid_payload(
            "`completed_at_utc` cannot be earlier than `compromise_triggered_at_utc`",
        ));
    }
    Ok(completed - triggered <= Duration::minutes(ROTATION_EMERGENCY_DEADLINE_MINUTES))
}

pub fn emergency_rotation_deadline_utc(
    compromise_triggered_at_utc: &str,
) -> Result<String, CredentialRotationContractError> {
    let triggered =
        parse_rotation_utc_timestamp("compromise_triggered_at_utc", compromise_triggered_at_utc)?;
    Ok(
        (triggered + Duration::minutes(ROTATION_EMERGENCY_DEADLINE_MINUTES))
            .format(&Rfc3339)
            .expect("RFC3339 formatting for emergency deadline should succeed"),
    )
}

pub fn validate_rotation_metadata(metadata: &Value) -> Result<(), CredentialRotationContractError> {
    if !metadata.is_object() {
        return Err(CredentialRotationContractError::missing_metadata(
            "rotation metadata must be a JSON object",
        ));
    }
    validate_metadata_value(metadata, 0)
}

fn validate_metadata_value(
    value: &Value,
    depth: usize,
) -> Result<(), CredentialRotationContractError> {
    if depth > MAX_ROTATION_METADATA_DEPTH {
        return Err(CredentialRotationContractError::invalid_payload(
            "rotation metadata exceeds maximum nesting depth",
        ));
    }

    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.trim().is_empty() {
                    return Err(CredentialRotationContractError::missing_metadata(
                        "rotation metadata keys cannot be blank",
                    ));
                }
                if looks_like_secret_token(key) {
                    return Err(CredentialRotationContractError::secret_material(format!(
                        "rotation metadata key `{key}` is not allowed"
                    )));
                }
                validate_metadata_value(child, depth + 1)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_metadata_value(child, depth + 1)?;
            }
        }
        Value::String(text) => {
            validate_reference_like_field("metadata", text)?;
        }
        Value::Number(_) | Value::Bool(_) | Value::Null => {}
    }
    Ok(())
}

fn validate_non_blank_rotation_field(
    field: &'static str,
    value: &str,
) -> Result<(), CredentialRotationContractError> {
    if value.trim().is_empty() {
        return Err(CredentialRotationContractError::invalid_payload(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(())
}

fn validate_reference_like_field(
    field: &'static str,
    value: &str,
) -> Result<(), CredentialRotationContractError> {
    let candidate = value.trim();
    if candidate.is_empty() {
        return Err(CredentialRotationContractError::invalid_payload(format!(
            "{field} cannot be blank"
        )));
    }
    if candidate.len() > MAX_ROTATION_METADATA_TEXT_LEN {
        return Err(CredentialRotationContractError::invalid_payload(format!(
            "{field} exceeds maximum supported length"
        )));
    }
    if looks_like_secret_token(candidate) {
        return Err(CredentialRotationContractError::secret_material(format!(
            "{field} appears to contain secret material"
        )));
    }
    if !candidate.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
    }) {
        return Err(CredentialRotationContractError::invalid_payload(format!(
            "{field} must use reference-safe characters only"
        )));
    }
    Ok(())
}

fn looks_like_secret_token(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "api_key",
        "authorization",
        "password",
        "private_key",
        "secret",
        "token",
        "credential_value",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn parse_rotation_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, CredentialRotationContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CredentialRotationContractError::invalid_timestamp(field, value))?;
    if parsed.offset() != time::UtcOffset::UTC {
        return Err(CredentialRotationContractError::invalid_timestamp(
            field, value,
        ));
    }
    Ok(parsed)
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

    #[test]
    fn critical_action_identifiers_are_canonicalized_for_fr34_scope() {
        for action in CriticalActionId::ALL {
            let parsed = CriticalActionId::parse(action.as_str())
                .expect("canonical action identifiers should parse");
            assert_eq!(parsed, action);
        }
        let err =
            CriticalActionId::parse("execute_control_plane_action").expect_err("unknown action");
        assert_eq!(err.code, "approval_invalid_action");
    }

    #[test]
    fn approval_window_expiry_denies_at_exact_boundary() {
        let expires_at = "2026-04-05T00:00:00Z";
        assert!(
            approval_window_expired("2026-04-05T00:00:00Z", expires_at)
                .expect("boundary timestamp should parse")
        );
        assert!(
            !approval_window_expired("2026-04-04T23:59:59Z", expires_at)
                .expect("pre-boundary timestamp should parse")
        );
    }

    #[test]
    fn proposer_and_approver_distinctness_uses_canonical_actor_identity() {
        assert!(actors_are_distinct("ops-one", "admin-two"));
        assert!(!actors_are_distinct("Ops-One", "ops-one"));
        assert!(!actors_are_distinct("ops-one", ""));
    }

    #[test]
    fn approval_reference_generation_is_deterministic_and_non_blank() {
        let reference = generate_approval_reference(
            "req-42",
            CriticalActionId::RiskLimitIncrease,
            "Ops-One",
            "Admin-Two",
            "2026-04-05T00:00:00Z",
        )
        .expect("approval reference generation should succeed");
        assert_eq!(
            reference,
            "apr_risk_limit_increase_req-42_ops-one_admin-two_1775347200"
        );
    }

    #[test]
    fn scheduled_rotation_due_enforces_89_vs_90_day_boundary() {
        assert!(
            !scheduled_rotation_due("2026-01-01T00:00:00Z", "2026-03-31T00:00:00Z")
                .expect("89-day boundary should evaluate")
        );
        assert!(
            scheduled_rotation_due("2026-01-01T00:00:00Z", "2026-04-01T00:00:00Z")
                .expect("90-day boundary should evaluate")
        );
    }

    #[test]
    fn emergency_rotation_deadline_enforces_30_minute_boundary() {
        assert!(
            emergency_rotation_within_deadline("2026-04-05T00:00:00Z", "2026-04-05T00:30:00Z")
                .expect("30-minute deadline should evaluate")
        );
        assert!(
            !emergency_rotation_within_deadline("2026-04-05T00:00:00Z", "2026-04-05T00:31:00Z")
                .expect("31-minute deadline should evaluate")
        );
    }

    #[test]
    fn emergency_rotation_deadline_rejects_non_utc_inputs() {
        let error =
            emergency_rotation_within_deadline("2026-04-05T00:00:00+01:00", "2026-04-05T00:10:00Z")
                .expect_err("non-UTC trigger timestamp must fail");
        assert_eq!(error.code, "credential_rotation_invalid_payload");
    }

    #[test]
    fn rotation_metadata_rejects_secret_like_fields() {
        let error = validate_rotation_metadata(&json!({
            "provider_secret": "abc123"
        }))
        .expect_err("secret-like metadata should fail");
        assert_eq!(error.code, "credential_rotation_secret_material_rejected");
    }

    #[test]
    fn credential_rotation_evidence_contract_requires_reference_safe_values() {
        let evidence = CredentialRotationEvidence {
            rotation_id: "rot-1".to_string(),
            actor_id: "ops-1".to_string(),
            trigger_type: CredentialRotationTrigger::ScheduledCadence,
            credential_scope: "control_api".to_string(),
            credential_reference: "vault://control-api/prod".to_string(),
            outcome: CredentialRotationDecisionOutcome::Allow,
            status: CredentialRotationState::Succeeded,
            reason_code: CredentialRotationReasonCode::RotationAllowed
                .code()
                .to_string(),
            correlation_id: "corr-rot-1".to_string(),
            initiated_at_utc: "2026-04-05T00:00:00Z".to_string(),
            deadline_at_utc: Some("2026-04-05T00:30:00Z".to_string()),
            completed_at_utc: Some("2026-04-05T00:05:00Z".to_string()),
            rotation_reference: Some("rotref://vault/control-api/42".to_string()),
            metadata: json!({
                "crypto_posture_verified": true,
                "runtime_injection_mode": "runtime_only",
                "provider_ref": "vault://team/control-api"
            }),
        };
        assert!(evidence.validate_contract().is_ok());
    }
}
