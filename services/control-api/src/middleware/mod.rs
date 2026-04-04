use axum::http::HeaderMap;
use domain::governance::{
    AuthorizationDecision, AuthorizationEvaluator, AuthorizationRequest, ControlAction,
};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorContext {
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
}

impl ActorContext {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            actor_id: read_header(headers, "x-actor-id"),
            role: read_header(headers, "x-actor-role"),
            correlation_id: read_header(headers, "x-correlation-id"),
        }
    }
}

pub trait AuthorizationGuard: Send + Sync {
    fn evaluate(&self, actor: &ActorContext, action: ControlAction) -> AuthorizationDecision;
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
    fn evaluate(&self, actor: &ActorContext, action: ControlAction) -> AuthorizationDecision {
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
}

impl ControlApiState {
    pub fn new(authorization_guard: Arc<dyn AuthorizationGuard>) -> Self {
        Self {
            authorization_guard,
        }
    }
}

fn read_header(headers: &HeaderMap, key: &str) -> String {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}
