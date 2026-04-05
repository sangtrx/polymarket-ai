use crate::middleware::{
    AuthenticatedActor, ControlApiState, audit_append_failure_status, require_authenticated_actor,
};
use axum::{
    Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    middleware as axum_middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::governance::{
    ApprovalDecisionEvidence, ApprovalDecisionOutcome, ApprovalReasonCode, AuthorizationDecision,
    AuthorizationOutcome, AuthorizationReason, ControlAction, CredentialRotationDecisionOutcome,
    CredentialRotationEvidence, CredentialRotationReasonCode, PrivilegedAuditOutcome,
    PrivilegedAuditRecord,
};
use governance_service::approvals::{
    EvaluateApprovalExecutionInput, RecordApprovalVoteInput, SubmitApprovalRequestInput,
};
use governance_service::audit::AuditAppendError;
use governance_service::credentials::{
    TriggerEmergencyRotationInput, TriggerScheduledRotationInput,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub fn app_router(state: ControlApiState) -> Router {
    let privileged_routes = Router::new().route(
        "/control/rebalance",
        post(rebalance_portfolio).route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        )),
    );
    let critical_routes = Router::new()
        .route(
            "/control/critical-actions/{action_id}/requests/{request_id}",
            post(submit_critical_action_request),
        )
        .route(
            "/control/critical-actions/{action_id}/requests/{request_id}/votes",
            post(record_critical_action_vote),
        )
        .route(
            "/control/critical-actions/{action_id}/requests/{request_id}/execute",
            post(execute_critical_action),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let credential_rotation_routes = Router::new()
        .route(
            "/control/credentials/rotation/scheduled",
            post(trigger_scheduled_credential_rotation),
        )
        .route(
            "/control/credentials/rotation/emergency",
            post(trigger_emergency_credential_rotation),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));

    Router::new()
        .route("/health", get(health))
        .merge(privileged_routes)
        .merge(critical_routes)
        .merge(credential_rotation_routes)
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

    let audit_record = PrivilegedAuditRecord::from_authorization_decision(
        &decision,
        actor.authentication_outcome.as_str(),
        json!({
            "endpoint": "/control/rebalance",
            "http_method": "POST",
        }),
    );

    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            decision.action.clone(),
            decision.actor_id.clone(),
            decision.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.timestamp_utc.clone(),
        );
    }

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

pub async fn submit_critical_action_request(
    State(state): State<ControlApiState>,
    Path((action_id, request_id)): Path<(String, String)>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<CriticalActionRequestPayload>,
) -> Response {
    let endpoint = format!("/control/critical-actions/{action_id}/requests/{request_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let decision = match state
        .approval_orchestrator
        .submit_request(SubmitApprovalRequestInput {
            request_id,
            action_id,
            proposer_actor_id: actor.actor_id.clone(),
            proposer_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            now_utc: authorization.timestamp_utc.clone(),
            expires_at_utc: payload.expires_at_utc,
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return approval_service_error_response(
                error.code,
                error.message,
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    critical_approval_response(&state, &actor, decision, endpoint, "request_submission")
}

pub async fn record_critical_action_vote(
    State(state): State<ControlApiState>,
    Path((action_id, request_id)): Path<(String, String)>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<CriticalActionVotePayload>,
) -> Response {
    let endpoint = format!("/control/critical-actions/{action_id}/requests/{request_id}/votes");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let vote = match domain::governance::ApprovalVoteDecision::parse(&payload.decision) {
        Ok(vote) => vote,
        Err(error) => {
            return approval_service_error_response(
                error.code,
                error.message,
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let decision = match state
        .approval_orchestrator
        .record_vote(RecordApprovalVoteInput {
            request_id,
            action_id,
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            vote,
            correlation_id: actor.correlation_id.clone(),
            now_utc: authorization.timestamp_utc.clone(),
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return approval_service_error_response(
                error.code,
                error.message,
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    critical_approval_response(&state, &actor, decision, endpoint, "approval_vote")
}

pub async fn execute_critical_action(
    State(state): State<ControlApiState>,
    Path((action_id, request_id)): Path<(String, String)>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/critical-actions/{action_id}/requests/{request_id}/execute");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let decision =
        match state
            .approval_orchestrator
            .evaluate_execution(EvaluateApprovalExecutionInput {
                request_id,
                action_id,
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                correlation_id: actor.correlation_id.clone(),
                now_utc: authorization.timestamp_utc.clone(),
            }) {
            Ok(decision) => decision,
            Err(error) => {
                return approval_service_error_response(
                    error.code,
                    error.message,
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };

    critical_approval_response(&state, &actor, decision, endpoint, "execution_gate")
}

pub async fn trigger_scheduled_credential_rotation(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<ScheduledCredentialRotationPayload>,
) -> Response {
    let endpoint = "/control/credentials/rotation/scheduled";
    let authorization = match authorize_critical_action(&state, &actor, endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let decision = match state
        .credential_rotation_orchestrator
        .trigger_scheduled_rotation(TriggerScheduledRotationInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            credential_scope: payload.credential_scope,
            credential_reference: payload.credential_reference,
            metadata: payload.metadata,
            correlation_id: actor.correlation_id.clone(),
            now_utc: authorization.timestamp_utc.clone(),
            last_rotated_at_utc: payload.last_rotated_at_utc,
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return credential_rotation_service_error_response(
                error.code,
                error.message,
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint.to_string(),
            );
        }
    };

    credential_rotation_response(&state, &actor, decision, endpoint.to_string())
}

pub async fn trigger_emergency_credential_rotation(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<EmergencyCredentialRotationPayload>,
) -> Response {
    let endpoint = "/control/credentials/rotation/emergency";
    let authorization = match authorize_critical_action(&state, &actor, endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let decision = match state
        .credential_rotation_orchestrator
        .trigger_emergency_rotation(TriggerEmergencyRotationInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            credential_scope: payload.credential_scope,
            credential_reference: payload.credential_reference,
            metadata: payload.metadata,
            correlation_id: actor.correlation_id.clone(),
            now_utc: authorization.timestamp_utc.clone(),
            compromise_triggered_at_utc: payload.compromise_triggered_at_utc,
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return credential_rotation_service_error_response(
                error.code,
                error.message,
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint.to_string(),
            );
        }
    };

    credential_rotation_response(&state, &actor, decision, endpoint.to_string())
}

fn authorize_critical_action(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    endpoint: &str,
) -> Result<AuthorizationDecision, Box<Response>> {
    let decision = state
        .authorization_guard
        .evaluate(actor, ControlAction::ExecuteControlPlaneAction);
    emit_authorization_telemetry(&decision, actor.authentication_outcome.as_str());

    let audit_record = PrivilegedAuditRecord::from_authorization_decision(
        &decision,
        actor.authentication_outcome.as_str(),
        json!({
            "endpoint": endpoint,
            "http_method": "POST",
        }),
    );
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return Err(Box::new(audit_append_failure_response(
            audit_error,
            decision.action.clone(),
            decision.actor_id.clone(),
            decision.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.timestamp_utc.clone(),
        )));
    }

    if decision.outcome == AuthorizationOutcome::Allow {
        return Ok(decision);
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

    Err(Box::new(
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
            .into_response(),
    ))
}

fn critical_approval_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: ApprovalDecisionEvidence,
    endpoint: String,
    stage: &'static str,
) -> Response {
    if let Some(audit_outcome) = critical_approval_audit_outcome(decision.outcome) {
        let audit_record = PrivilegedAuditRecord {
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            action_type: decision.action_id.clone(),
            parameters: json!({
                "endpoint": endpoint,
                "http_method": "POST",
                "approval_stage": stage,
                "request_id": decision.request_id.as_deref(),
            }),
            approval_reference: if audit_outcome == PrivilegedAuditOutcome::Allow {
                decision.approval_reference.clone()
            } else {
                None
            },
            timestamp: decision.timestamp_utc.clone(),
            outcome: audit_outcome,
            reason_code: decision.reason_code.clone(),
            authentication_outcome: actor.authentication_outcome.as_str().to_string(),
            correlation_id: decision.correlation_id.clone(),
        };

        if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
            return audit_append_failure_response(
                audit_error,
                decision.action_id.clone(),
                actor.actor_id.clone(),
                actor.role.clone(),
                actor.authentication_outcome.as_str(),
                decision.correlation_id.clone(),
                decision.timestamp_utc.clone(),
            );
        }
    }

    match decision.outcome {
        ApprovalDecisionOutcome::Allow => (
            StatusCode::ACCEPTED,
            axum::Json(CriticalActionDecisionResponse {
                status: "accepted",
                error_code: None,
                message: None,
                action: decision.action_id,
                request_id: decision
                    .request_id
                    .unwrap_or_else(|| "unknown_request".to_string()),
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                outcome: decision.outcome.as_str(),
                state: decision.state.as_str(),
                reason_code: decision.reason_code,
                approval_reference: decision.approval_reference,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.timestamp_utc,
                security_signal: None,
            }),
        )
            .into_response(),
        ApprovalDecisionOutcome::Pending => (
            StatusCode::ACCEPTED,
            axum::Json(CriticalActionDecisionResponse {
                status: "pending",
                error_code: None,
                message: None,
                action: decision.action_id,
                request_id: decision
                    .request_id
                    .unwrap_or_else(|| "unknown_request".to_string()),
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                outcome: decision.outcome.as_str(),
                state: decision.state.as_str(),
                reason_code: decision.reason_code,
                approval_reference: decision.approval_reference,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.timestamp_utc,
                security_signal: None,
            }),
        )
            .into_response(),
        ApprovalDecisionOutcome::Deny => (
            approval_denied_status(decision.reason_code.as_str()),
            axum::Json(CriticalActionDecisionResponse {
                status: "denied",
                error_code: Some(decision.reason_code.clone()),
                message: Some(approval_reason_message(decision.reason_code.as_str()).to_string()),
                action: decision.action_id,
                request_id: decision
                    .request_id
                    .unwrap_or_else(|| "unknown_request".to_string()),
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                outcome: decision.outcome.as_str(),
                state: decision.state.as_str(),
                reason_code: decision.reason_code,
                approval_reference: None,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.timestamp_utc,
                security_signal: Some(CriticalApprovalSecuritySignal {
                    name: "unauthorized_privileged_approval_attempt_v1",
                    severity: "high",
                    alert_compatible: true,
                    alert_target_seconds: 30,
                }),
            }),
        )
            .into_response(),
    }
}

fn credential_rotation_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: CredentialRotationEvidence,
    endpoint: String,
) -> Response {
    if let Some(audit_outcome) = credential_rotation_audit_outcome(decision.outcome) {
        let action_name = format!("credential_rotation_{}", decision.trigger_type.as_str());
        let audit_record = PrivilegedAuditRecord {
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            action_type: action_name.clone(),
            parameters: json!({
                "endpoint": endpoint,
                "http_method": "POST",
                "trigger_type": decision.trigger_type.as_str(),
                "credential_scope": decision.credential_scope,
                "rotation_id": decision.rotation_id,
            }),
            approval_reference: if audit_outcome == PrivilegedAuditOutcome::Allow {
                decision.rotation_reference.clone()
            } else {
                None
            },
            timestamp: decision
                .completed_at_utc
                .clone()
                .unwrap_or_else(|| decision.initiated_at_utc.clone()),
            outcome: audit_outcome,
            reason_code: decision.reason_code.clone(),
            authentication_outcome: actor.authentication_outcome.as_str().to_string(),
            correlation_id: decision.correlation_id.clone(),
        };

        if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
            return audit_append_failure_response(
                audit_error,
                action_name,
                actor.actor_id.clone(),
                actor.role.clone(),
                actor.authentication_outcome.as_str(),
                decision.correlation_id.clone(),
                decision
                    .completed_at_utc
                    .clone()
                    .unwrap_or_else(|| decision.initiated_at_utc.clone()),
            );
        }
    }

    match decision.outcome {
        CredentialRotationDecisionOutcome::Allow => (
            StatusCode::ACCEPTED,
            axum::Json(CredentialRotationDecisionResponse {
                status: "accepted",
                error_code: None,
                message: None,
                trigger_type: decision.trigger_type.as_str().to_string(),
                rotation_id: decision.rotation_id,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                outcome: decision.outcome.as_str(),
                state: decision.status.as_str(),
                reason_code: decision.reason_code,
                credential_scope: decision.credential_scope,
                rotation_reference: decision.rotation_reference,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision
                    .completed_at_utc
                    .unwrap_or(decision.initiated_at_utc),
                security_signal: None,
            }),
        )
            .into_response(),
        CredentialRotationDecisionOutcome::Pending => (
            StatusCode::ACCEPTED,
            axum::Json(CredentialRotationDecisionResponse {
                status: "pending",
                error_code: None,
                message: None,
                trigger_type: decision.trigger_type.as_str().to_string(),
                rotation_id: decision.rotation_id,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                outcome: decision.outcome.as_str(),
                state: decision.status.as_str(),
                reason_code: decision.reason_code,
                credential_scope: decision.credential_scope,
                rotation_reference: decision.rotation_reference,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.initiated_at_utc,
                security_signal: None,
            }),
        )
            .into_response(),
        CredentialRotationDecisionOutcome::Deny => (
            credential_rotation_denied_status(decision.reason_code.as_str()),
            axum::Json(CredentialRotationDecisionResponse {
                status: "denied",
                error_code: Some(decision.reason_code.clone()),
                message: Some(
                    credential_rotation_reason_message(decision.reason_code.as_str()).to_string(),
                ),
                trigger_type: decision.trigger_type.as_str().to_string(),
                rotation_id: decision.rotation_id,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                outcome: decision.outcome.as_str(),
                state: decision.status.as_str(),
                reason_code: decision.reason_code,
                credential_scope: decision.credential_scope,
                rotation_reference: None,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision
                    .completed_at_utc
                    .unwrap_or(decision.initiated_at_utc),
                security_signal: Some(CredentialRotationSecuritySignal {
                    name: "unauthorized_privileged_credential_rotation_attempt_v1",
                    severity: "high",
                    alert_compatible: true,
                    alert_target_seconds: 30,
                }),
            }),
        )
            .into_response(),
    }
}

fn credential_rotation_audit_outcome(
    outcome: CredentialRotationDecisionOutcome,
) -> Option<PrivilegedAuditOutcome> {
    match outcome {
        CredentialRotationDecisionOutcome::Allow => Some(PrivilegedAuditOutcome::Allow),
        CredentialRotationDecisionOutcome::Deny => {
            Some(PrivilegedAuditOutcome::AuthorizationDenied)
        }
        CredentialRotationDecisionOutcome::Pending => None,
    }
}

fn credential_rotation_denied_status(reason_code: &str) -> StatusCode {
    match reason_code {
        code if code == CredentialRotationReasonCode::InvalidPayload.code()
            || code == CredentialRotationReasonCode::MissingMetadata.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        code if code == CredentialRotationReasonCode::ProviderUnavailable.code()
            || code == CredentialRotationReasonCode::RuntimeUnavailable.code() =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        code if code == CredentialRotationReasonCode::ScheduledNotDue.code() => {
            StatusCode::CONFLICT
        }
        _ => StatusCode::FORBIDDEN,
    }
}

fn credential_rotation_reason_message(reason_code: &str) -> &'static str {
    match reason_code {
        code if code == CredentialRotationReasonCode::ScheduledNotDue.code() => {
            "scheduled rotation cadence is not yet due"
        }
        code if code == CredentialRotationReasonCode::EmergencyWindowExpired.code() => {
            "emergency rotation window has expired"
        }
        code if code == CredentialRotationReasonCode::ReadinessAmbiguous.code() => {
            "rotation readiness constraints are ambiguous and fail-closed"
        }
        code if code == CredentialRotationReasonCode::ProviderUnavailable.code() => {
            "secret provider dependency unavailable during rotation"
        }
        code if code == CredentialRotationReasonCode::RuntimeUnavailable.code() => {
            "runtime dependency unavailable during rotation"
        }
        code if code == CredentialRotationReasonCode::InvalidPayload.code() => {
            "credential rotation payload is invalid"
        }
        code if code == CredentialRotationReasonCode::MissingMetadata.code() => {
            "credential rotation metadata is missing required fields"
        }
        code if code == CredentialRotationReasonCode::UnauthorizedRole.code() => {
            "actor role is not authorized for credential rotation workflow"
        }
        code if code == CredentialRotationReasonCode::SecretMaterialRejected.code() => {
            "credential rotation payload appears to contain secret material"
        }
        _ => "credential rotation request was denied",
    }
}

fn credential_rotation_service_error_response(
    error_code: &'static str,
    message: String,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        credential_rotation_service_error_status(error_code),
        axum::Json(CredentialRotationServiceErrorResponse {
            error_code,
            message,
            action: "credential_rotation_workflow".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            security_signal: Some(CredentialRotationSecuritySignal {
                name: "unauthorized_privileged_credential_rotation_attempt_v1",
                severity: "high",
                alert_compatible: true,
                alert_target_seconds: 30,
            }),
        }),
    )
        .into_response()
}

fn credential_rotation_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == CredentialRotationReasonCode::InvalidPayload.code()
            || code == CredentialRotationReasonCode::MissingMetadata.code()
            || code == CredentialRotationReasonCode::SecretMaterialRejected.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        code if code == CredentialRotationReasonCode::UnauthorizedRole.code() => {
            StatusCode::FORBIDDEN
        }
        "credential_rotation_duplicate_rotation_id"
        | "credential_rotation_duplicate_reference"
        | "credential_rotation_constraint_violation"
        | "credential_rotation_invalid_state_transition" => StatusCode::CONFLICT,
        "credential_rotation_not_found" => StatusCode::BAD_REQUEST,
        "credential_rotation_persistence_unavailable"
        | "credential_rotation_query_failed"
        | "credential_rotation_row_decode_failed" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn critical_approval_audit_outcome(
    outcome: ApprovalDecisionOutcome,
) -> Option<PrivilegedAuditOutcome> {
    match outcome {
        ApprovalDecisionOutcome::Allow => Some(PrivilegedAuditOutcome::Allow),
        ApprovalDecisionOutcome::Deny => Some(PrivilegedAuditOutcome::AuthorizationDenied),
        ApprovalDecisionOutcome::Pending => None,
    }
}

fn approval_denied_status(reason_code: &str) -> StatusCode {
    match reason_code {
        code if code == ApprovalReasonCode::MissingApprovalRequest.code()
            || code == ApprovalReasonCode::UnknownRequest.code()
            || code == ApprovalReasonCode::InvalidStateTransition.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        _ => StatusCode::FORBIDDEN,
    }
}

fn approval_reason_message(reason_code: &str) -> &'static str {
    match reason_code {
        code if code == ApprovalReasonCode::MissingApprovalRequest.code() => {
            "approval request is required before execution"
        }
        code if code == ApprovalReasonCode::UnknownRequest.code() => {
            "approval request was not found"
        }
        code if code == ApprovalReasonCode::MissingSecondApprover.code() => {
            "a second distinct approver is required"
        }
        code if code == ApprovalReasonCode::SelfApprovalAttempt.code() => {
            "proposer and approver must be different actors"
        }
        code if code == ApprovalReasonCode::DuplicateVote.code() => {
            "actor has already cast a vote for this request"
        }
        code if code == ApprovalReasonCode::ExpiredWindow.code() => {
            "approval request has expired and cannot be used"
        }
        code if code == ApprovalReasonCode::UnauthorizedRole.code() => {
            "actor role is not authorized for critical approval workflow"
        }
        code if code == ApprovalReasonCode::RateLimitedActor.code() => {
            "actor exceeded the per-hour critical approval request limit"
        }
        code if code == ApprovalReasonCode::InvalidStateTransition.code() => {
            "approval request is in an invalid state for this operation"
        }
        code if code == ApprovalReasonCode::ExplicitlyRejected.code() => {
            "approval request has been explicitly rejected"
        }
        _ => "critical approval request was denied",
    }
}

fn approval_service_error_response(
    error_code: &'static str,
    message: String,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        approval_service_error_status(error_code),
        axum::Json(CriticalApprovalServiceErrorResponse {
            error_code,
            message,
            action: "critical_approval_workflow".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            security_signal: Some(CriticalApprovalSecuritySignal {
                name: "unauthorized_privileged_approval_attempt_v1",
                severity: "high",
                alert_compatible: true,
                alert_target_seconds: 30,
            }),
        }),
    )
        .into_response()
}

fn approval_service_error_status(code: &str) -> StatusCode {
    match code {
        "approval_invalid_payload" | "approval_invalid_action" | "approval_invalid_vote" => {
            StatusCode::BAD_REQUEST
        }
        "approval_duplicate_request"
        | "approval_duplicate_vote"
        | "approval_constraint_violation" => StatusCode::CONFLICT,
        "approval_request_not_found" => StatusCode::BAD_REQUEST,
        "approval_persistence_unavailable"
        | "approval_query_failed"
        | "approval_row_decode_failed" => StatusCode::SERVICE_UNAVAILABLE,
        "approval_unauthorized_role" => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn audit_append_failure_response(
    audit_error: AuditAppendError,
    action: String,
    actor_id: String,
    role: String,
    authentication_outcome: &'static str,
    correlation_id: String,
    timestamp_utc: String,
) -> Response {
    (
        audit_append_failure_status(audit_error.code),
        axum::Json(ControlActionAuditFailure {
            error_code: audit_error.code,
            message: audit_error.message,
            action,
            actor_id,
            role,
            authentication_outcome,
            correlation_id,
            timestamp_utc,
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

#[derive(Debug, Serialize)]
pub struct ControlActionAuditFailure {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub authentication_outcome: &'static str,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[derive(Debug, Deserialize)]
pub struct CriticalActionRequestPayload {
    pub expires_at_utc: String,
}

#[derive(Debug, Deserialize)]
pub struct CriticalActionVotePayload {
    pub decision: String,
}

#[derive(Debug, Serialize)]
pub struct CriticalActionDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub action: String,
    pub request_id: String,
    pub actor_id: String,
    pub role: String,
    pub outcome: &'static str,
    pub state: &'static str,
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<CriticalApprovalSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct CriticalApprovalServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<CriticalApprovalSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct CriticalApprovalSecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u16,
}

#[derive(Debug, Deserialize)]
pub struct ScheduledCredentialRotationPayload {
    pub credential_scope: String,
    pub credential_reference: String,
    pub last_rotated_at_utc: String,
    #[serde(default = "default_rotation_metadata")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct EmergencyCredentialRotationPayload {
    pub credential_scope: String,
    pub credential_reference: String,
    pub compromise_triggered_at_utc: String,
    #[serde(default = "default_rotation_metadata")]
    pub metadata: serde_json::Value,
}

fn default_rotation_metadata() -> serde_json::Value {
    json!({})
}

#[derive(Debug, Serialize)]
pub struct CredentialRotationDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub trigger_type: String,
    pub rotation_id: String,
    pub actor_id: String,
    pub role: String,
    pub outcome: &'static str,
    pub state: &'static str,
    pub reason_code: String,
    pub credential_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation_reference: Option<String>,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<CredentialRotationSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct CredentialRotationServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<CredentialRotationSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct CredentialRotationSecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u16,
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
    use domain::governance::{
        AuthorizationDecision, AuthorizationEvaluator, ControlAction, PrivilegedAuditOutcome,
        PrivilegedAuditRecord,
    };
    use governance_service::audit::{AuditAppendError, PrivilegedAuditAppender};
    use std::sync::{Arc, Mutex};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    use tower::ServiceExt;

    fn bearer_token(actor_id: &str, role: &str, expires_unix: i64) -> String {
        format!("Bearer {actor_id}:{role}:{expires_unix}")
    }

    #[derive(Debug, Default)]
    struct CapturingAuditAppender {
        records: Mutex<Vec<PrivilegedAuditRecord>>,
    }

    impl CapturingAuditAppender {
        fn snapshot(&self) -> Vec<PrivilegedAuditRecord> {
            self.records
                .lock()
                .expect("captured audit records lock should not be poisoned")
                .clone()
        }
    }

    impl PrivilegedAuditAppender for CapturingAuditAppender {
        fn append_privileged_audit(
            &self,
            record: PrivilegedAuditRecord,
        ) -> Result<(), AuditAppendError> {
            self.records
                .lock()
                .expect("captured audit records lock should not be poisoned")
                .push(record);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FailingAuditAppender {
        code: &'static str,
    }

    impl PrivilegedAuditAppender for FailingAuditAppender {
        fn append_privileged_audit(
            &self,
            _record: PrivilegedAuditRecord,
        ) -> Result<(), AuditAppendError> {
            let error = match self.code {
                "audit_invalid_payload" => AuditAppendError::invalid_payload("invalid payload"),
                "audit_append_constraint_violation" => {
                    AuditAppendError::append_constraint_violation("append-only violation")
                }
                _ => AuditAppendError::persistence_unavailable("persistence unavailable"),
            };
            Err(error)
        }
    }

    fn test_app() -> Router {
        test_app_with_audit_appender(Arc::new(CapturingAuditAppender::default()))
    }

    fn test_app_with_audit_appender(audit_appender: Arc<dyn PrivilegedAuditAppender>) -> Router {
        app_router(ControlApiState::new(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(HeaderTokenAuthenticator),
            audit_appender,
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
        assert_eq!(payload["actor_id"], "unauthenticated");
        assert_eq!(payload["role"], "unknown_role");
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
        assert_eq!(payload["actor_id"], "unauthenticated");
        assert_eq!(payload["role"], "unknown_role");
        assert_eq!(payload["correlation_id"], "unknown_correlation_id");
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

    #[tokio::test]
    async fn allow_path_appends_audit_record_with_required_traceability_contract() {
        let audit_appender = Arc::new(CapturingAuditAppender::default());
        let app = test_app_with_audit_appender(audit_appender.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-allow-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let records = audit_appender.snapshot();
        assert_eq!(
            records.len(),
            1,
            "allow path should append exactly one record"
        );

        let record = &records[0];
        assert_eq!(record.actor_id, "ops-1");
        assert_eq!(record.role, "operational_control");
        assert_eq!(record.action_type, "execute_control_plane_action");
        assert_eq!(record.approval_reference, None);
        assert_eq!(record.correlation_id, "corr-audit-allow-001");
        assert_eq!(record.authentication_outcome, "authenticated");
        assert_eq!(record.reason_code, "authorization_allowed");
        assert_eq!(record.outcome, PrivilegedAuditOutcome::Allow);
        assert_eq!(record.parameters["endpoint"], "/control/rebalance");
        assert_eq!(record.parameters["http_method"], "POST");

        let record_timestamp = OffsetDateTime::parse(&record.timestamp, &Rfc3339)
            .expect("audit timestamp should be RFC3339 UTC");
        let audit_latency_seconds = (OffsetDateTime::now_utc() - record_timestamp)
            .whole_seconds()
            .abs();
        assert!(
            audit_latency_seconds <= 5,
            "audit record timestamp should be within FR32/NFR9 latency bounds"
        );
    }

    #[tokio::test]
    async fn denied_and_authentication_failure_paths_append_expected_audit_outcomes() {
        let audit_appender = Arc::new(CapturingAuditAppender::default());
        let app = test_app_with_audit_appender(audit_appender.clone());

        let denied_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("analytics-reader", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-deny-001")
                    .body(Body::empty())
                    .expect("deny request should build"),
            )
            .await
            .expect("deny request should complete");
        assert_eq!(denied_response.status(), StatusCode::FORBIDDEN);

        let auth_failure_response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header("x-correlation-id", "corr-audit-auth-deny-001")
                    .body(Body::empty())
                    .expect("auth-failure request should build"),
            )
            .await
            .expect("auth-failure request should complete");
        assert_eq!(auth_failure_response.status(), StatusCode::UNAUTHORIZED);

        let records = audit_appender.snapshot();
        assert_eq!(
            records.len(),
            2,
            "deny and auth-failure paths should append one record each"
        );

        assert_eq!(
            records[0].outcome,
            PrivilegedAuditOutcome::AuthorizationDenied
        );
        assert_eq!(records[0].reason_code, "authorization_denied");
        assert_eq!(
            records[1].outcome,
            PrivilegedAuditOutcome::AuthenticationDenied
        );
        assert_eq!(records[1].reason_code, "auth_missing_credentials");
    }

    #[tokio::test]
    async fn allow_path_is_fail_closed_when_audit_append_is_unavailable() {
        let app = test_app_with_audit_appender(Arc::new(FailingAuditAppender {
            code: "audit_persistence_unavailable",
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-fail-allow-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["error_code"], "audit_persistence_unavailable");
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["authentication_outcome"], "authenticated");
    }

    #[tokio::test]
    async fn audit_invalid_payload_returns_explicit_machine_readable_error() {
        let app = test_app_with_audit_appender(Arc::new(FailingAuditAppender {
            code: "audit_invalid_payload",
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-invalid-payload-001")
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
        assert_eq!(payload["error_code"], "audit_invalid_payload");
    }

    #[tokio::test]
    async fn authentication_failure_returns_explicit_audit_append_error_when_append_fails() {
        let app = test_app_with_audit_appender(Arc::new(FailingAuditAppender {
            code: "audit_append_constraint_violation",
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header("x-correlation-id", "corr-audit-auth-fail-append-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(
            payload["error"]["code"],
            "audit_append_constraint_violation"
        );
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["authentication_outcome"], "denied");
    }

    #[tokio::test]
    async fn denied_path_returns_explicit_audit_append_error_when_append_fails() {
        let app = test_app_with_audit_appender(Arc::new(FailingAuditAppender {
            code: "audit_append_constraint_violation",
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("analytics-reader", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-deny-append-fail-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["error_code"], "audit_append_constraint_violation");
        assert_eq!(payload["action"], "execute_control_plane_action");
        assert_eq!(payload["actor_id"], "analytics-reader");
        assert_eq!(payload["role"], "read_only_analytics");
        assert_eq!(payload["authentication_outcome"], "authenticated");
        assert_eq!(payload["correlation_id"], "corr-audit-deny-append-fail-001");
    }

    #[tokio::test]
    async fn terminal_paths_append_redacted_records_with_nullable_approval_reference() {
        let audit_appender = Arc::new(CapturingAuditAppender::default());
        let app = test_app_with_audit_appender(audit_appender.clone());

        let allow_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-terminal-allow-001")
                    .body(Body::empty())
                    .expect("allow request should build"),
            )
            .await
            .expect("allow request should complete");
        assert_eq!(allow_response.status(), StatusCode::ACCEPTED);

        let deny_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("analytics-reader", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-audit-terminal-deny-001")
                    .body(Body::empty())
                    .expect("deny request should build"),
            )
            .await
            .expect("deny request should complete");
        assert_eq!(deny_response.status(), StatusCode::FORBIDDEN);

        let auth_failure_response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance")
                    .method("POST")
                    .header("x-correlation-id", "corr-audit-terminal-auth-deny-001")
                    .body(Body::empty())
                    .expect("auth-deny request should build"),
            )
            .await
            .expect("auth-deny request should complete");
        assert_eq!(auth_failure_response.status(), StatusCode::UNAUTHORIZED);

        let records = audit_appender.snapshot();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].outcome, PrivilegedAuditOutcome::Allow);
        assert_eq!(
            records[1].outcome,
            PrivilegedAuditOutcome::AuthorizationDenied
        );
        assert_eq!(
            records[2].outcome,
            PrivilegedAuditOutcome::AuthenticationDenied
        );
        assert_eq!(records[2].actor_id, "unauthenticated");
        assert_eq!(records[2].role, "unknown_role");
        assert_eq!(
            records[2].correlation_id,
            "corr-audit-terminal-auth-deny-001"
        );

        for record in records {
            assert_eq!(record.action_type, "execute_control_plane_action");
            assert_eq!(record.approval_reference, None);

            let parameters = record
                .parameters
                .as_object()
                .expect("audit parameters should always be a JSON object");
            assert!(parameters.contains_key("endpoint"));
            assert!(parameters.contains_key("http_method"));
            assert!(
                !parameters.keys().any(|key| key.contains("token")
                    || key.contains("secret")
                    || key == "authorization"),
                "audit parameters must not include plaintext secret-like keys"
            );
            assert!(
                !record.parameters.to_string().contains("Bearer "),
                "audit parameters must never contain bearer tokens"
            );
        }
    }

    #[tokio::test]
    async fn critical_action_request_submission_returns_pending_machine_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/critical-actions/risk_limit_increase/requests/req-critical-001")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-submit-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"expires_at_utc":"2099-01-01T00:00:00Z"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["action"], "risk_limit_increase");
        assert_eq!(payload["request_id"], "req-critical-001");
        assert_eq!(payload["outcome"], "pending");
        assert_eq!(payload["reason_code"], "approval_pending");
    }

    #[tokio::test]
    async fn critical_action_execute_denies_missing_second_approver_with_security_signal() {
        let app = test_app();
        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/critical-actions/risk_limit_increase/requests/req-critical-002")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-submit-002")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"expires_at_utc":"2099-01-01T00:00:00Z"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(submit_response.status(), StatusCode::ACCEPTED);

        let execute_response = app
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-002/execute",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-execute-002")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(execute_response.status(), StatusCode::FORBIDDEN);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(execute_response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(payload["reason_code"], "approval_missing_second_approver");
        assert_eq!(
            payload["security_signal"]["name"],
            "unauthorized_privileged_approval_attempt_v1"
        );
        assert_eq!(payload["security_signal"]["alert_compatible"], true);
        assert_eq!(payload["security_signal"]["alert_target_seconds"], 30);
    }

    #[tokio::test]
    async fn critical_action_vote_denies_self_approval_attempt() {
        let app = test_app();
        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/critical-actions/risk_limit_increase/requests/req-critical-003")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-submit-003")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"expires_at_utc":"2099-01-01T00:00:00Z"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(submit_response.status(), StatusCode::ACCEPTED);

        let vote_response = app
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-003/votes",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-vote-003")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"decision":"approve"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(vote_response.status(), StatusCode::FORBIDDEN);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(vote_response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(payload["reason_code"], "approval_self_approval_attempt");
    }

    #[tokio::test]
    async fn critical_action_rate_limit_boundary_denies_sixth_request() {
        let app = test_app();
        for index in 0..5 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!(
                            "/control/critical-actions/risk_limit_increase/requests/req-critical-rate-{index}"
                        ))
                        .method("POST")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", format!("corr-critical-rate-{index}"))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            r#"{"expires_at_utc":"2099-01-01T00:00:00Z"}"#,
                        ))
                        .expect("request should build"),
                )
                .await
                .expect("request should complete");
            assert_eq!(response.status(), StatusCode::ACCEPTED);
        }

        let sixth = app
            .oneshot(
                Request::builder()
                    .uri("/control/critical-actions/risk_limit_increase/requests/req-critical-rate-5")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-rate-5")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"expires_at_utc":"2099-01-01T00:00:00Z"}"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(sixth.status(), StatusCode::FORBIDDEN);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(sixth.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(payload["reason_code"], "approval_rate_limited_actor");
    }

    #[tokio::test]
    async fn critical_action_submit_denies_expired_window_with_machine_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-expired-001",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-submit-expired-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"expires_at_utc":"2000-01-01T00:00:00Z"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(payload["state"], "expired");
        assert_eq!(payload["reason_code"], "approval_expired_window");
        assert_eq!(payload["error_code"], "approval_expired_window");
        assert_eq!(
            payload["message"],
            "approval request has expired and cannot be used"
        );
        assert_eq!(payload["security_signal"]["alert_compatible"], true);
    }

    #[tokio::test]
    async fn critical_action_vote_unknown_request_returns_bad_request_machine_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-missing-001/votes",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("admin-1", "administrative_actions", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-vote-missing-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"decision":"approve"}"#))
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
        assert_eq!(payload["status"], "denied");
        assert_eq!(payload["reason_code"], "approval_unknown_request");
        assert_eq!(payload["error_code"], "approval_unknown_request");
        assert_eq!(payload["message"], "approval request was not found");
        assert_eq!(
            payload["security_signal"]["name"],
            "unauthorized_privileged_approval_attempt_v1"
        );
    }

    #[tokio::test]
    async fn critical_action_execute_without_request_returns_bad_request_machine_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-missing-002/execute",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-execute-missing-002")
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
        assert_eq!(payload["status"], "denied");
        assert_eq!(payload["reason_code"], "approval_missing_request");
        assert_eq!(payload["error_code"], "approval_missing_request");
        assert_eq!(
            payload["message"],
            "approval request is required before execution"
        );
        assert_eq!(
            payload["security_signal"]["name"],
            "unauthorized_privileged_approval_attempt_v1"
        );
    }

    #[tokio::test]
    async fn approved_critical_execution_propagates_non_null_approval_reference_to_audit() {
        let audit_appender = Arc::new(CapturingAuditAppender::default());
        let app = test_app_with_audit_appender(audit_appender.clone());

        let submit = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/critical-actions/risk_limit_increase/requests/req-critical-004")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-submit-004")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"expires_at_utc":"2099-01-01T00:00:00Z"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(submit.status(), StatusCode::ACCEPTED);

        let vote = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-004/votes",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("admin-1", "administrative_actions", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-vote-004")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"decision":"approve"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(vote.status(), StatusCode::ACCEPTED);

        let execute = app
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/critical-actions/risk_limit_increase/requests/req-critical-004/execute",
                    )
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-critical-execute-004")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(execute.status(), StatusCode::ACCEPTED);

        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(execute.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        let approval_reference = payload["approval_reference"]
            .as_str()
            .expect("approved response should include approval reference")
            .to_string();
        assert!(!approval_reference.is_empty());

        let records = audit_appender.snapshot();
        let linked = records
            .iter()
            .filter(|record| record.action_type == "risk_limit_increase")
            .any(|record| {
                record.approval_reference.as_deref() == Some(approval_reference.as_str())
            });
        assert!(
            linked,
            "at least one immutable audit record should carry approval_reference for approved critical action"
        );

        let denied_records_with_reference = records
            .iter()
            .filter(|record| record.outcome == PrivilegedAuditOutcome::AuthorizationDenied)
            .filter(|record| record.action_type == "risk_limit_increase")
            .any(|record| record.approval_reference.is_some());
        assert!(
            !denied_records_with_reference,
            "denied critical approvals must not synthesize approval_reference values"
        );
    }

    #[tokio::test]
    async fn scheduled_rotation_route_returns_allow_with_machine_evidence() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-scheduled-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"2025-12-01T00:00:00Z",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["outcome"], "allow");
        assert_eq!(payload["state"], "succeeded");
        assert_eq!(payload["reason_code"], "credential_rotation_allowed");
        assert_eq!(payload["trigger_type"], "scheduled_cadence");
        assert!(payload["rotation_reference"].is_string());
    }

    #[tokio::test]
    async fn scheduled_rotation_route_denies_not_due_boundary_with_machine_code() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-scheduled-002")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"2026-01-07T00:00:01Z",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(
            payload["reason_code"],
            "credential_rotation_scheduled_not_due"
        );
        assert_eq!(
            payload["error_code"],
            "credential_rotation_scheduled_not_due"
        );
    }

    #[tokio::test]
    async fn emergency_rotation_route_denies_expired_window() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/emergency")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-emergency-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "compromise_triggered_at_utc":"2026-04-05T00:00:00Z",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(
            payload["reason_code"],
            "credential_rotation_emergency_window_expired"
        );
    }

    #[tokio::test]
    async fn emergency_rotation_route_returns_allow_with_machine_evidence() {
        let compromise_triggered_at_utc = (OffsetDateTime::now_utc() - time::Duration::minutes(10))
            .format(&Rfc3339)
            .expect("RFC3339 formatting should succeed");
        let request_body = format!(
            r#"{{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "compromise_triggered_at_utc":"{compromise_triggered_at_utc}",
                            "metadata":{{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }}
                        }}"#
        );
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/emergency")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-emergency-allow-001")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["outcome"], "allow");
        assert_eq!(payload["state"], "succeeded");
        assert_eq!(payload["reason_code"], "credential_rotation_allowed");
        assert_eq!(payload["trigger_type"], "emergency_compromise");
        assert!(payload["rotation_reference"].is_string());
    }

    #[tokio::test]
    async fn emergency_rotation_route_returns_machine_error_for_invalid_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/emergency")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-emergency-invalid-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "compromise_triggered_at_utc":"not-a-timestamp",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }
                        }"#,
                    ))
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
        assert_eq!(payload["error_code"], "credential_rotation_invalid_payload");
        assert_eq!(payload["action"], "credential_rotation_workflow");
    }

    #[tokio::test]
    async fn emergency_rotation_route_returns_machine_error_for_missing_metadata() {
        let compromise_triggered_at_utc = (OffsetDateTime::now_utc() - time::Duration::minutes(10))
            .format(&Rfc3339)
            .expect("RFC3339 formatting should succeed");
        let request_body = format!(
            r#"{{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "compromise_triggered_at_utc":"{compromise_triggered_at_utc}"
                        }}"#
        );
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/emergency")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-emergency-metadata-001")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
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
        assert_eq!(
            payload["error_code"],
            "credential_rotation_missing_metadata"
        );
        assert_eq!(payload["action"], "credential_rotation_workflow");
    }

    #[tokio::test]
    async fn emergency_rotation_route_reuses_authorization_guard_for_unauthorized_role() {
        let compromise_triggered_at_utc = (OffsetDateTime::now_utc() - time::Duration::minutes(10))
            .format(&Rfc3339)
            .expect("RFC3339 formatting should succeed");
        let request_body = format!(
            r#"{{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "compromise_triggered_at_utc":"{compromise_triggered_at_utc}",
                            "metadata":{{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }}
                        }}"#
        );
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/emergency")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-emergency-authz-001")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["error_code"], "authorization_denied");
        assert_eq!(payload["action"], "execute_control_plane_action");
    }

    #[tokio::test]
    async fn emergency_rotation_route_surfaces_provider_failure_machine_reason() {
        let compromise_triggered_at_utc = (OffsetDateTime::now_utc() - time::Duration::minutes(10))
            .format(&Rfc3339)
            .expect("RFC3339 formatting should succeed");
        let request_body = format!(
            r#"{{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "compromise_triggered_at_utc":"{compromise_triggered_at_utc}",
                            "metadata":{{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod",
                                "force_provider_failure":true
                            }}
                        }}"#
        );
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/emergency")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-emergency-provider-001")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(
            payload["reason_code"],
            "credential_rotation_provider_unavailable"
        );
    }

    #[tokio::test]
    async fn scheduled_rotation_route_returns_machine_error_for_invalid_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-invalid-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"not-a-timestamp",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }
                        }"#,
                    ))
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
        assert_eq!(payload["error_code"], "credential_rotation_invalid_payload");
        assert_eq!(payload["action"], "credential_rotation_workflow");
    }

    #[tokio::test]
    async fn scheduled_rotation_route_returns_machine_error_for_missing_metadata() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-metadata-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"2025-12-01T00:00:00Z"
                        }"#,
                    ))
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
        assert_eq!(
            payload["error_code"],
            "credential_rotation_missing_metadata"
        );
        assert_eq!(payload["action"], "credential_rotation_workflow");
    }

    #[tokio::test]
    async fn scheduled_rotation_route_reuses_authorization_guard_for_unauthorized_role() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-authz-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"2025-12-01T00:00:00Z",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod"
                            }
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["error_code"], "authorization_denied");
        assert_eq!(payload["action"], "execute_control_plane_action");
    }

    #[tokio::test]
    async fn scheduled_rotation_route_surfaces_provider_failure_machine_reason() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-provider-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"2025-12-01T00:00:00Z",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod",
                                "force_provider_failure":true
                            }
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(
            payload["reason_code"],
            "credential_rotation_provider_unavailable"
        );
    }

    #[tokio::test]
    async fn scheduled_rotation_route_surfaces_runtime_failure_machine_reason() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/credentials/rotation/scheduled")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rotation-runtime-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "credential_scope":"control_api",
                            "credential_reference":"vault://control-api/prod",
                            "last_rotated_at_utc":"2025-12-01T00:00:00Z",
                            "metadata":{
                                "crypto_posture_verified":true,
                                "runtime_injection_mode":"runtime_only",
                                "provider_ref":"vault://control-api/prod",
                                "force_runtime_failure":true
                            }
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "denied");
        assert_eq!(
            payload["reason_code"],
            "credential_rotation_runtime_unavailable"
        );
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
            Arc::new(CapturingAuditAppender::default()),
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
            Arc::new(CapturingAuditAppender::default()),
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
