use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::governance::{
    AuthorizationDecision, AuthorizationEvaluator, AuthorizationRequest, ControlAction,
    GovernanceRole, PrivilegedAuditRecord,
};
use governance_service::approvals::{ApprovalOrchestrator, GovernanceApprovalService};
use governance_service::audit::{AuditAppendError, PrivilegedAuditAppender};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedActor {
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub authentication_outcome: AuthenticationOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationOutcome {
    Authenticated,
    Denied,
}

impl AuthenticationOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authenticated => "authenticated",
            Self::Denied => "denied",
        }
    }
}

pub trait AuthorizationGuard: Send + Sync {
    fn evaluate(&self, actor: &AuthenticatedActor, action: ControlAction) -> AuthorizationDecision;
}

pub trait Authenticator: Send + Sync {
    fn authenticate(
        &self,
        headers: &HeaderMap,
    ) -> Result<AuthenticatedActor, Box<AuthenticationError>>;
}

#[derive(Debug, Clone, Default)]
pub struct HeaderTokenAuthenticator;

impl Authenticator for HeaderTokenAuthenticator {
    fn authenticate(
        &self,
        headers: &HeaderMap,
    ) -> Result<AuthenticatedActor, Box<AuthenticationError>> {
        let correlation_id = match read_required_header(headers, "x-correlation-id") {
            Ok(value) => value.to_string(),
            Err(HeaderReadError::Missing) => {
                return Err(Box::new(AuthenticationError::unknown_actor_context(
                    "missing required `x-correlation-id` header",
                    None,
                    None,
                    None,
                )));
            }
            Err(HeaderReadError::InvalidEncoding) => {
                return Err(Box::new(AuthenticationError::unknown_actor_context(
                    "unable to decode `x-correlation-id` header",
                    None,
                    None,
                    None,
                )));
            }
        };

        if !is_valid_identifier(&correlation_id) {
            return Err(Box::new(AuthenticationError::unknown_actor_context(
                "invalid `x-correlation-id` format",
                None,
                None,
                Some(correlation_id),
            )));
        }

        let authorization = match read_required_header(headers, "authorization") {
            Ok(value) => value,
            Err(HeaderReadError::Missing) => {
                return Err(Box::new(AuthenticationError::missing_credentials(Some(
                    correlation_id,
                ))));
            }
            Err(HeaderReadError::InvalidEncoding) => {
                return Err(Box::new(AuthenticationError::verification_failed(
                    "unable to decode `authorization` header",
                    Some(correlation_id),
                )));
            }
        };

        let token = authorization
            .strip_prefix("Bearer ")
            .ok_or_else(|| {
                AuthenticationError::malformed_credentials(
                    "expected Authorization header format: `Bearer <actor_id>:<role>:<expires_unix>`",
                    Some(correlation_id.clone()),
                )
            })
            .map_err(Box::new)?
            .trim();

        let mut segments = token.split(':');
        let actor_id = segments.next().unwrap_or_default().trim();
        let role = segments.next().unwrap_or_default().trim();
        let expires_unix = segments.next().unwrap_or_default().trim();
        if segments.next().is_some()
            || actor_id.is_empty()
            || role.is_empty()
            || expires_unix.is_empty()
        {
            return Err(Box::new(AuthenticationError::malformed_credentials(
                "expected token payload: `<actor_id>:<role>:<expires_unix>`",
                Some(correlation_id),
            )));
        }

        if !is_valid_identifier(actor_id) {
            return Err(Box::new(AuthenticationError::unknown_actor_context(
                "invalid actor id format",
                Some(actor_id.to_string()),
                Some(role.to_string()),
                Some(correlation_id),
            )));
        }

        let canonical_role = GovernanceRole::parse(role)
            .map_err(|_| {
                AuthenticationError::unknown_actor_context(
                    "unknown role in credential context",
                    Some(actor_id.to_string()),
                    Some(role.to_string()),
                    Some(correlation_id.clone()),
                )
            })
            .map_err(Box::new)?;

        let expiry = expires_unix
            .parse::<i64>()
            .map_err(|_| {
                AuthenticationError::invalid_credentials(
                    "token expiry is not a valid unix timestamp",
                    Some(actor_id.to_string()),
                    Some(canonical_role.as_str().to_string()),
                    Some(correlation_id.clone()),
                )
            })
            .map_err(Box::new)?;

        if expiry <= OffsetDateTime::now_utc().unix_timestamp() {
            return Err(Box::new(AuthenticationError::expired_credentials(
                Some(actor_id.to_string()),
                Some(canonical_role.as_str().to_string()),
                Some(correlation_id.clone()),
                expiry,
            )));
        }

        Ok(AuthenticatedActor {
            actor_id: actor_id.to_string(),
            role: canonical_role.as_str().to_string(),
            correlation_id,
            authentication_outcome: AuthenticationOutcome::Authenticated,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct GovernanceAuthorizationGuard {
    evaluator: AuthorizationEvaluator,
}

impl GovernanceAuthorizationGuard {
    pub fn new(evaluator: AuthorizationEvaluator) -> Self {
        Self { evaluator }
    }
}

impl AuthorizationGuard for GovernanceAuthorizationGuard {
    fn evaluate(&self, actor: &AuthenticatedActor, action: ControlAction) -> AuthorizationDecision {
        self.evaluator.evaluate(&AuthorizationRequest {
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            action: action.as_str().to_string(),
            correlation_id: actor.correlation_id.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ControlApiState {
    pub authorization_guard: Arc<dyn AuthorizationGuard>,
    pub authenticator: Arc<dyn Authenticator>,
    pub audit_appender: Arc<dyn PrivilegedAuditAppender>,
    pub approval_orchestrator: Arc<dyn ApprovalOrchestrator>,
}

impl ControlApiState {
    #[allow(dead_code)]
    pub fn new(
        authorization_guard: Arc<dyn AuthorizationGuard>,
        authenticator: Arc<dyn Authenticator>,
        audit_appender: Arc<dyn PrivilegedAuditAppender>,
    ) -> Self {
        Self::with_approval_orchestrator(
            authorization_guard,
            authenticator,
            audit_appender,
            Arc::new(GovernanceApprovalService::default()),
        )
    }

    pub fn with_approval_orchestrator(
        authorization_guard: Arc<dyn AuthorizationGuard>,
        authenticator: Arc<dyn Authenticator>,
        audit_appender: Arc<dyn PrivilegedAuditAppender>,
        approval_orchestrator: Arc<dyn ApprovalOrchestrator>,
    ) -> Self {
        Self {
            authorization_guard,
            authenticator,
            audit_appender,
            approval_orchestrator,
        }
    }
}

pub async fn require_authenticated_actor(
    State(state): State<ControlApiState>,
    mut request: Request,
    next: Next,
) -> Response {
    match state.authenticator.authenticate(request.headers()) {
        Ok(actor) => {
            emit_authentication_telemetry(AuthenticationTelemetryEvent {
                event_name: "control_api_auth_decision_v1",
                signal_name: "privileged_authentication_allowed_v1",
                alert_compatible: false,
                alert_target_seconds: 30,
                actor_id: &actor.actor_id,
                role: &actor.role,
                correlation_id: &actor.correlation_id,
                authentication_outcome: actor.authentication_outcome.as_str(),
                decision_code: "auth_authenticated",
                timestamp_utc: &timestamp_utc(),
            });
            request.extensions_mut().insert(actor);
            next.run(request).await
        }
        Err(error) => {
            let error = *error;
            let timestamp_utc = timestamp_utc();
            let actor_id = error
                .actor_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unauthenticated")
                .to_string();
            let role = error
                .role
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown_role")
                .to_string();
            let correlation_id = error
                .correlation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown_correlation_id")
                .to_string();
            let decision_code = error.code.code();

            emit_authentication_telemetry(AuthenticationTelemetryEvent {
                event_name: "control_api_auth_decision_v1",
                signal_name: "unauthorized_privileged_auth_attempt_v1",
                alert_compatible: true,
                alert_target_seconds: 30,
                actor_id: &actor_id,
                role: &role,
                correlation_id: &correlation_id,
                authentication_outcome: AuthenticationOutcome::Denied.as_str(),
                decision_code,
                timestamp_utc: &timestamp_utc,
            });

            let audit_record = PrivilegedAuditRecord::from_authentication_denial(
                Some(&actor_id),
                Some(&role),
                ControlAction::ExecuteControlPlaneAction.as_str(),
                decision_code,
                &correlation_id,
                &timestamp_utc,
                json!({
                    "endpoint": "/control/rebalance",
                    "http_method": "POST",
                    "decision_code": decision_code,
                }),
            );

            if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
                return audit_append_failure_response(
                    audit_error,
                    AuditAppendFailureContext {
                        action: ControlAction::ExecuteControlPlaneAction
                            .as_str()
                            .to_string(),
                        actor_id,
                        role,
                        correlation_id,
                        authentication_outcome: AuthenticationOutcome::Denied.as_str(),
                        timestamp_utc,
                        attempted_decision_code: Some(decision_code.to_string()),
                    },
                );
            }

            (
                error.code.status_code(),
                Json(AuthenticationErrorEnvelope {
                    error: AuthenticationErrorBody {
                        code: error.code.code(),
                        message: error.message,
                        details: error.details,
                    },
                    actor_id,
                    role,
                    correlation_id,
                    authentication_outcome: AuthenticationOutcome::Denied.as_str(),
                    timestamp_utc,
                    security_signal: AuthenticationSecuritySignal {
                        name: "unauthorized_privileged_auth_attempt_v1",
                        severity: "high",
                        alert_compatible: true,
                        alert_target_seconds: 30,
                    },
                }),
            )
                .into_response()
        }
    }
}

pub(crate) fn audit_append_failure_status(code: &str) -> StatusCode {
    match code {
        "audit_invalid_payload" => StatusCode::BAD_REQUEST,
        "audit_append_constraint_violation" => StatusCode::CONFLICT,
        "audit_persistence_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn audit_append_failure_response(
    audit_error: AuditAppendError,
    context: AuditAppendFailureContext,
) -> Response {
    let details = context.attempted_decision_code.map(|decision_code| {
        json!({
            "attempted_decision_code": decision_code,
        })
    });

    (
        audit_append_failure_status(audit_error.code),
        Json(AuditAppendFailureEnvelope {
            error: AuditAppendFailureBody {
                code: audit_error.code,
                message: audit_error.message,
                details,
            },
            action: context.action,
            actor_id: context.actor_id,
            role: context.role,
            correlation_id: context.correlation_id,
            authentication_outcome: context.authentication_outcome,
            timestamp_utc: context.timestamp_utc,
        }),
    )
        .into_response()
}

struct AuditAppendFailureContext {
    action: String,
    actor_id: String,
    role: String,
    correlation_id: String,
    authentication_outcome: &'static str,
    timestamp_utc: String,
    attempted_decision_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderReadError {
    Missing,
    InvalidEncoding,
}

fn read_required_header<'a>(headers: &'a HeaderMap, key: &str) -> Result<&'a str, HeaderReadError> {
    let value = headers.get(key).ok_or(HeaderReadError::Missing)?;
    let value = value
        .to_str()
        .map_err(|_| HeaderReadError::InvalidEncoding)?
        .trim();
    if value.is_empty() {
        return Err(HeaderReadError::Missing);
    }
    Ok(value)
}

fn is_valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn timestamp_utc() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC3339 timestamp formatting should always succeed")
}

fn emit_authentication_telemetry(event: AuthenticationTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event)
            .expect("auth decision telemetry event should always serialize")
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationErrorCode {
    MissingCredentials,
    MalformedCredentials,
    InvalidCredentials,
    ExpiredCredentials,
    UnknownActorContext,
    VerificationFailed,
}

impl AuthenticationErrorCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingCredentials => "auth_missing_credentials",
            Self::MalformedCredentials => "auth_malformed_credentials",
            Self::InvalidCredentials => "auth_invalid_credentials",
            Self::ExpiredCredentials => "auth_expired_credentials",
            Self::UnknownActorContext => "auth_unknown_actor_context",
            Self::VerificationFailed => "auth_verification_failed",
        }
    }

    pub const fn status_code(self) -> StatusCode {
        match self {
            Self::UnknownActorContext => StatusCode::BAD_REQUEST,
            Self::MissingCredentials
            | Self::MalformedCredentials
            | Self::InvalidCredentials
            | Self::ExpiredCredentials
            | Self::VerificationFailed => StatusCode::UNAUTHORIZED,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticationError {
    pub code: AuthenticationErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub actor_id: Option<String>,
    pub role: Option<String>,
    pub correlation_id: Option<String>,
}

impl AuthenticationError {
    pub fn verification_failed(message: impl Into<String>, correlation_id: Option<String>) -> Self {
        Self {
            code: AuthenticationErrorCode::VerificationFailed,
            message: message.into(),
            details: Some(json!({
                "verification_mode": "provider_adapter",
            })),
            actor_id: None,
            role: None,
            correlation_id,
        }
    }

    fn missing_credentials(correlation_id: Option<String>) -> Self {
        Self {
            code: AuthenticationErrorCode::MissingCredentials,
            message: "missing privileged authentication credentials".to_string(),
            details: Some(json!({
                "required_header": "authorization",
            })),
            actor_id: None,
            role: None,
            correlation_id,
        }
    }

    fn malformed_credentials(message: impl Into<String>, correlation_id: Option<String>) -> Self {
        Self {
            code: AuthenticationErrorCode::MalformedCredentials,
            message: "malformed privileged authentication credentials".to_string(),
            details: Some(json!({
                "reason": message.into(),
            })),
            actor_id: None,
            role: None,
            correlation_id,
        }
    }

    fn invalid_credentials(
        message: impl Into<String>,
        actor_id: Option<String>,
        role: Option<String>,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            code: AuthenticationErrorCode::InvalidCredentials,
            message: "invalid privileged authentication credentials".to_string(),
            details: Some(json!({
                "reason": message.into(),
            })),
            actor_id,
            role,
            correlation_id,
        }
    }

    fn expired_credentials(
        actor_id: Option<String>,
        role: Option<String>,
        correlation_id: Option<String>,
        expires_unix: i64,
    ) -> Self {
        Self {
            code: AuthenticationErrorCode::ExpiredCredentials,
            message: "expired privileged authentication credentials".to_string(),
            details: Some(json!({
                "expires_unix": expires_unix,
            })),
            actor_id,
            role,
            correlation_id,
        }
    }

    fn unknown_actor_context(
        message: impl Into<String>,
        actor_id: Option<String>,
        role: Option<String>,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            code: AuthenticationErrorCode::UnknownActorContext,
            message: "unknown or invalid authenticated actor context".to_string(),
            details: Some(json!({
                "reason": message.into(),
            })),
            actor_id,
            role,
            correlation_id,
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthenticationTelemetryEvent<'a> {
    event_name: &'a str,
    signal_name: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
    actor_id: &'a str,
    role: &'a str,
    correlation_id: &'a str,
    authentication_outcome: &'a str,
    decision_code: &'a str,
    timestamp_utc: &'a str,
}

#[derive(Debug, Serialize)]
struct AuthenticationErrorEnvelope {
    error: AuthenticationErrorBody,
    actor_id: String,
    role: String,
    correlation_id: String,
    authentication_outcome: &'static str,
    timestamp_utc: String,
    security_signal: AuthenticationSecuritySignal,
}

#[derive(Debug, Serialize)]
struct AuthenticationErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct AuthenticationSecuritySignal {
    name: &'static str,
    severity: &'static str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Serialize)]
struct AuditAppendFailureEnvelope {
    error: AuditAppendFailureBody,
    action: String,
    actor_id: String,
    role: String,
    correlation_id: String,
    authentication_outcome: &'static str,
    timestamp_utc: String,
}

#[derive(Debug, Serialize)]
struct AuditAppendFailureBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(actor_id: &str, role: &str, expiry: i64) -> String {
        format!("Bearer {actor_id}:{role}:{expiry}")
    }

    #[test]
    fn authenticator_accepts_valid_token_shape() {
        let authenticator = HeaderTokenAuthenticator;
        let mut headers = HeaderMap::new();
        headers.insert("x-correlation-id", "corr-valid-001".parse().unwrap());
        headers.insert(
            "authorization",
            token("ops-1", "operational_control", 4_102_444_800)
                .parse()
                .unwrap(),
        );

        let actor = authenticator
            .authenticate(&headers)
            .expect("valid token should authenticate");
        assert_eq!(actor.actor_id, "ops-1");
        assert_eq!(actor.role, "operational_control");
        assert_eq!(actor.correlation_id, "corr-valid-001");
        assert_eq!(
            actor.authentication_outcome,
            AuthenticationOutcome::Authenticated
        );
    }

    #[test]
    fn authenticator_rejects_unknown_actor_role() {
        let authenticator = HeaderTokenAuthenticator;
        let mut headers = HeaderMap::new();
        headers.insert("x-correlation-id", "corr-valid-001".parse().unwrap());
        headers.insert(
            "authorization",
            token("ops-1", "guest", 4_102_444_800).parse().unwrap(),
        );

        let error = authenticator
            .authenticate(&headers)
            .expect_err("unknown role should be rejected");
        assert_eq!(error.code, AuthenticationErrorCode::UnknownActorContext);
    }

    #[test]
    fn authenticator_rejects_expired_credentials() {
        let authenticator = HeaderTokenAuthenticator;
        let mut headers = HeaderMap::new();
        headers.insert("x-correlation-id", "corr-valid-001".parse().unwrap());
        headers.insert(
            "authorization",
            token("ops-1", "operational_control", 1).parse().unwrap(),
        );

        let error = authenticator
            .authenticate(&headers)
            .expect_err("expired token should be rejected");
        assert_eq!(error.code, AuthenticationErrorCode::ExpiredCredentials);
    }

    #[test]
    fn authenticator_rejects_non_utf8_correlation_id_as_unknown_actor_context() {
        let authenticator = HeaderTokenAuthenticator;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-correlation-id",
            axum::http::HeaderValue::from_bytes(b"\x80").unwrap(),
        );
        headers.insert(
            "authorization",
            token("ops-1", "operational_control", 4_102_444_800)
                .parse()
                .unwrap(),
        );

        let error = authenticator
            .authenticate(&headers)
            .expect_err("non-utf8 correlation id should be rejected");
        assert_eq!(error.code, AuthenticationErrorCode::UnknownActorContext);
    }

    #[test]
    fn verification_failure_uses_fail_closed_error_code() {
        let failure = AuthenticationError::verification_failed(
            "auth adapter dependency unavailable",
            Some("corr-verify-fail-001".to_string()),
        );

        assert_eq!(failure.code, AuthenticationErrorCode::VerificationFailed);
        assert_eq!(failure.code.status_code(), StatusCode::UNAUTHORIZED);
    }
}
