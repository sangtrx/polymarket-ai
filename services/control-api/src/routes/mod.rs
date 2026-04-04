use crate::middleware::{AuthenticatedActor, ControlApiState, require_authenticated_actor};
use axum::{
    Router,
    extract::{Extension, State},
    http::StatusCode,
    middleware as axum_middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::governance::{
    AuthorizationDecision, AuthorizationOutcome, AuthorizationReason, ControlAction,
};
use serde::Serialize;

pub fn app_router(state: ControlApiState) -> Router {
    let privileged_routes = Router::new().route(
        "/control/rebalance",
        post(rebalance_portfolio).route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        )),
    );

    Router::new()
        .route("/health", get(health))
        .merge(privileged_routes)
        .with_state(state)
}

pub async fn health() -> &'static str {
    "ok"
}

pub async fn rebalance_portfolio(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let decision = state
        .authorization_guard
        .evaluate(&actor, ControlAction::ExecuteControlPlaneAction);

    emit_authorization_telemetry(&decision, actor.authentication_outcome.as_str());

    if decision.outcome == AuthorizationOutcome::Allow {
        return (
            StatusCode::ACCEPTED,
            axum::Json(ControlActionAccepted {
                status: "accepted",
                action: decision.action,
                actor_id: decision.actor_id,
                role: decision.role,
                outcome: "allow",
                authentication_outcome: actor.authentication_outcome.as_str(),
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.timestamp_utc,
            }),
        )
            .into_response();
    }

    let machine_error = decision
        .machine_error()
        .expect("denied authorization decisions always produce machine errors");
    let status = match decision.reason {
        AuthorizationReason::UnknownRole
        | AuthorizationReason::UnknownAction
        | AuthorizationReason::InvalidActorContext => StatusCode::BAD_REQUEST,
        _ => StatusCode::FORBIDDEN,
    };

    (
        status,
        axum::Json(ControlActionDenied {
            error_code: machine_error.code,
            reason: reason_key(decision.reason),
            message: machine_error.message,
            action: decision.action,
            actor_id: decision.actor_id,
            role: decision.role,
            authentication_outcome: actor.authentication_outcome.as_str(),
            correlation_id: decision.correlation_id,
            timestamp_utc: decision.timestamp_utc,
        }),
    )
        .into_response()
}

fn emit_authorization_telemetry(
    decision: &AuthorizationDecision,
    authentication_outcome: &'static str,
) {
    let telemetry_event = AuthorizationTelemetryEvent {
        event_name: "authorization_decision_v1",
        actor_id: &decision.actor_id,
        role: &decision.role,
        action: &decision.action,
        outcome: match decision.outcome {
            AuthorizationOutcome::Allow => "allow",
            AuthorizationOutcome::Deny => "deny",
        },
        reason: reason_key(decision.reason),
        authentication_outcome,
        correlation_id: &decision.correlation_id,
        timestamp_utc: &decision.timestamp_utc,
    };

    println!(
        "{}",
        serde_json::to_string(&telemetry_event)
            .expect("authorization telemetry event should always serialize")
    );
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

#[derive(Debug, Serialize)]
struct AuthorizationTelemetryEvent<'a> {
    event_name: &'a str,
    actor_id: &'a str,
    role: &'a str,
    action: &'a str,
    outcome: &'a str,
    reason: &'a str,
    authentication_outcome: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
}

#[derive(Debug, Serialize)]
pub struct ControlActionAccepted {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub outcome: &'static str,
    pub authentication_outcome: &'static str,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[derive(Debug, Serialize)]
pub struct ControlActionDenied {
    pub error_code: &'static str,
    pub reason: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub authentication_outcome: &'static str,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::{
        AuthenticatedActor, AuthenticationError, Authenticator, AuthorizationGuard,
        ControlApiState, GovernanceAuthorizationGuard, HeaderTokenAuthenticator,
    };
    use axum::{
        body::{Body, to_bytes},
        http::HeaderMap,
        http::Request,
    };
    use domain::governance::{AuthorizationDecision, AuthorizationEvaluator, ControlAction};
    use std::sync::Arc;
    use tower::ServiceExt;

    fn bearer_token(actor_id: &str, role: &str, expires_unix: i64) -> String {
        format!("Bearer {actor_id}:{role}:{expires_unix}")
    }

    fn test_app() -> Router {
        app_router(ControlApiState::new(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(HeaderTokenAuthenticator),
        ))
    }

    fn test_app_with_state(state: ControlApiState) -> Router {
        app_router(state)
    }

    #[tokio::test]
    async fn health_endpoint_remains_non_privileged() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .method("GET")
                    .body(Body::empty())
                    .expect("health request should build"),
            )
            .await
            .expect("health request should complete");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_credentials_reject_privileged_request_with_machine_readable_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header("x-correlation-id", "corr-auth-missing-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_missing_credentials");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-auth-missing-001");
        assert_eq!(
            payload["security_signal"]["name"],
            "unauthorized_privileged_auth_attempt_v1"
        );
        assert_eq!(payload["security_signal"]["alert_compatible"], true);
        assert_eq!(payload["security_signal"]["alert_target_seconds"], 30);
    }

    #[tokio::test]
    async fn malformed_credentials_reject_privileged_request_with_machine_readable_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header("authorization", "Token malformed")
                    .header("x-correlation-id", "corr-auth-malformed-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_malformed_credentials");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-auth-malformed-001");
    }

    #[tokio::test]
    async fn expired_credentials_reject_privileged_request_with_machine_readable_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 1),
                    )
                    .header("x-correlation-id", "corr-auth-expired-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_expired_credentials");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-auth-expired-001");
    }

    #[tokio::test]
    async fn invalid_expiry_material_rejects_privileged_request_with_machine_readable_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        "Bearer ops-1:operational_control:not-a-timestamp",
                    )
                    .header("x-correlation-id", "corr-auth-invalid-expiry-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_invalid_credentials");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-auth-invalid-expiry-001");
    }

    #[tokio::test]
    async fn invalid_actor_id_format_rejects_privileged_request_with_machine_readable_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops@1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-auth-invalid-actor-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_unknown_actor_context");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-auth-invalid-actor-001");
    }

    #[tokio::test]
    async fn invalid_correlation_id_format_rejects_privileged_request_with_machine_readable_error()
    {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr invalid format 001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_unknown_actor_context");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr invalid format 001");
        assert_eq!(
            payload["security_signal"]["name"],
            "unauthorized_privileged_auth_attempt_v1"
        );
        assert_eq!(payload["security_signal"]["alert_compatible"], true);
    }

    #[tokio::test]
    async fn denied_control_path_returns_machine_readable_authorization_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("analytics-reader", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-deny-001")
                    .body(Body::empty())
                    .expect("deny request should build"),
            )
            .await
            .expect("deny request should complete");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("deny payload should be valid json");

        assert_eq!(payload["error_code"], "authorization_denied");
        assert_eq!(payload["reason"], "insufficient_role");
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["correlation_id"], "corr-deny-001");
    }

    #[tokio::test]
    async fn denied_control_path_includes_traceability_evidence_fields() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("analytics-reader", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-deny-002")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("payload should be valid json");

        assert_eq!(payload["error_code"], "authorization_denied");
        assert_eq!(payload["reason"], "insufficient_role");
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["actor_id"], "analytics-reader");
        assert_eq!(payload["role"], "read_only_analytics");
        assert_eq!(payload["correlation_id"], "corr-deny-002");

        let message = payload["message"]
            .as_str()
            .expect("deny payload should include machine-readable message");
        assert!(
            message.contains("authorization denied"),
            "deny payload message should explain authorization failure"
        );

        let timestamp_utc = payload["timestamp_utc"]
            .as_str()
            .expect("deny payload should include timestamp evidence");
        assert!(
            timestamp_utc.contains('T') && timestamp_utc.ends_with('Z'),
            "deny payload timestamp should be RFC3339 UTC"
        );
    }

    #[tokio::test]
    async fn unknown_actor_context_role_returns_machine_readable_auth_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "unsupported_role", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-unknown-actor-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_unknown_actor_context");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-unknown-actor-001");
    }

    #[tokio::test]
    async fn missing_correlation_id_returns_invalid_actor_context_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_unknown_actor_context");
        assert_eq!(payload["authentication_outcome"], "denied");
    }

    #[tokio::test]
    async fn operational_control_role_is_allowed_for_control_path() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-allow-001")
                    .body(Body::empty())
                    .expect("allow request should build"),
            )
            .await
            .expect("allow request should complete");

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("allow payload should be valid json");

        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["authentication_outcome"], "authenticated");
        assert_eq!(payload["outcome"], "allow");
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["actor_id"], "ops-1");
        assert_eq!(payload["role"], "operational_control");
        assert_eq!(payload["correlation_id"], "corr-allow-001");
    }

    #[tokio::test]
    async fn administrative_actions_role_allow_path_includes_timestamp_traceability() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("admin-1", "administrative_actions", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-allow-admin-001")
                    .body(Body::empty())
                    .expect("allow request should build"),
            )
            .await
            .expect("allow request should complete");

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("allow payload should be valid json");

        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["outcome"], "allow");
        assert_eq!(payload["authentication_outcome"], "authenticated");
        assert_eq!(payload["actor_id"], "admin-1");
        assert_eq!(payload["role"], "administrative_actions");
        assert_eq!(payload["correlation_id"], "corr-allow-admin-001");

        let timestamp_utc = payload["timestamp_utc"]
            .as_str()
            .expect("allow payload should include timestamp evidence");
        assert!(
            timestamp_utc.contains('T') && timestamp_utc.ends_with('Z'),
            "allow payload timestamp should be RFC3339 UTC"
        );
    }

    #[tokio::test]
    async fn role_boundary_matrix_is_deterministic_for_control_rebalance() {
        let scenarios = [
            (
                "analytics-reader",
                "read_only_analytics",
                "corr-matrix-001",
                StatusCode::FORBIDDEN,
            ),
            (
                "ops-1",
                "operational_control",
                "corr-matrix-002",
                StatusCode::ACCEPTED,
            ),
            (
                "admin-1",
                "administrative_actions",
                "corr-matrix-003",
                StatusCode::ACCEPTED,
            ),
        ];

        for (actor_id, role, correlation_id, expected_status) in scenarios {
            let response = test_app()
                .oneshot(
                    Request::builder()
                        .uri("/control/rebalance")
                        .method("POST")
                        .header("authorization", bearer_token(actor_id, role, 4_102_444_800))
                        .header("x-correlation-id", correlation_id)
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("request should complete");

            assert_eq!(
                response.status(),
                expected_status,
                "role boundary for `{role}` should be deterministic"
            );

            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable");
            let payload: serde_json::Value =
                serde_json::from_slice(&body).expect("payload should be valid json");

            assert_eq!(payload["action"], "execute_control_plane_action");
            assert_eq!(payload["actor_id"], actor_id);
            assert_eq!(payload["role"], role);
            assert_eq!(payload["correlation_id"], correlation_id);
            assert_eq!(payload["authentication_outcome"], "authenticated");

            if expected_status == StatusCode::FORBIDDEN {
                assert_eq!(payload["error_code"], "authorization_denied");
                assert_eq!(payload["reason"], "insufficient_role");
            } else {
                assert_eq!(payload["status"], "accepted");
                assert_eq!(payload["outcome"], "allow");
            }
        }
    }

    #[derive(Debug)]
    struct FailingAuthenticator;

    impl Authenticator for FailingAuthenticator {
        fn authenticate(
            &self,
            headers: &HeaderMap,
        ) -> Result<AuthenticatedActor, Box<AuthenticationError>> {
            let correlation_id = headers
                .get("x-correlation-id")
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_string());
            Err(Box::new(AuthenticationError::verification_failed(
                "authentication adapter unavailable",
                correlation_id,
            )))
        }
    }

    #[derive(Debug)]
    struct PanicAuthorizationGuard;

    impl AuthorizationGuard for PanicAuthorizationGuard {
        fn evaluate(
            &self,
            _actor: &AuthenticatedActor,
            _action: ControlAction,
        ) -> AuthorizationDecision {
            panic!("authorization guard should not be invoked when authentication fails")
        }
    }

    #[tokio::test]
    async fn authenticator_adapter_failure_is_fail_closed() {
        let app = test_app_with_state(ControlApiState::new(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(FailingAuthenticator),
        ));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-auth-adapter-failure-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["error"]["code"], "auth_verification_failed");
        assert_eq!(payload["authentication_outcome"], "denied");
        assert_eq!(payload["correlation_id"], "corr-auth-adapter-failure-001");
    }

    #[tokio::test]
    async fn authentication_failures_do_not_invoke_authorization_guard() {
        let app = test_app_with_state(ControlApiState::new(
            Arc::new(PanicAuthorizationGuard),
            Arc::new(HeaderTokenAuthenticator),
        ));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header("x-correlation-id", "corr-pre-execution-guard-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["error"]["code"], "auth_missing_credentials");
    }
}
