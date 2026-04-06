use crate::middleware::{
    AuthenticatedActor, ControlApiState, audit_append_failure_status, require_authenticated_actor,
};
use axum::{
    Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    middleware as axum_middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::allocation::{
    DEFAULT_EXPOSURE_DRIFT_THRESHOLD_PCT, DEFAULT_POLICY_STALE_AFTER_SECONDS,
    DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT, RebalanceReasonCode,
};
use domain::attribution::{
    AttributionPeriod, AttributionReasonCode, AttributionRow, AttributionValidationIssue,
    build_cost_aware_attribution_rows, build_query_scope,
};
use domain::governance::{
    ApprovalDecisionEvidence, ApprovalDecisionOutcome, ApprovalReasonCode, AuthorizationDecision,
    AuthorizationOutcome, AuthorizationReason, ControlAction, CredentialRotationDecisionOutcome,
    CredentialRotationEvidence, CredentialRotationReasonCode, PrivilegedAuditOutcome,
    PrivilegedAuditRecord,
};
use domain::incidents::{
    IncidentQueryFilters, IncidentReasonCode, IncidentTimelineEvent, IncidentTimelineStage,
    IncidentValidationIssue, apply_incident_query, build_incident_query_filters,
};
use domain::risk::{
    EmergencyControlAction, EmergencyControlReasonCode, MarketPolicyReasonCode,
    MarketPolicyValidationIssue, RiskLimitReasonCode, RiskLimitScope, RiskLimitValidationIssue,
    SafetyControlActionRecord,
};
use governance_service::allocation_policy::{
    AllocationPolicyMutationEvidence, EvaluateRebalanceDriftInput,
    ExecuteRebalanceRecommendationInput, PendingRebalanceRecommendationsInput,
    RebalanceRecommendationEvidence, UpsertAllocationPolicyInput,
};
use governance_service::approvals::{
    EvaluateApprovalExecutionInput, RecordApprovalVoteInput, SubmitApprovalRequestInput,
};
use governance_service::audit::AuditAppendError;
use governance_service::credentials::{
    TriggerEmergencyRotationInput, TriggerScheduledRotationInput,
};
use governance_service::market_policy::{ToggleMarketClusterInput, UpsertMarketPolicyProfileInput};
use governance_service::risk_limits::{
    PendingRiskLimitProfilesInput, RiskLimitProfileMutationEvidence, RiskLimitRuleInput,
    UpsertRiskLimitProfileInput,
};
use governance_service::safety_controls::ExecuteManualSafetyControlInput;
use persistence::postgres::attribution_snapshots::load_latest_attribution_snapshots;
use persistence::postgres::incident_query_views::load_incident_forensics_timeline;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

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
    let market_policy_routes = Router::new()
        .route(
            "/control/market-policy/profiles/{cluster_id}",
            post(update_market_policy_profile),
        )
        .route(
            "/control/market-policy/clusters/{cluster_id}/toggle",
            post(toggle_market_policy_cluster),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let risk_limit_routes = Router::new()
        .route(
            "/control/risk-limits/profiles/{profile_key}",
            post(upsert_risk_limit_profile),
        )
        .route(
            "/control/risk-limits/pending",
            get(list_pending_risk_limit_profiles),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let allocation_policy_routes = Router::new()
        .route(
            "/control/allocation-policies/{policy_key}",
            post(upsert_allocation_policy),
        )
        .route(
            "/control/rebalance/recommendations/pending",
            get(list_pending_rebalance_recommendations),
        )
        .route(
            "/control/rebalance/recommendations/{recommendation_id}/execute",
            post(execute_rebalance_recommendation),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let attribution_routes = Router::new()
        .route("/control/portfolio/attribution", get(read_portfolio_attribution))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let incident_forensics_routes = Router::new()
        .route("/control/incidents/forensics", get(read_incident_forensics))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let emergency_control_routes = Router::new()
        .route("/control/emergency/pause", post(trigger_emergency_pause))
        .route(
            "/control/emergency/reduce-only",
            post(trigger_emergency_reduce_only),
        )
        .route(
            "/control/emergency/cancel-all",
            post(trigger_emergency_cancel_all),
        )
        .route(
            "/control/emergency/actions/{action_id}",
            get(get_emergency_control_action_result),
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
        .merge(market_policy_routes)
        .merge(risk_limit_routes)
        .merge(allocation_policy_routes)
        .merge(attribution_routes)
        .merge(incident_forensics_routes)
        .merge(emergency_control_routes)
        .with_state(state)
}

pub async fn health() -> &'static str {
    "ok"
}

pub async fn rebalance_portfolio(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    maybe_payload: Option<axum::Json<RebalanceDriftPayload>>,
) -> Response {
    let endpoint = "/control/rebalance".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let Some(axum::Json(payload)) = maybe_payload else {
        return (
            StatusCode::ACCEPTED,
            axum::Json(ControlActionAccepted {
                status: "accepted",
                action: authorization.action,
                actor_id: authorization.actor_id,
                role: authorization.role,
                outcome: "allow",
                authentication_outcome: actor.authentication_outcome.as_str(),
                correlation_id: authorization.correlation_id,
                timestamp_utc: authorization.timestamp_utc,
            }),
        )
            .into_response();
    };

    let observed_at_utc = payload
        .observed_at_utc
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let approval_request_id = payload
        .approval_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let provided_approval_reference = payload
        .approval_reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if provided_approval_reference.is_some() && approval_request_id.is_none() {
        return approval_reference_requires_request_id_response(
            "rebalance_recommendation_evaluate",
            &actor,
            observed_at_utc.clone(),
            endpoint,
        );
    }

    let mut approval_reference = None;
    if payload.require_execution && let Some(request_id) = approval_request_id {
        let approval_decision =
            match state
                .approval_orchestrator
                .evaluate_execution(EvaluateApprovalExecutionInput {
                    request_id,
                    action_id: "rebalance_recommendation_execute".to_string(),
                    actor_id: actor.actor_id.clone(),
                    actor_role: actor.role.clone(),
                    correlation_id: actor.correlation_id.clone(),
                    now_utc: observed_at_utc.clone(),
                }) {
                Ok(decision) => decision,
                Err(error) => {
                    return approval_service_error_response(
                        error.code,
                        error.message,
                        &actor,
                        observed_at_utc,
                        endpoint,
                    );
                }
            };
        match approval_decision.outcome {
            ApprovalDecisionOutcome::Allow => {
                approval_reference = approval_decision
                    .approval_reference
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
            }
            ApprovalDecisionOutcome::Pending => {}
            ApprovalDecisionOutcome::Deny => {
                return critical_approval_response(
                    &state,
                    &actor,
                    approval_decision,
                    endpoint,
                    "rebalance_recommendation_evaluate",
                );
            }
        }
    }

    let recommendation = match state
        .allocation_policy_orchestrator
        .evaluate_rebalance_drift(EvaluateRebalanceDriftInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            policy_key: payload.policy_key,
            exposure_drift_pct: payload.exposure_drift_pct,
            relative_alpha_drift_pct: payload.relative_alpha_drift_pct,
            observed_at_utc: observed_at_utc.clone(),
            stale_after_seconds: payload.stale_after_seconds,
            correlation_id: actor.correlation_id.clone(),
            require_execution: payload.require_execution,
            approval_reference,
        }) {
        Ok(evidence) => evidence,
        Err(error) => {
            return allocation_policy_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "rebalance_recommendation_evaluate",
                &actor,
                observed_at_utc,
                endpoint,
            );
        }
    };

    rebalance_recommendation_response(
        &state,
        &actor,
        recommendation,
        endpoint,
        "POST",
        "rebalance_recommendation_evaluate",
    )
}

pub async fn submit_critical_action_request(
    State(state): State<ControlApiState>,
    Path((action_id, request_id)): Path<(String, String)>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<CriticalActionRequestPayload>,
) -> Response {
    let endpoint = format!("/control/critical-actions/{action_id}/requests/{request_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
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
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
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
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
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
    let authorization = match authorize_critical_action(&state, &actor, endpoint, "POST") {
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
    let authorization = match authorize_critical_action(&state, &actor, endpoint, "POST") {
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

pub async fn update_market_policy_profile(
    State(state): State<ControlApiState>,
    Path(cluster_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<MarketPolicyProfilePayload>,
) -> Response {
    let endpoint = format!("/control/market-policy/profiles/{cluster_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let decision = match state
        .market_policy_orchestrator
        .upsert_market_policy_profile(UpsertMarketPolicyProfileInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            cluster_id,
            min_liquidity_usd: payload.min_liquidity_usd,
            max_spread_bps: payload.max_spread_bps,
            min_reward_score: payload.min_reward_score,
            max_exposure_pct_nav: payload.max_exposure_pct_nav,
            correlation_id: actor.correlation_id.clone(),
            updated_at_utc: authorization.timestamp_utc.clone(),
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return market_policy_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "market_policy_profile_update",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    market_policy_profile_response(&state, &actor, decision, endpoint)
}

pub async fn toggle_market_policy_cluster(
    State(state): State<ControlApiState>,
    Path(cluster_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<MarketClusterTogglePayload>,
) -> Response {
    let endpoint = format!("/control/market-policy/clusters/{cluster_id}/toggle");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let decision =
        match state
            .market_policy_orchestrator
            .toggle_market_cluster(ToggleMarketClusterInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                cluster_id,
                is_enabled: payload.is_enabled,
                reason_code: payload.reason_code,
                correlation_id: actor.correlation_id.clone(),
                updated_at_utc: authorization.timestamp_utc.clone(),
            }) {
            Ok(decision) => decision,
            Err(error) => {
                return market_policy_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "market_policy_cluster_toggle",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };

    market_policy_cluster_toggle_response(&state, &actor, decision, endpoint)
}

pub async fn upsert_risk_limit_profile(
    State(state): State<ControlApiState>,
    Path(profile_key): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<RiskLimitProfilePayload>,
) -> Response {
    let endpoint = format!("/control/risk-limits/profiles/{profile_key}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let mut approval_reference = None;
    if let Some(request_id) = payload.approval_request_id.clone() {
        let approval_decision =
            match state
                .approval_orchestrator
                .evaluate_execution(EvaluateApprovalExecutionInput {
                    request_id,
                    action_id: "risk_limit_increase".to_string(),
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

        match approval_decision.outcome {
            ApprovalDecisionOutcome::Allow => {
                approval_reference = approval_decision.approval_reference.clone();
            }
            ApprovalDecisionOutcome::Pending => {}
            ApprovalDecisionOutcome::Deny => {
                return critical_approval_response(
                    &state,
                    &actor,
                    approval_decision,
                    endpoint,
                    "risk_limit_profile_update",
                );
            }
        }
    }

    let mut inventory_rules = Vec::with_capacity(payload.inventory_rules.len());
    for rule in payload.inventory_rules {
        let scope = match RiskLimitScope::parse(&rule.scope) {
            Ok(scope) => scope,
            Err(error) => {
                return risk_limit_service_error_response(
                    error.code,
                    error.message,
                    vec![RiskLimitValidationIssue {
                        field: "inventory_rules.scope",
                        code: RiskLimitReasonCode::InvalidPayload.code(),
                        message: "inventory_rules.scope must be `market` or `strategy`".to_string(),
                    }],
                    "risk_limit_profile_update",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        inventory_rules.push(RiskLimitRuleInput {
            scope,
            scope_id: rule.scope_id,
            max_position_units: rule.max_position_units,
            max_order_size_units: rule.max_order_size_units,
            max_concentration_pct_nav: rule.max_concentration_pct_nav,
        });
    }

    let decision =
        match state
            .risk_limit_orchestrator
            .upsert_risk_limit_profile(UpsertRiskLimitProfileInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                profile_key,
                version: payload.version,
                portfolio_scope_id: payload.portfolio_scope_id,
                market_scope_id: payload.market_scope_id,
                strategy_scope_id: payload.strategy_scope_id,
                portfolio_max_notional_usd: payload.portfolio_max_notional_usd,
                market_max_notional_usd: payload.market_max_notional_usd,
                strategy_max_notional_usd: payload.strategy_max_notional_usd,
                portfolio_max_inventory_units: payload.portfolio_max_inventory_units,
                market_max_inventory_units: payload.market_max_inventory_units,
                strategy_max_inventory_units: payload.strategy_max_inventory_units,
                portfolio_max_concentration_pct_nav: payload.portfolio_max_concentration_pct_nav,
                market_max_concentration_pct_nav: payload.market_max_concentration_pct_nav,
                strategy_max_concentration_pct_nav: payload.strategy_max_concentration_pct_nav,
                inventory_rules,
                correlation_id: actor.correlation_id.clone(),
                updated_at_utc: authorization.timestamp_utc.clone(),
                approval_reference,
            }) {
            Ok(decision) => decision,
            Err(error) => {
                return risk_limit_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "risk_limit_profile_update",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };

    risk_limit_profile_response(&state, &actor, decision, endpoint)
}

pub async fn list_pending_risk_limit_profiles(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/risk-limits/pending".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let pending = match state
        .risk_limit_orchestrator
        .list_pending_risk_limit_profiles(PendingRiskLimitProfilesInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            queried_at_utc: authorization.timestamp_utc.clone(),
            profile_key: None,
        }) {
        Ok(pending) => pending,
        Err(error) => {
            return risk_limit_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "risk_limit_pending_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    risk_limit_pending_query_response(
        &state,
        &actor,
        pending,
        authorization.timestamp_utc,
        endpoint,
    )
}

pub async fn upsert_allocation_policy(
    State(state): State<ControlApiState>,
    Path(policy_key): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<AllocationPolicyPayload>,
) -> Response {
    let endpoint = format!("/control/allocation-policies/{policy_key}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let approval_request_id = payload
        .approval_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let provided_approval_reference = payload
        .approval_reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if provided_approval_reference.is_some() && approval_request_id.is_none() {
        return approval_reference_requires_request_id_response(
            "allocation_policy_update",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    }

    let mut approval_reference = None;
    if let Some(request_id) = approval_request_id {
        let approval_decision =
            match state
                .approval_orchestrator
                .evaluate_execution(EvaluateApprovalExecutionInput {
                    request_id,
                    action_id: "allocation_policy_increase".to_string(),
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

        match approval_decision.outcome {
            ApprovalDecisionOutcome::Allow => {
                approval_reference = approval_decision
                    .approval_reference
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
            }
            ApprovalDecisionOutcome::Pending => {}
            ApprovalDecisionOutcome::Deny => {
                return critical_approval_response(
                    &state,
                    &actor,
                    approval_decision,
                    endpoint,
                    "allocation_policy_update",
                );
            }
        }
    }

    let decision = match state
        .allocation_policy_orchestrator
        .upsert_allocation_policy(UpsertAllocationPolicyInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            policy_key,
            version: payload.version,
            portfolio_scope_id: payload.portfolio_scope_id,
            target_exposure_pct_nav: payload.target_exposure_pct_nav,
            target_relative_alpha_weight: payload.target_relative_alpha_weight,
            exposure_drift_threshold_pct: payload.exposure_drift_threshold_pct,
            relative_alpha_drift_threshold_pct: payload.relative_alpha_drift_threshold_pct,
            advanced_parameters: payload.advanced_parameters,
            correlation_id: actor.correlation_id.clone(),
            updated_at_utc: authorization.timestamp_utc.clone(),
            approval_reference,
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return allocation_policy_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "allocation_policy_update",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    allocation_policy_mutation_response(&state, &actor, decision, endpoint)
}

pub async fn list_pending_rebalance_recommendations(
    State(state): State<ControlApiState>,
    Query(query): Query<PendingRebalanceRecommendationsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/rebalance/recommendations/pending".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let pending = match state
        .allocation_policy_orchestrator
        .list_pending_rebalance_recommendations(PendingRebalanceRecommendationsInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            queried_at_utc: authorization.timestamp_utc.clone(),
            policy_key: query.policy_key,
        }) {
        Ok(recommendations) => recommendations,
        Err(error) => {
            return allocation_policy_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "rebalance_pending_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    rebalance_pending_query_response(
        &state,
        &actor,
        pending,
        authorization.timestamp_utc,
        endpoint,
    )
}

pub async fn read_portfolio_attribution(
    State(state): State<ControlApiState>,
    Query(query): Query<AttributionQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/portfolio/attribution".to_string();
    let authorization = match authorize_attribution_read(&state, &actor, &endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let period_raw = query.period.as_deref().unwrap_or("24h");
    let period = match AttributionPeriod::parse(period_raw) {
        Ok(period) => period,
        Err(error) => {
            return attribution_service_error_response(
                error.code,
                error.message,
                if error.field_errors.is_empty() {
                    vec![AttributionValidationIssue {
                        field: "period",
                        code: AttributionReasonCode::InvalidPayload.code(),
                        message: "period must be one of: 1h, 24h, 30d".to_string(),
                    }]
                } else {
                    error.field_errors
                },
                "attribution_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let as_of_utc = query
        .as_of_utc
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let dependency_state = match parse_attribution_dependency_state(query.dependency_state.as_deref())
    {
        Ok(state) => state,
        Err(error) => {
            return attribution_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "attribution_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    match dependency_state {
        AttributionDependencyState::ProjectionUnavailable => {
            return attribution_service_error_response(
                AttributionReasonCode::ProjectionUnavailable.code(),
                "attribution projection dependency is unavailable".to_string(),
                Vec::new(),
                "attribution_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        AttributionDependencyState::StaleSource => {
            return attribution_service_error_response(
                AttributionReasonCode::StaleSource.code(),
                "reconciliation evidence is stale for attribution query".to_string(),
                Vec::new(),
                "attribution_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        AttributionDependencyState::ReconciliationUnavailable => {
            return attribution_service_error_response(
                AttributionReasonCode::PersistenceUnavailable.code(),
                "reconciliation evidence is unavailable for attribution query".to_string(),
                Vec::new(),
                "attribution_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        AttributionDependencyState::Healthy => {}
    }

    let scope = match build_query_scope(
        period,
        &as_of_utc,
        query.market_id.as_deref(),
        query.alpha_id.as_deref(),
    ) {
        Ok(scope) => scope,
        Err(error) => {
            return attribution_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "attribution_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let rows = if let Some(pool) = state.attribution_pool.as_ref() {
        match load_latest_attribution_snapshots(
            pool,
            period,
            query.market_id.as_deref(),
            query.alpha_id.as_deref(),
            200,
        )
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                return attribution_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "attribution_query",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
    } else {
        let observations =
            match synthetic_attribution_observations(&scope.as_of_utc, &actor.correlation_id) {
                Ok(observations) => observations,
                Err(error) => {
                    return attribution_service_error_response(
                        error.code,
                        error.message,
                        error.field_errors,
                        "attribution_query",
                        &actor,
                        authorization.timestamp_utc.clone(),
                        endpoint,
                    );
                }
            };
        match build_cost_aware_attribution_rows(&observations, &scope) {
            Ok(rows) => rows,
            Err(error) => {
                return attribution_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "attribution_query",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
    };

    attribution_query_response(
        &state,
        &actor,
        rows,
        AttributionQueryWindow {
            period,
            start_inclusive_utc: scope.start_inclusive_utc,
            end_exclusive_utc: scope.end_exclusive_utc,
            as_of_utc: scope.as_of_utc,
        },
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

pub async fn read_incident_forensics(
    State(state): State<ControlApiState>,
    Query(query): Query<IncidentForensicsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/incidents/forensics".to_string();
    let authorization = match authorize_incident_forensics_read(&state, &actor, &endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let started_at = Instant::now();

    let filters = match build_incident_query_filters(
        query.market_id.as_deref(),
        query.order_id.as_deref(),
        query.alpha_id.as_deref(),
        query.actor_id.as_deref(),
        query.start_ts.as_deref(),
        query.end_ts.as_deref(),
        &authorization.timestamp_utc,
    ) {
        Ok(filters) => filters,
        Err(error) => {
            return incident_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let dependency_state = match parse_incident_dependency_state(query.dependency_state.as_deref()) {
        Ok(state) => state,
        Err(error) => {
            return incident_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };
    match dependency_state {
        IncidentDependencyState::DependencyUnavailable => {
            return incident_service_error_response(
                IncidentReasonCode::DependencyUnavailable.code(),
                "incident forensics dependencies are unavailable".to_string(),
                Vec::new(),
                "incident_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::StaleEvidence => {
            return incident_service_error_response(
                IncidentReasonCode::StaleEvidence.code(),
                "incident evidence is stale for forensics query".to_string(),
                Vec::new(),
                "incident_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::Healthy => {}
    }

    let events = if let Some(pool) = state.attribution_pool.as_ref() {
        match load_incident_forensics_timeline(pool, &filters).await {
            Ok(events) => events,
            Err(error) => {
                return incident_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "incident_query",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
    } else {
        let synthetic = synthetic_incident_timeline(&authorization.timestamp_utc, &actor.correlation_id);
        match apply_incident_query(&synthetic, &filters) {
            Ok(events) => events,
            Err(error) => {
                return incident_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "incident_query",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
    };

    let query_latency_ms = i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX);

    incident_forensics_query_response(
        &state,
        &actor,
        filters,
        events,
        query_latency_ms,
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

pub async fn execute_rebalance_recommendation(
    State(state): State<ControlApiState>,
    Path(recommendation_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<RebalanceExecutionPayload>,
) -> Response {
    let endpoint = format!("/control/rebalance/recommendations/{recommendation_id}/execute");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let executed_at_utc = payload
        .executed_at_utc
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let approval_request_id = payload
        .approval_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let provided_approval_reference = payload
        .approval_reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if provided_approval_reference.is_some() && approval_request_id.is_none() {
        return approval_reference_requires_request_id_response(
            "rebalance_recommendation_execute",
            &actor,
            executed_at_utc,
            endpoint,
        );
    }

    let mut approval_reference = None;
    if let Some(request_id) = approval_request_id {
        let approval_decision =
            match state
                .approval_orchestrator
                .evaluate_execution(EvaluateApprovalExecutionInput {
                    request_id,
                    action_id: "rebalance_recommendation_execute".to_string(),
                    actor_id: actor.actor_id.clone(),
                    actor_role: actor.role.clone(),
                    correlation_id: actor.correlation_id.clone(),
                    now_utc: executed_at_utc.clone(),
                }) {
                Ok(decision) => decision,
                Err(error) => {
                    return approval_service_error_response(
                        error.code,
                        error.message,
                        &actor,
                        executed_at_utc,
                        endpoint,
                    );
                }
            };

        match approval_decision.outcome {
            ApprovalDecisionOutcome::Allow => {
                approval_reference = approval_decision
                    .approval_reference
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
            }
            ApprovalDecisionOutcome::Pending | ApprovalDecisionOutcome::Deny => {
                return critical_approval_response(
                    &state,
                    &actor,
                    approval_decision,
                    endpoint,
                    "rebalance_recommendation_execute",
                );
            }
        }
    }

    let decision = match state
        .allocation_policy_orchestrator
        .execute_rebalance_recommendation(ExecuteRebalanceRecommendationInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            recommendation_id,
            correlation_id: actor.correlation_id.clone(),
            executed_at_utc: executed_at_utc.clone(),
            approval_reference,
        }) {
        Ok(decision) => decision,
        Err(error) => {
            return allocation_policy_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "rebalance_recommendation_execute",
                &actor,
                executed_at_utc,
                endpoint,
            );
        }
    };

    rebalance_recommendation_response(
        &state,
        &actor,
        decision,
        endpoint,
        "POST",
        "rebalance_recommendation_execute",
    )
}

pub async fn trigger_emergency_pause(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<EmergencyControlPayload>,
) -> Response {
    trigger_manual_emergency_control(
        &state,
        &actor,
        payload,
        EmergencyControlAction::Pause,
        "/control/emergency/pause".to_string(),
        "emergency_control_pause",
    )
}

pub async fn trigger_emergency_reduce_only(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<EmergencyControlPayload>,
) -> Response {
    trigger_manual_emergency_control(
        &state,
        &actor,
        payload,
        EmergencyControlAction::ReduceOnly,
        "/control/emergency/reduce-only".to_string(),
        "emergency_control_reduce_only",
    )
}

pub async fn trigger_emergency_cancel_all(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<EmergencyControlPayload>,
) -> Response {
    trigger_manual_emergency_control(
        &state,
        &actor,
        payload,
        EmergencyControlAction::CancelAll,
        "/control/emergency/cancel-all".to_string(),
        "emergency_control_cancel_all",
    )
}

pub async fn get_emergency_control_action_result(
    State(state): State<ControlApiState>,
    Path(action_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/emergency/actions/{action_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let result = match state
        .safety_control_orchestrator
        .get_action_result(&action_id)
    {
        Ok(record) => record,
        Err(error) => {
            return emergency_control_service_error_response(
                error.code,
                error.message,
                "emergency_control_action_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    emergency_control_action_response(
        &state,
        &actor,
        result,
        endpoint,
        "emergency_control_action_query",
        "GET",
        StatusCode::OK,
    )
}

fn trigger_manual_emergency_control(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    payload: EmergencyControlPayload,
    action: EmergencyControlAction,
    endpoint: String,
    action_type: &'static str,
) -> Response {
    let authorization = match authorize_critical_action(state, actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let result = match state.safety_control_orchestrator.execute_manual_control(
        ExecuteManualSafetyControlInput {
            action,
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            requested_at_utc: authorization.timestamp_utc.clone(),
            audit_reference: payload.audit_reference,
        },
    ) {
        Ok(record) => record,
        Err(error) => {
            return emergency_control_service_error_response(
                error.code,
                error.message,
                action_type,
                actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    emergency_control_action_response(
        state,
        actor,
        result,
        endpoint,
        action_type,
        "POST",
        StatusCode::ACCEPTED,
    )
}

fn authorize_critical_action(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    endpoint: &str,
    http_method: &'static str,
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
            "http_method": http_method,
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

fn authorize_attribution_read(
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
            "http_method": "GET",
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

    Err(Box::new(attribution_service_error_response(
        AttributionReasonCode::Unauthorized.code(),
        machine_error.message,
        Vec::new(),
        "attribution_query",
        actor,
        decision.timestamp_utc,
        endpoint.to_string(),
    )))
}

fn authorize_incident_forensics_read(
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
            "http_method": "GET",
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
    Err(Box::new(incident_service_error_response(
        IncidentReasonCode::Unauthorized.code(),
        machine_error.message,
        Vec::new(),
        "incident_query",
        actor,
        decision.timestamp_utc,
        endpoint.to_string(),
    )))
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

fn market_policy_profile_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: governance_service::market_policy::MarketPolicyProfileEvidence,
    endpoint: String,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "market_policy_profile_update".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "POST",
            "cluster_id": decision.cluster_id,
            "min_liquidity_usd": decision.min_liquidity_usd,
            "max_spread_bps": decision.max_spread_bps,
            "min_reward_score": decision.min_reward_score,
            "max_exposure_pct_nav": decision.max_exposure_pct_nav,
        }),
        approval_reference: None,
        timestamp: decision.updated_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: decision.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: decision.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "market_policy_profile_update".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.updated_at_utc,
        );
    }

    (
        StatusCode::ACCEPTED,
        axum::Json(MarketPolicyProfileDecisionResponse {
            status: "accepted",
            error_code: None,
            message: None,
            cluster_id: decision.cluster_id,
            min_liquidity_usd: decision.min_liquidity_usd,
            max_spread_bps: decision.max_spread_bps,
            min_reward_score: decision.min_reward_score,
            max_exposure_pct_nav: decision.max_exposure_pct_nav,
            actor_id: decision.actor_id,
            role: actor.role.clone(),
            reason_code: decision.reason_code,
            correlation_id: decision.correlation_id,
            timestamp_utc: decision.updated_at_utc,
            security_signal: None,
        }),
    )
        .into_response()
}

fn market_policy_cluster_toggle_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: governance_service::market_policy::MarketClusterToggleEvidence,
    endpoint: String,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "market_policy_cluster_toggle".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "POST",
            "cluster_id": decision.cluster_id,
            "is_enabled": decision.is_enabled,
        }),
        approval_reference: None,
        timestamp: decision.updated_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: decision.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: decision.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "market_policy_cluster_toggle".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.updated_at_utc,
        );
    }

    (
        StatusCode::ACCEPTED,
        axum::Json(MarketPolicyClusterToggleDecisionResponse {
            status: "accepted",
            error_code: None,
            message: None,
            cluster_id: decision.cluster_id,
            is_enabled: decision.is_enabled,
            actor_id: decision.actor_id,
            role: actor.role.clone(),
            reason_code: decision.reason_code,
            correlation_id: decision.correlation_id,
            timestamp_utc: decision.updated_at_utc,
            security_signal: None,
        }),
    )
        .into_response()
}

fn market_policy_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<MarketPolicyValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let security_signal = if error_code == "market_policy_unauthorized_role" {
        Some(MarketPolicySecuritySignal {
            name: "unauthorized_market_policy_mutation_attempt_v1",
            severity: "high",
            alert_compatible: true,
            alert_target_seconds: 30,
        })
    } else {
        None
    };

    (
        market_policy_service_error_status(error_code),
        axum::Json(MarketPolicyServiceErrorResponse {
            error_code,
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| MarketPolicyFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
            security_signal,
        }),
    )
        .into_response()
}

fn market_policy_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == MarketPolicyReasonCode::InvalidPayload.code()
            || code == MarketPolicyReasonCode::InvalidClusterId.code()
            || code == MarketPolicyReasonCode::InvalidThreshold.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        "market_policy_unauthorized_role" => StatusCode::FORBIDDEN,
        "market_policy_constraint_violation" => StatusCode::CONFLICT,
        code if code == MarketPolicyReasonCode::PersistenceUnavailable.code()
            || code == "market_policy_query_failed"
            || code == "market_policy_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn risk_limit_profile_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: RiskLimitProfileMutationEvidence,
    endpoint: String,
) -> Response {
    let audit_outcome = match decision.status.as_str() {
        "active" => PrivilegedAuditOutcome::Allow,
        "pending" | "denied" => PrivilegedAuditOutcome::AuthorizationDenied,
        _ => PrivilegedAuditOutcome::AuthorizationDenied,
    };

    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "risk_limit_profile_update".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "POST",
            "profile_key": decision.profile_key,
            "version": decision.version,
            "approval_status": decision.status,
            "inventory_rule_count": decision.inventory_rule_count,
        }),
        approval_reference: if decision.status == "active" {
            decision.approval_reference.clone()
        } else {
            None
        },
        timestamp: decision.updated_at_utc.clone(),
        outcome: audit_outcome,
        reason_code: decision.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: decision.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "risk_limit_profile_update".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.updated_at_utc,
        );
    }

    match decision.status.as_str() {
        "active" => (
            StatusCode::ACCEPTED,
            axum::Json(RiskLimitProfileDecisionResponse {
                status: "accepted",
                error_code: None,
                message: None,
                profile_key: decision.profile_key,
                version: decision.version,
                approval_status: "active".to_string(),
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                reason_code: decision.reason_code,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.updated_at_utc,
                approval_reference: decision.approval_reference,
                inventory_rule_count: decision.inventory_rule_count,
                security_signal: None,
            }),
        )
            .into_response(),
        "pending" => (
            StatusCode::ACCEPTED,
            axum::Json(RiskLimitProfileDecisionResponse {
                status: "pending",
                error_code: None,
                message: None,
                profile_key: decision.profile_key,
                version: decision.version,
                approval_status: "pending".to_string(),
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                reason_code: decision.reason_code,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.updated_at_utc,
                approval_reference: None,
                inventory_rule_count: decision.inventory_rule_count,
                security_signal: None,
            }),
        )
            .into_response(),
        _ => (
            StatusCode::FORBIDDEN,
            axum::Json(RiskLimitProfileDecisionResponse {
                status: "denied",
                error_code: Some(decision.reason_code.clone()),
                message: Some("risk limit mutation was denied".to_string()),
                profile_key: decision.profile_key,
                version: decision.version,
                approval_status: decision.status,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                reason_code: decision.reason_code,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.updated_at_utc,
                approval_reference: None,
                inventory_rule_count: decision.inventory_rule_count,
                security_signal: Some(RiskLimitSecuritySignal {
                    name: "unauthorized_risk_limit_mutation_attempt_v1",
                    severity: "high",
                    alert_compatible: true,
                    alert_target_seconds: 30,
                }),
            }),
        )
            .into_response(),
    }
}

fn risk_limit_pending_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    pending: Vec<RiskLimitProfileMutationEvidence>,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "risk_limit_pending_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "pending_count": pending.len(),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: RiskLimitReasonCode::ProfilePendingApproval
            .code()
            .to_string(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "risk_limit_pending_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc,
        );
    }

    (
        StatusCode::OK,
        axum::Json(RiskLimitPendingProfilesResponse {
            status: "accepted",
            action: "risk_limit_pending_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            pending_profiles: pending
                .into_iter()
                .map(|item| RiskLimitPendingProfileItem {
                    profile_key: item.profile_key,
                    version: item.version,
                    action_type: "risk_limit_profile_update".to_string(),
                    actor_id: item.actor_id,
                    approval_status: item.status,
                    reason_code: item.reason_code,
                    correlation_id: item.correlation_id,
                    updated_at_utc: item.updated_at_utc,
                    approval_reference: item.approval_reference,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn risk_limit_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<RiskLimitValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let security_signal = if error_code == "risk_limit_unauthorized_role" {
        Some(RiskLimitSecuritySignal {
            name: "unauthorized_risk_limit_mutation_attempt_v1",
            severity: "high",
            alert_compatible: true,
            alert_target_seconds: 30,
        })
    } else {
        None
    };

    (
        risk_limit_service_error_status(error_code),
        axum::Json(RiskLimitServiceErrorResponse {
            error_code,
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| RiskLimitFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
            security_signal,
        }),
    )
        .into_response()
}

fn risk_limit_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == RiskLimitReasonCode::InvalidPayload.code()
            || code == RiskLimitReasonCode::InvalidThreshold.code()
            || code == RiskLimitReasonCode::InvalidScopeInvariant.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        "risk_limit_unauthorized_role" => StatusCode::FORBIDDEN,
        "risk_limit_constraint_violation" => StatusCode::CONFLICT,
        code if code == RiskLimitReasonCode::PersistenceUnavailable.code()
            || code == "risk_limit_query_failed"
            || code == "risk_limit_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn allocation_policy_mutation_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: AllocationPolicyMutationEvidence,
    endpoint: String,
) -> Response {
    let audit_outcome = if decision.approval_status == "approved" {
        PrivilegedAuditOutcome::Allow
    } else {
        PrivilegedAuditOutcome::AuthorizationDenied
    };

    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "allocation_policy_update".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "POST",
            "policy_key": decision.policy_key,
            "version": decision.version,
            "approval_status": decision.approval_status,
        }),
        approval_reference: if decision.approval_status == "approved" {
            decision.approval_reference.clone()
        } else {
            None
        },
        timestamp: decision.updated_at_utc.clone(),
        outcome: audit_outcome,
        reason_code: decision.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: decision.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "allocation_policy_update".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.updated_at_utc,
        );
    }

    match decision.approval_status.as_str() {
        "approved" => (
            StatusCode::ACCEPTED,
            axum::Json(AllocationPolicyDecisionResponse {
                status: "accepted",
                error_code: None,
                message: None,
                policy_key: decision.policy_key,
                version: decision.version,
                approval_status: decision.approval_status,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                reason_code: decision.reason_code,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.updated_at_utc,
                approval_reference: decision.approval_reference,
                security_signal: None,
            }),
        )
            .into_response(),
        "pending" => (
            StatusCode::ACCEPTED,
            axum::Json(AllocationPolicyDecisionResponse {
                status: "pending",
                error_code: None,
                message: None,
                policy_key: decision.policy_key,
                version: decision.version,
                approval_status: decision.approval_status,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                reason_code: decision.reason_code,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.updated_at_utc,
                approval_reference: None,
                security_signal: None,
            }),
        )
            .into_response(),
        _ => (
            StatusCode::FORBIDDEN,
            axum::Json(AllocationPolicyDecisionResponse {
                status: "denied",
                error_code: Some(decision.reason_code.clone()),
                message: Some("allocation policy mutation was denied".to_string()),
                policy_key: decision.policy_key,
                version: decision.version,
                approval_status: decision.approval_status,
                actor_id: decision.actor_id,
                role: actor.role.clone(),
                reason_code: decision.reason_code,
                correlation_id: decision.correlation_id,
                timestamp_utc: decision.updated_at_utc,
                approval_reference: None,
                security_signal: Some(AllocationPolicySecuritySignal {
                    name: "unauthorized_allocation_policy_mutation_attempt_v1",
                    severity: "high",
                    alert_compatible: true,
                    alert_target_seconds: 30,
                }),
            }),
        )
            .into_response(),
    }
}

fn rebalance_recommendation_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: RebalanceRecommendationEvidence,
    endpoint: String,
    http_method: &'static str,
    action: &'static str,
) -> Response {
    let audit_outcome = match decision.status.as_str() {
        "executed" | "approved" | "proposed" => PrivilegedAuditOutcome::Allow,
        "pending_approval" | "denied" => PrivilegedAuditOutcome::AuthorizationDenied,
        _ => PrivilegedAuditOutcome::AuthorizationDenied,
    };
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "recommendation_id": decision.recommendation_id,
            "policy_key": decision.policy_key,
            "policy_version": decision.policy_version,
            "status": decision.status,
            "approval_status": decision.approval_status,
            "reason_code": decision.reason_code,
        }),
        approval_reference: if decision.approval_status == "approved" {
            decision.approval_reference.clone()
        } else {
            None
        },
        timestamp: decision.updated_at_utc.clone(),
        outcome: audit_outcome,
        reason_code: decision.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: decision.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            decision.correlation_id.clone(),
            decision.updated_at_utc,
        );
    }

    let (status_code, status, error_code, message, security_signal) = match decision.status.as_str()
    {
        "pending_approval" => (StatusCode::ACCEPTED, "pending", None, None, None),
        "denied" if decision.reason_code == RebalanceReasonCode::InBounds.code() => {
            (StatusCode::ACCEPTED, "accepted", None, None, None)
        }
        "denied" => (
            StatusCode::FORBIDDEN,
            "denied",
            Some(decision.reason_code.clone()),
            Some("rebalance recommendation was denied".to_string()),
            Some(AllocationPolicySecuritySignal {
                name: "rebalance_recommendation_denied_v1",
                severity: "high",
                alert_compatible: true,
                alert_target_seconds: 30,
            }),
        ),
        _ => (StatusCode::ACCEPTED, "accepted", None, None, None),
    };

    (
        status_code,
        axum::Json(RebalanceRecommendationDecisionResponse {
            status,
            error_code,
            message,
            recommendation_id: decision.recommendation_id,
            policy_key: decision.policy_key,
            policy_version: decision.policy_version,
            recommendation_status: decision.status,
            approval_status: decision.approval_status,
            action_type: decision.action_type,
            rationale: decision.rationale,
            recommended_next_action: decision.recommended_next_action,
            actor_id: decision.actor_id,
            role: actor.role.clone(),
            reason_code: decision.reason_code,
            correlation_id: decision.correlation_id,
            created_at_utc: decision.created_at_utc,
            timestamp_utc: decision.updated_at_utc,
            approval_reference: decision.approval_reference,
            security_signal,
        }),
    )
        .into_response()
}

fn rebalance_pending_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    pending: Vec<RebalanceRecommendationEvidence>,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "rebalance_pending_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "pending_count": pending.len(),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: RebalanceReasonCode::ApprovalRequired.code().to_string(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "rebalance_pending_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc,
        );
    }

    (
        StatusCode::OK,
        axum::Json(PendingRebalanceRecommendationsResponse {
            status: "accepted",
            action: "rebalance_pending_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            pending_recommendations: pending
                .into_iter()
                .map(|item| PendingRebalanceRecommendationItem {
                    recommendation_id: item.recommendation_id,
                    policy_key: item.policy_key,
                    policy_version: item.policy_version,
                    recommendation_status: item.status,
                    approval_status: item.approval_status,
                    action_type: item.action_type,
                    rationale: item.rationale,
                    recommended_next_action: item.recommended_next_action,
                    actor_id: item.actor_id,
                    reason_code: item.reason_code,
                    correlation_id: item.correlation_id,
                    created_at_utc: item.created_at_utc,
                    updated_at_utc: item.updated_at_utc,
                    approval_reference: item.approval_reference,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn allocation_policy_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<domain::allocation::AllocationValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let security_signal = if error_code == "allocation_policy_unauthorized_role" {
        Some(AllocationPolicySecuritySignal {
            name: "unauthorized_allocation_policy_mutation_attempt_v1",
            severity: "high",
            alert_compatible: true,
            alert_target_seconds: 30,
        })
    } else {
        None
    };
    (
        allocation_policy_service_error_status(error_code),
        axum::Json(AllocationPolicyServiceErrorResponse {
            error_code,
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| AllocationPolicyFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
            security_signal,
        }),
    )
        .into_response()
}

fn allocation_policy_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == RebalanceReasonCode::InvalidPayload.code()
            || code == RebalanceReasonCode::InvalidThreshold.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        "allocation_policy_unauthorized_role" => StatusCode::FORBIDDEN,
        "allocation_constraint_violation" => StatusCode::CONFLICT,
        code if code == RebalanceReasonCode::RecommendationNotFound.code() => StatusCode::NOT_FOUND,
        code if code == RebalanceReasonCode::RecommendationDenied.code() => StatusCode::CONFLICT,
        code if code == RebalanceReasonCode::ApprovalRequired.code() => StatusCode::CONFLICT,
        code if code == RebalanceReasonCode::PolicyStateUnavailable.code()
            || code == RebalanceReasonCode::PolicyStateStale.code()
            || code == RebalanceReasonCode::PersistenceUnavailable.code()
            || code == "allocation_query_failed"
            || code == "allocation_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn approval_reference_requires_request_id_response(
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    allocation_policy_service_error_response(
        RebalanceReasonCode::InvalidPayload.code(),
        "approval_reference cannot be supplied directly; provide approval_request_id and rely on approval workflow evidence."
            .to_string(),
        vec![domain::allocation::AllocationValidationIssue {
            field: "approval_reference",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "approval_reference requires approval_request_id and must originate from an approved request.".to_string(),
        }],
        action,
        actor,
        timestamp_utc,
        endpoint,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttributionDependencyState {
    Healthy,
    ProjectionUnavailable,
    StaleSource,
    ReconciliationUnavailable,
}

fn parse_attribution_dependency_state(
    value: Option<&str>,
) -> Result<AttributionDependencyState, domain::attribution::AttributionContractError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("healthy") => Ok(AttributionDependencyState::Healthy),
        Some("projection_unavailable") => Ok(AttributionDependencyState::ProjectionUnavailable),
        Some("stale_source") => Ok(AttributionDependencyState::StaleSource),
        Some("reconciliation_unavailable") => Ok(AttributionDependencyState::ReconciliationUnavailable),
        Some(other) => Err(domain::attribution::AttributionContractError::invalid_payload_with_issues(
            format!(
                "dependency_state `{other}` is not supported; expected healthy, projection_unavailable, stale_source, or reconciliation_unavailable"
            ),
            vec![AttributionValidationIssue {
                field: "dependency_state",
                code: AttributionReasonCode::InvalidPayload.code(),
                message: "dependency_state must be healthy, projection_unavailable, stale_source, or reconciliation_unavailable".to_string(),
            }],
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncidentDependencyState {
    Healthy,
    DependencyUnavailable,
    StaleEvidence,
}

fn parse_incident_dependency_state(
    value: Option<&str>,
) -> Result<IncidentDependencyState, domain::incidents::IncidentContractError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("healthy") => Ok(IncidentDependencyState::Healthy),
        Some("dependency_unavailable") => Ok(IncidentDependencyState::DependencyUnavailable),
        Some("stale_evidence") => Ok(IncidentDependencyState::StaleEvidence),
        Some(other) => Err(domain::incidents::IncidentContractError::invalid_payload_with_issues(
            format!(
                "dependency_state `{other}` is not supported; expected healthy, dependency_unavailable, or stale_evidence"
            ),
            vec![IncidentValidationIssue {
                field: "dependency_state",
                code: IncidentReasonCode::InvalidPayload.code(),
                message: "dependency_state must be healthy, dependency_unavailable, or stale_evidence".to_string(),
            }],
        )),
    }
}

fn synthetic_incident_timeline(as_of_utc: &str, correlation_id: &str) -> Vec<IncidentTimelineEvent> {
    let as_of = OffsetDateTime::parse(as_of_utc, &Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .to_offset(UtcOffset::UTC);
    let signal_at = (as_of - Duration::minutes(4))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let order_at = (as_of - Duration::minutes(3))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let fill_at = (as_of - Duration::minutes(2))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let pnl_at = (as_of - Duration::minutes(1))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let risk_at = (as_of - Duration::seconds(30))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");

    vec![
        IncidentTimelineEvent {
            event_id: "incident::signal::run-incident-001".to_string(),
            occurred_at: signal_at,
            stage: IncidentTimelineStage::Signal,
            source: "reconciliation.runs.v1".to_string(),
            reason_code: "reconciliation_non_critical_mismatch".to_string(),
            correlation_id: correlation_id.to_string(),
            summary: "Trigger: reconciliation run detected non-critical mismatch cluster."
                .to_string(),
            recommended_next_action:
                "Inspect correlated order mismatch events before executing recovery controls."
                    .to_string(),
            severity: "warning".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: None,
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-incident-001".to_string()),
            snapshot_id: None,
        },
        IncidentTimelineEvent {
            event_id: "incident::order::run-incident-001::order-101".to_string(),
            occurred_at: order_at,
            stage: IncidentTimelineStage::Order,
            source: "reconciliation.diffs.v1".to_string(),
            reason_code: "reconciliation_non_critical_mismatch".to_string(),
            correlation_id: correlation_id.to_string(),
            summary: "Context: order-101 lifecycle diverged between internal and venue states."
                .to_string(),
            recommended_next_action:
                "Compare internal and venue lifecycle progression for order-101.".to_string(),
            severity: "warning".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: Some("order-101".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-incident-001".to_string()),
            snapshot_id: None,
        },
        IncidentTimelineEvent {
            event_id: "incident::fill::snapshot-incident-001".to_string(),
            occurred_at: fill_at,
            stage: IncidentTimelineStage::Fill,
            source: "execution.fills.v1".to_string(),
            reason_code: "fill_latency_warning".to_string(),
            correlation_id: correlation_id.to_string(),
            summary: "Action: fill evidence indicates delayed completion for affected order."
                .to_string(),
            recommended_next_action:
                "Validate venue acknowledgement latency and residual queue pressure.".to_string(),
            severity: "warning".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: Some("order-101".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-incident-001".to_string()),
            snapshot_id: Some("snapshot-incident-001".to_string()),
        },
        IncidentTimelineEvent {
            event_id: "incident::pnl::snapshot-incident-001".to_string(),
            occurred_at: pnl_at,
            stage: IncidentTimelineStage::Pnl,
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: "attribution_ready".to_string(),
            correlation_id: correlation_id.to_string(),
            summary:
                "Verification: cost-aware attribution confirms realized drag for this incident."
                    .to_string(),
            recommended_next_action:
                "Quantify net-cost impact before deciding to de-risk or rebalance.".to_string(),
            severity: "normal".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: Some("order-101".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-incident-001".to_string()),
            snapshot_id: Some("snapshot-incident-001".to_string()),
        },
        IncidentTimelineEvent {
            event_id: "incident::risk_action::snapshot-incident-001".to_string(),
            occurred_at: risk_at,
            stage: IncidentTimelineStage::RiskAction,
            source: "risk.controls.v1".to_string(),
            reason_code: "risk_posture_warning".to_string(),
            correlation_id: correlation_id.to_string(),
            summary: "Recommended containment remains reduce-only until mismatch trajectory stabilizes."
                .to_string(),
            recommended_next_action:
                "Keep reduce-only active and re-run incident query after next reconciliation window."
                    .to_string(),
            severity: "warning".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: Some("order-101".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-incident-001".to_string()),
            snapshot_id: Some("snapshot-incident-001".to_string()),
        },
    ]
}

fn synthetic_attribution_observations(
    as_of_utc: &str,
    correlation_id: &str,
) -> Result<Vec<domain::attribution::AttributionObservation>, domain::attribution::AttributionContractError>
{
    let as_of = OffsetDateTime::parse(as_of_utc, &Rfc3339)
        .map_err(|_| {
            domain::attribution::AttributionContractError::invalid_payload_with_issues(
                "as_of_utc must be an RFC3339 UTC timestamp",
                vec![AttributionValidationIssue {
                    field: "as_of_utc",
                    code: AttributionReasonCode::InvalidPayload.code(),
                    message: "as_of_utc must be an RFC3339 UTC timestamp".to_string(),
                }],
            )
        })?
        .to_offset(UtcOffset::UTC);

    let point_30m = (as_of - Duration::minutes(30))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let point_2h = (as_of - Duration::hours(2))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let point_20h = (as_of - Duration::hours(20))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");
    let point_29d = (as_of - Duration::days(29))
        .format(&Rfc3339)
        .expect("UTC timestamp formatting must succeed");

    Ok(vec![
        domain::attribution::AttributionObservation {
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period_end_utc: point_30m,
            realized_pnl_usd: 132.5,
            unrealized_pnl_usd: 24.0,
            fees_usd: 6.8,
            rebates_usd: 1.9,
            incentives_usd: 0.6,
            as_of_utc: as_of_utc.to_string(),
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: correlation_id.to_string(),
            snapshot_id: Some("snapshot::attribution::btc::001".to_string()),
            run_id: Some("run::reconciliation::btc::001".to_string()),
        },
        domain::attribution::AttributionObservation {
            market_id: "market-eth-election".to_string(),
            alpha_id: "alpha-carry".to_string(),
            period_end_utc: point_2h,
            realized_pnl_usd: 45.0,
            unrealized_pnl_usd: 12.0,
            fees_usd: 3.2,
            rebates_usd: 0.7,
            incentives_usd: 0.2,
            as_of_utc: as_of_utc.to_string(),
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: correlation_id.to_string(),
            snapshot_id: Some("snapshot::attribution::eth::001".to_string()),
            run_id: Some("run::reconciliation::eth::001".to_string()),
        },
        domain::attribution::AttributionObservation {
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-carry".to_string(),
            period_end_utc: point_20h,
            realized_pnl_usd: 18.0,
            unrealized_pnl_usd: 9.0,
            fees_usd: 2.1,
            rebates_usd: 0.4,
            incentives_usd: 0.3,
            as_of_utc: as_of_utc.to_string(),
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: correlation_id.to_string(),
            snapshot_id: Some("snapshot::attribution::btc::002".to_string()),
            run_id: Some("run::reconciliation::btc::002".to_string()),
        },
        domain::attribution::AttributionObservation {
            market_id: "market-sol-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period_end_utc: point_29d,
            realized_pnl_usd: 80.0,
            unrealized_pnl_usd: 20.0,
            fees_usd: 5.0,
            rebates_usd: 0.9,
            incentives_usd: 0.0,
            as_of_utc: as_of_utc.to_string(),
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: correlation_id.to_string(),
            snapshot_id: Some("snapshot::attribution::sol::001".to_string()),
            run_id: Some("run::reconciliation::sol::001".to_string()),
        },
    ])
}

struct AttributionQueryWindow {
    period: AttributionPeriod,
    start_inclusive_utc: String,
    end_exclusive_utc: String,
    as_of_utc: String,
}

fn attribution_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    rows: Vec<AttributionRow>,
    window: AttributionQueryWindow,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let AttributionQueryWindow {
        period,
        start_inclusive_utc,
        end_exclusive_utc,
        as_of_utc,
    } = window;
    let data_state = if rows.is_empty() { "empty" } else { "ready" };
    let reason_code = if rows.is_empty() {
        AttributionReasonCode::EmptyWindow.code().to_string()
    } else {
        AttributionReasonCode::Ready.code().to_string()
    };
    let source = rows
        .first()
        .map(|row| row.source.clone())
        .unwrap_or_else(|| "portfolio-engine.attribution.v1".to_string());
    let correlation_id = rows
        .first()
        .map(|row| row.correlation_id.clone())
        .unwrap_or_else(|| actor.correlation_id.clone());
    let recommended_next_action = if rows.is_empty() {
        "No activity in this period. Expand to a wider period window or remove restrictive filters."
            .to_string()
    } else {
        "Review highest net contributors first, then verify cost drag (fees/rebates/incentives) against execution quality."
            .to_string()
    };

    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "attribution_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "period": period.as_str(),
            "rows": rows.len(),
            "data_state": data_state,
            "market_id": rows.first().map(|row| row.market_id.clone()),
            "alpha_id": rows.first().map(|row| row.alpha_id.clone()),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "attribution_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            correlation_id,
            timestamp_utc.clone(),
        );
    }

    (
        StatusCode::OK,
        axum::Json(AttributionQueryResponse {
            status: "accepted",
            action: "attribution_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            period: period.as_str().to_string(),
            start_inclusive_utc,
            end_exclusive_utc,
            as_of_utc,
            source,
            reason_code,
            data_state: data_state.to_string(),
            recommended_next_action,
            rows: rows
                .into_iter()
                .map(|row| AttributionRowItem {
                    market_id: row.market_id,
                    alpha_id: row.alpha_id,
                    period: row.period,
                    period_start_utc: row.period_start_utc,
                    period_end_utc: row.period_end_utc,
                    realized_pnl_usd: row.realized_pnl_usd,
                    unrealized_pnl_usd: row.unrealized_pnl_usd,
                    gross_pnl_usd: row.gross_pnl_usd,
                    net_pnl_usd: row.net_pnl_usd,
                    fees_usd: row.costs.fees_usd,
                    rebates_usd: row.costs.rebates_usd,
                    incentives_usd: row.costs.incentives_usd,
                    net_cost_impact_usd: row.costs.net_cost_impact_usd,
                    as_of_utc: row.as_of_utc,
                    source: row.source,
                    reason_code: row.reason_code,
                    correlation_id: row.correlation_id,
                    snapshot_id: row.snapshot_id,
                    run_id: row.run_id,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn attribution_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<AttributionValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        attribution_service_error_status(error_code),
        axum::Json(AttributionServiceErrorResponse {
            error_code,
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| AttributionFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn attribution_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == AttributionReasonCode::InvalidPayload.code() => StatusCode::BAD_REQUEST,
        code if code == AttributionReasonCode::Unauthorized.code() => StatusCode::FORBIDDEN,
        code if code == AttributionReasonCode::ProjectionUnavailable.code()
            || code == AttributionReasonCode::StaleSource.code()
            || code == AttributionReasonCode::PersistenceUnavailable.code()
            || code == "attribution_query_failed"
            || code == "attribution_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        "attribution_constraint_violation" => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn incident_forensics_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    filters: IncidentQueryFilters,
    events: Vec<IncidentTimelineEvent>,
    query_latency_ms: i64,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let start_inclusive_utc = filters.start_ts.clone();
    let end_exclusive_utc = filters.end_ts.clone();
    let data_state = if events.is_empty() { "empty" } else { "ready" };
    let reason_code = if events.is_empty() {
        IncidentReasonCode::EmptyWindow.code().to_string()
    } else {
        IncidentReasonCode::Ready.code().to_string()
    };
    let severity = incident_highest_severity(&events).to_string();
    let recommended_next_action = if events.is_empty() {
        "No incidents matched this window. Broaden filters or widen the time window for context."
            .to_string()
    } else if severity == "critical" {
        "Trigger pause or reduce-only controls, then verify downstream reconciliation and attribution evidence."
            .to_string()
    } else if severity == "degraded" {
        "Keep containment controls active and validate dependency freshness before resuming privileged operations."
            .to_string()
    } else if severity == "warning" {
        "Inspect order/fill divergence and validate risk posture before executing any privileged mutation."
            .to_string()
    } else {
        "Continue monitoring; no immediate containment action is required for this timeline."
            .to_string()
    };
    let source = events
        .first()
        .map(|event| event.source.clone())
        .unwrap_or_else(|| "incident.forensics.v1".to_string());
    let correlation_id = actor.correlation_id.clone();
    let causal_scope = select_causal_flow_scope(&events);
    let trigger = causal_scope
        .iter()
        .find(|event| event.stage == IncidentTimelineStage::Signal)
        .map(|event| event.summary.clone())
        .unwrap_or_else(|| "No trigger evidence available for selected incident scope.".to_string());
    let context = causal_scope
        .iter()
        .find(|event| event.stage == IncidentTimelineStage::Order)
        .map(|event| event.summary.clone())
        .unwrap_or_else(|| "No order-context evidence available for selected incident scope.".to_string());
    let action = causal_scope
        .iter()
        .find(|event| {
            event.stage == IncidentTimelineStage::RiskAction
                || event.stage == IncidentTimelineStage::Fill
        })
        .map(|event| event.summary.clone())
        .unwrap_or_else(|| {
            "No action evidence available; validate upstream event production health.".to_string()
        });
    let verification = causal_scope
        .iter()
        .find(|event| event.stage == IncidentTimelineStage::Pnl)
        .map(|event| event.summary.clone())
        .unwrap_or_else(|| {
            "No verification evidence available; confirm attribution/reconciliation dependencies."
                .to_string()
        });

    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "incident_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "query_latency_ms": query_latency_ms,
            "p95_latency_target_ms": 5000,
            "data_state": data_state,
            "severity": severity.clone(),
            "event_count": events.len(),
            "market_id": filters.market_id.clone(),
            "order_id": filters.order_id.clone(),
            "alpha_id": filters.alpha_id.clone(),
            "actor_id_filter": filters.actor_id.clone(),
            "start_ts": start_inclusive_utc.clone(),
            "end_ts": end_exclusive_utc.clone(),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "incident_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            correlation_id.clone(),
            timestamp_utc.clone(),
        );
    }

    let filters = IncidentFilterSummary {
        market_id: filters.market_id,
        order_id: filters.order_id,
        alpha_id: filters.alpha_id,
        actor_id: filters.actor_id,
    };

    (
        StatusCode::OK,
        axum::Json(IncidentForensicsQueryResponse {
            status: "accepted",
            action: "incident_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: correlation_id.clone(),
            timestamp_utc,
            start_inclusive_utc,
            end_exclusive_utc,
            source,
            reason_code,
            data_state: data_state.to_string(),
            severity,
            query_latency_ms,
            p95_latency_target_ms: 5_000,
            recommended_next_action,
            filters,
            causal_flow: IncidentCausalFlowSummary {
                trigger,
                context,
                action,
                verification,
            },
            events: events
                .into_iter()
                .map(|event| IncidentTimelineEventItem {
                    event_id: event.event_id,
                    occurred_at: event.occurred_at,
                    stage: event.stage.as_str().to_string(),
                    source: event.source,
                    reason_code: event.reason_code,
                    correlation_id: event.correlation_id,
                    summary: event.summary,
                    recommended_next_action: event.recommended_next_action,
                    severity: event.severity,
                    market_id: event.market_id,
                    order_id: event.order_id,
                    alpha_id: event.alpha_id,
                    actor_id: event.actor_id,
                    run_id: event.run_id,
                    snapshot_id: event.snapshot_id,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn incident_highest_severity(events: &[IncidentTimelineEvent]) -> &'static str {
    if events.iter().any(|event| event.severity == "critical") {
        "critical"
    } else if events.iter().any(|event| event.severity == "degraded") {
        "degraded"
    } else if events.iter().any(|event| event.severity == "warning") {
        "warning"
    } else {
        "normal"
    }
}

fn select_causal_flow_scope(events: &[IncidentTimelineEvent]) -> Vec<&IncidentTimelineEvent> {
    let Some(primary_event) = events.first() else {
        return Vec::new();
    };

    if let Some(primary_run_id) = primary_event.run_id.as_deref() {
        let grouped = events
            .iter()
            .filter(|event| event.run_id.as_deref() == Some(primary_run_id))
            .collect::<Vec<_>>();
        if !grouped.is_empty() {
            return grouped;
        }
    }

    events
        .iter()
        .filter(|event| event.correlation_id == primary_event.correlation_id)
        .collect::<Vec<_>>()
}

fn incident_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<IncidentValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        incident_service_error_status(error_code),
        axum::Json(IncidentForensicsServiceErrorResponse {
            error_code,
            reason_code: error_code.to_string(),
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc: timestamp_utc.clone(),
            occurred_at: timestamp_utc,
            source: "control-api.incident-forensics.v1".to_string(),
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| IncidentFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn incident_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == IncidentReasonCode::InvalidPayload.code() => StatusCode::BAD_REQUEST,
        code if code == IncidentReasonCode::Unauthorized.code() => StatusCode::FORBIDDEN,
        code if code == IncidentReasonCode::DependencyUnavailable.code()
            || code == IncidentReasonCode::StaleEvidence.code() =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn emergency_control_action_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    record: SafetyControlActionRecord,
    endpoint: String,
    action_type: &'static str,
    http_method: &'static str,
    status: StatusCode,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action_type.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "action_id": record.action_id.clone(),
            "action": record.action.as_str(),
            "source": record.source.as_str(),
            "trigger_source": record.trigger_source.as_str(),
            "resulting_mode": record.resulting_mode.as_str(),
        }),
        approval_reference: if record.audit_reference.is_empty() {
            None
        } else {
            Some(record.audit_reference.clone())
        },
        timestamp: record.completed_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: record.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: record.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action_type.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            record.correlation_id.clone(),
            record.completed_at_utc,
        );
    }

    (
        status,
        axum::Json(EmergencyControlDecisionResponse {
            status: "accepted",
            error_code: None,
            message: None,
            action_id: record.action_id,
            action: record.action.as_str().to_string(),
            source: record.source.as_str().to_string(),
            trigger_source: record.trigger_source.as_str().to_string(),
            resulting_mode: record.resulting_mode.as_str().to_string(),
            reason_code: record.reason_code,
            actor_id: record.actor_id,
            actor_role: record.actor_role,
            correlation_id: record.correlation_id,
            timestamp_utc: record.completed_at_utc,
            audit_reference: if record.audit_reference.is_empty() {
                None
            } else {
                Some(record.audit_reference)
            },
        }),
    )
        .into_response()
}

fn emergency_control_service_error_response(
    error_code: &'static str,
    message: String,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        emergency_control_service_error_status(error_code),
        axum::Json(EmergencyControlServiceErrorResponse {
            error_code,
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
        }),
    )
        .into_response()
}

fn emergency_control_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == EmergencyControlReasonCode::InvalidPayload.code() => {
            StatusCode::BAD_REQUEST
        }
        code if code == EmergencyControlReasonCode::UnauthorizedRole.code() => {
            StatusCode::FORBIDDEN
        }
        code if code == EmergencyControlReasonCode::NotFound.code() => StatusCode::NOT_FOUND,
        "emergency_control_constraint_violation" | "safety_control_constraint_violation" => {
            StatusCode::CONFLICT
        }
        code if code == EmergencyControlReasonCode::PersistenceUnavailable.code()
            || code == EmergencyControlReasonCode::OrchestrationUnavailable.code()
            || code == "emergency_control_query_failed"
            || code == "emergency_control_row_decode_failed"
            || code == "safety_control_query_failed"
            || code == "safety_control_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
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

#[derive(Debug, Deserialize)]
pub struct MarketPolicyProfilePayload {
    pub min_liquidity_usd: f64,
    pub max_spread_bps: f64,
    pub min_reward_score: f64,
    pub max_exposure_pct_nav: f64,
}

#[derive(Debug, Deserialize)]
pub struct MarketClusterTogglePayload {
    pub is_enabled: bool,
    pub reason_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationPolicyPayload {
    pub version: i64,
    pub portfolio_scope_id: String,
    pub target_exposure_pct_nav: f64,
    pub target_relative_alpha_weight: f64,
    #[serde(default = "default_exposure_drift_threshold_pct")]
    pub exposure_drift_threshold_pct: f64,
    #[serde(default = "default_relative_alpha_drift_threshold_pct")]
    pub relative_alpha_drift_threshold_pct: f64,
    #[serde(default = "default_allocation_advanced_parameters")]
    pub advanced_parameters: serde_json::Value,
    #[serde(default)]
    pub approval_request_id: Option<String>,
    #[serde(default)]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RebalanceDriftPayload {
    pub policy_key: String,
    pub exposure_drift_pct: f64,
    pub relative_alpha_drift_pct: f64,
    #[serde(default)]
    pub observed_at_utc: Option<String>,
    #[serde(default = "default_policy_stale_after_seconds")]
    pub stale_after_seconds: f64,
    #[serde(default)]
    pub require_execution: bool,
    #[serde(default)]
    pub approval_request_id: Option<String>,
    #[serde(default)]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PendingRebalanceRecommendationsQuery {
    #[serde(default)]
    pub policy_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttributionQuery {
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub alpha_id: Option<String>,
    #[serde(default)]
    pub period: Option<String>,
    #[serde(default)]
    pub as_of_utc: Option<String>,
    #[serde(default)]
    pub dependency_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IncidentForensicsQuery {
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub order_id: Option<String>,
    #[serde(default)]
    pub alpha_id: Option<String>,
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub start_ts: Option<String>,
    #[serde(default)]
    pub end_ts: Option<String>,
    #[serde(default)]
    pub dependency_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RebalanceExecutionPayload {
    #[serde(default)]
    pub executed_at_utc: Option<String>,
    #[serde(default)]
    pub approval_request_id: Option<String>,
    #[serde(default)]
    pub approval_reference: Option<String>,
}

fn default_exposure_drift_threshold_pct() -> f64 {
    DEFAULT_EXPOSURE_DRIFT_THRESHOLD_PCT
}

fn default_relative_alpha_drift_threshold_pct() -> f64 {
    DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT
}

fn default_policy_stale_after_seconds() -> f64 {
    DEFAULT_POLICY_STALE_AFTER_SECONDS
}

fn default_allocation_advanced_parameters() -> serde_json::Value {
    json!({})
}

#[derive(Debug, Deserialize)]
pub struct RiskLimitRulePayload {
    pub scope: String,
    pub scope_id: String,
    pub max_position_units: f64,
    pub max_order_size_units: f64,
    pub max_concentration_pct_nav: f64,
}

#[derive(Debug, Deserialize)]
pub struct RiskLimitProfilePayload {
    pub version: i64,
    pub portfolio_scope_id: String,
    pub market_scope_id: String,
    pub strategy_scope_id: String,
    pub portfolio_max_notional_usd: f64,
    pub market_max_notional_usd: f64,
    pub strategy_max_notional_usd: f64,
    pub portfolio_max_inventory_units: f64,
    pub market_max_inventory_units: f64,
    pub strategy_max_inventory_units: f64,
    pub portfolio_max_concentration_pct_nav: f64,
    pub market_max_concentration_pct_nav: f64,
    pub strategy_max_concentration_pct_nav: f64,
    #[serde(default)]
    pub inventory_rules: Vec<RiskLimitRulePayload>,
    #[serde(default)]
    pub approval_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmergencyControlPayload {
    #[serde(default)]
    pub audit_reference: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct MarketPolicyProfileDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub cluster_id: String,
    pub min_liquidity_usd: f64,
    pub max_spread_bps: f64,
    pub min_reward_score: f64,
    pub max_exposure_pct_nav: f64,
    pub actor_id: String,
    pub role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<MarketPolicySecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct MarketPolicyClusterToggleDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub cluster_id: String,
    pub is_enabled: bool,
    pub actor_id: String,
    pub role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<MarketPolicySecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct MarketPolicyServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<MarketPolicyFieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<MarketPolicySecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct MarketPolicyFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct MarketPolicySecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u16,
}

#[derive(Debug, Serialize)]
pub struct AllocationPolicyDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub policy_key: String,
    pub version: i64,
    pub approval_status: String,
    pub actor_id: String,
    pub role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<AllocationPolicySecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct RebalanceRecommendationDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub recommendation_id: String,
    pub policy_key: String,
    pub policy_version: i64,
    pub recommendation_status: String,
    pub approval_status: String,
    pub action_type: String,
    pub rationale: String,
    pub recommended_next_action: String,
    pub actor_id: String,
    pub role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub created_at_utc: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<AllocationPolicySecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct PendingRebalanceRecommendationsResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub pending_recommendations: Vec<PendingRebalanceRecommendationItem>,
}

#[derive(Debug, Serialize)]
pub struct PendingRebalanceRecommendationItem {
    pub recommendation_id: String,
    pub policy_key: String,
    pub policy_version: i64,
    pub recommendation_status: String,
    pub approval_status: String,
    pub action_type: String,
    pub rationale: String,
    pub recommended_next_action: String,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AttributionQueryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub period: String,
    pub start_inclusive_utc: String,
    pub end_exclusive_utc: String,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub data_state: String,
    pub recommended_next_action: String,
    pub rows: Vec<AttributionRowItem>,
}

#[derive(Debug, Serialize)]
pub struct AttributionRowItem {
    pub market_id: String,
    pub alpha_id: String,
    pub period: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub realized_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub gross_pnl_usd: f64,
    pub net_pnl_usd: f64,
    pub fees_usd: f64,
    pub rebates_usd: f64,
    pub incentives_usd: f64,
    pub net_cost_impact_usd: f64,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AttributionServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<AttributionFieldError>,
}

#[derive(Debug, Serialize)]
pub struct AttributionFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct IncidentForensicsQueryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub start_inclusive_utc: String,
    pub end_exclusive_utc: String,
    pub source: String,
    pub reason_code: String,
    pub data_state: String,
    pub severity: String,
    pub query_latency_ms: i64,
    pub p95_latency_target_ms: i64,
    pub recommended_next_action: String,
    pub filters: IncidentFilterSummary,
    pub causal_flow: IncidentCausalFlowSummary,
    pub events: Vec<IncidentTimelineEventItem>,
}

#[derive(Debug, Serialize)]
pub struct IncidentFilterSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IncidentCausalFlowSummary {
    pub trigger: String,
    pub context: String,
    pub action: String,
    pub verification: String,
}

#[derive(Debug, Serialize)]
pub struct IncidentTimelineEventItem {
    pub event_id: String,
    pub occurred_at: String,
    pub stage: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub summary: String,
    pub recommended_next_action: String,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IncidentForensicsServiceErrorResponse {
    pub error_code: &'static str,
    pub reason_code: String,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub occurred_at: String,
    pub source: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<IncidentFieldError>,
}

#[derive(Debug, Serialize)]
pub struct IncidentFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AllocationPolicyServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<AllocationPolicyFieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<AllocationPolicySecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct AllocationPolicyFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AllocationPolicySecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u16,
}

#[derive(Debug, Serialize)]
pub struct RiskLimitProfileDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub profile_key: String,
    pub version: i64,
    pub approval_status: String,
    pub actor_id: String,
    pub role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    pub inventory_rule_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<RiskLimitSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct RiskLimitPendingProfilesResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub pending_profiles: Vec<RiskLimitPendingProfileItem>,
}

#[derive(Debug, Serialize)]
pub struct RiskLimitPendingProfileItem {
    pub profile_key: String,
    pub version: i64,
    pub action_type: String,
    pub actor_id: String,
    pub approval_status: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RiskLimitServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<RiskLimitFieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<RiskLimitSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct RiskLimitFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RiskLimitSecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u16,
}

#[derive(Debug, Serialize)]
pub struct EmergencyControlDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub action_id: String,
    pub action: String,
    pub source: String,
    pub trigger_source: String,
    pub resulting_mode: String,
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_role: Option<String>,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmergencyControlServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
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
    use domain::risk::{
        EmergencyControlMode, EmergencyControlReasonCode, EmergencyControlSource,
        EmergencyControlTriggerSource,
    };
    use governance_service::{
        allocation_policy::{
            AllocationPolicyMutationEvidence, AllocationPolicyOrchestrator,
            AllocationPolicyService, AllocationPolicyServiceError, EvaluateRebalanceDriftInput,
            ExecuteRebalanceRecommendationInput, PendingRebalanceRecommendationsInput,
            RebalanceRecommendationEvidence, UpsertAllocationPolicyInput,
        },
        approvals::GovernanceApprovalService,
        audit::{AuditAppendError, PrivilegedAuditAppender},
        credentials::CredentialRotationService,
        market_policy::{
            MarketClusterToggleEvidence, MarketPolicyOrchestrator, MarketPolicyProfileEvidence,
            MarketPolicyServiceError, ToggleMarketClusterInput, UpsertMarketPolicyProfileInput,
        },
        risk_limits::{
            PendingRiskLimitProfilesInput, RiskLimitOrchestrator, RiskLimitProfileMutationEvidence,
            RiskLimitService, RiskLimitServiceError, UpsertRiskLimitProfileInput,
        },
        safety_controls::{
            EffectiveSafetyControlModeEvidence, ExecuteManualSafetyControlInput,
            HandleAutomaticSafetyTriggerInput, SafetyControlOrchestrator, SafetyControlService,
            SafetyControlServiceError,
        },
    };
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

    #[derive(Debug, Default)]
    struct StubMarketPolicyOrchestrator {
        profile_error: Option<(&'static str, &'static str)>,
        toggle_error: Option<(&'static str, &'static str)>,
    }

    impl MarketPolicyOrchestrator for StubMarketPolicyOrchestrator {
        fn upsert_market_policy_profile(
            &self,
            input: UpsertMarketPolicyProfileInput,
        ) -> Result<MarketPolicyProfileEvidence, MarketPolicyServiceError> {
            if let Some((code, message)) = self.profile_error {
                return Err(MarketPolicyServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            let normalized_cluster = input.cluster_id.trim().to_lowercase();
            Ok(MarketPolicyProfileEvidence {
                profile_id: format!("policy::{normalized_cluster}"),
                cluster_id: normalized_cluster,
                min_liquidity_usd: input.min_liquidity_usd,
                max_spread_bps: input.max_spread_bps,
                min_reward_score: input.min_reward_score,
                max_exposure_pct_nav: input.max_exposure_pct_nav,
                is_active: true,
                actor_id: input.actor_id,
                correlation_id: input.correlation_id,
                reason_code: "market_policy_profile_updated".to_string(),
                updated_at_utc: input.updated_at_utc,
            })
        }

        fn toggle_market_cluster(
            &self,
            input: ToggleMarketClusterInput,
        ) -> Result<MarketClusterToggleEvidence, MarketPolicyServiceError> {
            if let Some((code, message)) = self.toggle_error {
                return Err(MarketPolicyServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            let default_reason = if input.is_enabled {
                "market_policy_cluster_enabled"
            } else {
                "market_policy_cluster_disabled_by_operator"
            };
            Ok(MarketClusterToggleEvidence {
                cluster_id: input.cluster_id.trim().to_lowercase(),
                is_enabled: input.is_enabled,
                actor_id: input.actor_id,
                correlation_id: input.correlation_id,
                reason_code: input
                    .reason_code
                    .unwrap_or_else(|| default_reason.to_string()),
                updated_at_utc: input.updated_at_utc,
            })
        }
    }

    #[derive(Debug, Default)]
    struct StubRiskLimitOrchestrator {
        upsert_error: Option<(&'static str, &'static str)>,
        pending_error: Option<(&'static str, &'static str)>,
        force_pending: bool,
    }

    impl RiskLimitOrchestrator for StubRiskLimitOrchestrator {
        fn upsert_risk_limit_profile(
            &self,
            input: UpsertRiskLimitProfileInput,
        ) -> Result<RiskLimitProfileMutationEvidence, RiskLimitServiceError> {
            if let Some((code, message)) = self.upsert_error {
                return Err(RiskLimitServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            let status = if self.force_pending {
                "pending"
            } else {
                "active"
            };
            Ok(RiskLimitProfileMutationEvidence {
                profile_key: input.profile_key.trim().to_lowercase(),
                version: input.version,
                status: status.to_string(),
                actor_id: input.actor_id,
                reason_code: if status == "pending" {
                    RiskLimitReasonCode::ApprovalRequired.code().to_string()
                } else {
                    RiskLimitReasonCode::ProfileApplied.code().to_string()
                },
                correlation_id: input.correlation_id,
                updated_at_utc: input.updated_at_utc,
                approval_reference: if status == "active" {
                    input.approval_reference
                } else {
                    None
                },
                inventory_rule_count: input.inventory_rules.len(),
            })
        }

        fn list_pending_risk_limit_profiles(
            &self,
            input: PendingRiskLimitProfilesInput,
        ) -> Result<Vec<RiskLimitProfileMutationEvidence>, RiskLimitServiceError> {
            if let Some((code, message)) = self.pending_error {
                return Err(RiskLimitServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            Ok(vec![RiskLimitProfileMutationEvidence {
                profile_key: input
                    .profile_key
                    .unwrap_or_else(|| "default".to_string())
                    .to_lowercase(),
                version: 3,
                status: "pending".to_string(),
                actor_id: input.actor_id,
                reason_code: RiskLimitReasonCode::ApprovalRequired.code().to_string(),
                correlation_id: input.correlation_id,
                updated_at_utc: input.queried_at_utc,
                approval_reference: None,
                inventory_rule_count: 2,
            }])
        }
    }

    #[derive(Debug, Default)]
    struct StubAllocationPolicyOrchestrator {
        upsert_error: Option<(&'static str, &'static str)>,
        evaluate_error: Option<(&'static str, &'static str)>,
        execute_error: Option<(&'static str, &'static str)>,
        pending_error: Option<(&'static str, &'static str)>,
    }

    impl AllocationPolicyOrchestrator for StubAllocationPolicyOrchestrator {
        fn upsert_allocation_policy(
            &self,
            input: UpsertAllocationPolicyInput,
        ) -> Result<AllocationPolicyMutationEvidence, AllocationPolicyServiceError> {
            if let Some((code, message)) = self.upsert_error {
                return Err(AllocationPolicyServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            let approval_status = if input.approval_reference.is_some() {
                "approved"
            } else {
                "pending"
            };
            Ok(AllocationPolicyMutationEvidence {
                policy_key: input.policy_key.trim().to_lowercase(),
                version: input.version,
                approval_status: approval_status.to_string(),
                actor_id: input.actor_id,
                reason_code: if approval_status == "approved" {
                    RebalanceReasonCode::AllocationPolicyUpdated
                        .code()
                        .to_string()
                } else {
                    RebalanceReasonCode::AllocationPolicyPendingApproval
                        .code()
                        .to_string()
                },
                correlation_id: input.correlation_id,
                updated_at_utc: input.updated_at_utc,
                approval_reference: input.approval_reference,
            })
        }

        fn evaluate_rebalance_drift(
            &self,
            input: EvaluateRebalanceDriftInput,
        ) -> Result<RebalanceRecommendationEvidence, AllocationPolicyServiceError> {
            if let Some((code, message)) = self.evaluate_error {
                return Err(AllocationPolicyServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            let threshold_exceeded =
                input.exposure_drift_pct > 10.0 || input.relative_alpha_drift_pct > 15.0;
            let (status, approval_status, reason_code, recommended_next_action) =
                if !threshold_exceeded {
                    (
                        "denied",
                        "not_required",
                        RebalanceReasonCode::InBounds.code().to_string(),
                        "Continue monitoring drift telemetry; no rebalance action is required."
                            .to_string(),
                    )
                } else if input.require_execution && input.approval_reference.is_none() {
                    (
                        "pending_approval",
                        "pending",
                        RebalanceReasonCode::ApprovalRequired.code().to_string(),
                        "Complete dual approval for rebalance execution.".to_string(),
                    )
                } else if input.require_execution {
                    (
                        "approved",
                        "approved",
                        RebalanceReasonCode::RecommendationApproved
                            .code()
                            .to_string(),
                        "Execute approved recommendation.".to_string(),
                    )
                } else {
                    (
                        "proposed",
                        "not_required",
                        RebalanceReasonCode::RecommendationProposed
                            .code()
                            .to_string(),
                        "Review rationale and execute if portfolio intent remains valid."
                            .to_string(),
                    )
                };

            Ok(RebalanceRecommendationEvidence {
                recommendation_id: format!(
                    "reco::{}::{}",
                    input.policy_key.trim().to_lowercase(),
                    input.correlation_id.trim().to_lowercase()
                ),
                policy_key: input.policy_key.trim().to_lowercase(),
                policy_version: 1,
                status: status.to_string(),
                approval_status: approval_status.to_string(),
                action_type: if input.require_execution {
                    "execute".to_string()
                } else {
                    "recommend".to_string()
                },
                rationale: "Drift exceeded threshold and requires operator review.".to_string(),
                recommended_next_action,
                actor_id: input.actor_id,
                reason_code,
                correlation_id: input.correlation_id,
                created_at_utc: input.observed_at_utc.clone(),
                updated_at_utc: input.observed_at_utc,
                approval_reference: input.approval_reference,
            })
        }

        fn execute_rebalance_recommendation(
            &self,
            input: ExecuteRebalanceRecommendationInput,
        ) -> Result<RebalanceRecommendationEvidence, AllocationPolicyServiceError> {
            if let Some((code, message)) = self.execute_error {
                return Err(AllocationPolicyServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            Ok(RebalanceRecommendationEvidence {
                recommendation_id: input.recommendation_id,
                policy_key: "portfolio-default".to_string(),
                policy_version: 1,
                status: "executed".to_string(),
                approval_status: if input.approval_reference.is_some() {
                    "approved".to_string()
                } else {
                    "not_required".to_string()
                },
                action_type: "execute".to_string(),
                rationale:
                    "Drift exceeded threshold and recommendation executed by privileged actor."
                        .to_string(),
                recommended_next_action:
                    "Monitor post-execution drift and verify audit references.".to_string(),
                actor_id: input.actor_id,
                reason_code: RebalanceReasonCode::RecommendationExecuted
                    .code()
                    .to_string(),
                correlation_id: input.correlation_id,
                created_at_utc: input.executed_at_utc.clone(),
                updated_at_utc: input.executed_at_utc,
                approval_reference: input.approval_reference,
            })
        }

        fn list_pending_rebalance_recommendations(
            &self,
            input: PendingRebalanceRecommendationsInput,
        ) -> Result<Vec<RebalanceRecommendationEvidence>, AllocationPolicyServiceError> {
            if let Some((code, message)) = self.pending_error {
                return Err(AllocationPolicyServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            Ok(vec![RebalanceRecommendationEvidence {
                recommendation_id: format!(
                    "reco::{}::pending",
                    input
                        .policy_key
                        .clone()
                        .unwrap_or_else(|| "portfolio-default".to_string())
                ),
                policy_key: input
                    .policy_key
                    .unwrap_or_else(|| "portfolio-default".to_string()),
                policy_version: 2,
                status: "pending_approval".to_string(),
                approval_status: "pending".to_string(),
                action_type: "execute".to_string(),
                rationale: "Critical drift exceeds threshold and requires dual approval."
                    .to_string(),
                recommended_next_action: "Complete dual approval and execute recommendation."
                    .to_string(),
                actor_id: input.actor_id,
                reason_code: RebalanceReasonCode::ApprovalRequired.code().to_string(),
                correlation_id: input.correlation_id,
                created_at_utc: input.queried_at_utc.clone(),
                updated_at_utc: input.queried_at_utc,
                approval_reference: None,
            }])
        }
    }

    #[derive(Debug, Default)]
    struct StubSafetyControlOrchestrator {
        manual_error: Option<(&'static str, &'static str)>,
        query_error: Option<(&'static str, &'static str)>,
    }

    impl SafetyControlOrchestrator for StubSafetyControlOrchestrator {
        fn execute_manual_control(
            &self,
            input: ExecuteManualSafetyControlInput,
        ) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
            if let Some((code, message)) = self.manual_error {
                return Err(SafetyControlServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            let (resulting_mode, reason_code) = match input.action {
                EmergencyControlAction::Pause => (
                    EmergencyControlMode::Paused,
                    EmergencyControlReasonCode::PauseActivated
                        .code()
                        .to_string(),
                ),
                EmergencyControlAction::ReduceOnly => (
                    EmergencyControlMode::ReduceOnly,
                    EmergencyControlReasonCode::ReduceOnlyActivated
                        .code()
                        .to_string(),
                ),
                EmergencyControlAction::CancelAll => (
                    EmergencyControlMode::Paused,
                    EmergencyControlReasonCode::CancelAllAccepted
                        .code()
                        .to_string(),
                ),
            };

            Ok(SafetyControlActionRecord {
                action_id: format!("action::{}", input.action.as_str()),
                source: EmergencyControlSource::Manual,
                action: input.action,
                trigger_source: EmergencyControlTriggerSource::OperatorCommand,
                actor_id: Some(input.actor_id),
                actor_role: Some(input.actor_role),
                resulting_mode,
                reason_code,
                correlation_id: input.correlation_id,
                audit_reference: input
                    .audit_reference
                    .unwrap_or_else(|| "audit::manual".to_string()),
                dedupe_key: "dedupe::manual".to_string(),
                requested_at_utc: input.requested_at_utc.clone(),
                acknowledged_at_utc: input.requested_at_utc.clone(),
                effective_at_utc: input.requested_at_utc.clone(),
                completed_at_utc: input.requested_at_utc,
            })
        }

        fn handle_automatic_trigger(
            &self,
            input: HandleAutomaticSafetyTriggerInput,
        ) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
            Ok(SafetyControlActionRecord {
                action_id: format!("action::auto::{}", input.trigger_source.as_str()),
                source: EmergencyControlSource::Automatic,
                action: EmergencyControlAction::Pause,
                trigger_source: input.trigger_source,
                actor_id: None,
                actor_role: None,
                resulting_mode: EmergencyControlMode::Paused,
                reason_code: EmergencyControlReasonCode::StaleFeedTriggered
                    .code()
                    .to_string(),
                correlation_id: input.correlation_id,
                audit_reference: "audit::automatic".to_string(),
                dedupe_key: "dedupe::automatic".to_string(),
                requested_at_utc: input.requested_at_utc.clone(),
                acknowledged_at_utc: input.requested_at_utc.clone(),
                effective_at_utc: input.requested_at_utc.clone(),
                completed_at_utc: input.requested_at_utc,
            })
        }

        fn get_action_result(
            &self,
            action_id: &str,
        ) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
            if let Some((code, message)) = self.query_error {
                return Err(SafetyControlServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            Ok(SafetyControlActionRecord {
                action_id: action_id.to_string(),
                source: EmergencyControlSource::Automatic,
                action: EmergencyControlAction::Pause,
                trigger_source: EmergencyControlTriggerSource::StaleFeed,
                actor_id: None,
                actor_role: None,
                resulting_mode: EmergencyControlMode::Paused,
                reason_code: EmergencyControlReasonCode::StaleFeedTriggered
                    .code()
                    .to_string(),
                correlation_id: "corr-emergency-query-001".to_string(),
                audit_reference: "audit::query".to_string(),
                dedupe_key: "dedupe::query".to_string(),
                requested_at_utc: "2026-01-01T00:00:00Z".to_string(),
                acknowledged_at_utc: "2026-01-01T00:00:01Z".to_string(),
                effective_at_utc: "2026-01-01T00:00:02Z".to_string(),
                completed_at_utc: "2026-01-01T00:00:03Z".to_string(),
            })
        }

        fn get_action_result_by_correlation(
            &self,
            _correlation_id: &str,
        ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError> {
            Ok(None)
        }

        fn get_current_mode(
            &self,
        ) -> Result<Option<EffectiveSafetyControlModeEvidence>, SafetyControlServiceError> {
            Ok(Some(EffectiveSafetyControlModeEvidence {
                action_id: "action::mode".to_string(),
                correlation_id: "corr-emergency-mode-001".to_string(),
                resulting_mode: EmergencyControlMode::Paused.as_str().to_string(),
                reason_code: EmergencyControlReasonCode::PauseActive.code().to_string(),
                effective_at_utc: "2026-01-01T00:00:00Z".to_string(),
            }))
        }
    }

    fn test_app_with_market_policy_orchestrator(
        market_policy_orchestrator: Arc<dyn MarketPolicyOrchestrator>,
    ) -> Router {
        test_app_with_state(ControlApiState::with_all_orchestrators(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(HeaderTokenAuthenticator),
            Arc::new(CapturingAuditAppender::default()),
            Arc::new(GovernanceApprovalService::default()),
            Arc::new(CredentialRotationService::default()),
            Arc::new(AllocationPolicyService::default()),
            market_policy_orchestrator,
            Arc::new(RiskLimitService::default()),
            Arc::new(SafetyControlService::default()),
        ))
    }

    fn test_app_with_allocation_policy_orchestrator(
        allocation_policy_orchestrator: Arc<dyn AllocationPolicyOrchestrator>,
    ) -> Router {
        test_app_with_state(ControlApiState::with_all_orchestrators(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(HeaderTokenAuthenticator),
            Arc::new(CapturingAuditAppender::default()),
            Arc::new(GovernanceApprovalService::default()),
            Arc::new(CredentialRotationService::default()),
            allocation_policy_orchestrator,
            Arc::new(StubMarketPolicyOrchestrator::default()),
            Arc::new(RiskLimitService::default()),
            Arc::new(SafetyControlService::default()),
        ))
    }

    fn test_app_with_risk_limit_orchestrator(
        risk_limit_orchestrator: Arc<dyn RiskLimitOrchestrator>,
    ) -> Router {
        test_app_with_state(ControlApiState::with_all_orchestrators(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(HeaderTokenAuthenticator),
            Arc::new(CapturingAuditAppender::default()),
            Arc::new(GovernanceApprovalService::default()),
            Arc::new(CredentialRotationService::default()),
            Arc::new(AllocationPolicyService::default()),
            Arc::new(StubMarketPolicyOrchestrator::default()),
            risk_limit_orchestrator,
            Arc::new(SafetyControlService::default()),
        ))
    }

    fn test_app_with_safety_control_orchestrator(
        safety_control_orchestrator: Arc<dyn SafetyControlOrchestrator>,
    ) -> Router {
        test_app_with_state(ControlApiState::with_all_orchestrators(
            Arc::new(GovernanceAuthorizationGuard::new(
                AuthorizationEvaluator::default(),
            )),
            Arc::new(HeaderTokenAuthenticator),
            Arc::new(CapturingAuditAppender::default()),
            Arc::new(GovernanceApprovalService::default()),
            Arc::new(CredentialRotationService::default()),
            Arc::new(AllocationPolicyService::default()),
            Arc::new(StubMarketPolicyOrchestrator::default()),
            Arc::new(RiskLimitService::default()),
            safety_control_orchestrator,
        ))
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

    #[tokio::test]
    async fn market_policy_profile_route_reuses_authorization_guard_for_unauthorized_role() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/profiles/cluster_alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-authz-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "min_liquidity_usd":500.0,
                            "max_spread_bps":2.5,
                            "min_reward_score":0.2,
                            "max_exposure_pct_nav":25.0
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
    async fn market_policy_profile_route_returns_machine_readable_success_evidence() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/profiles/cluster_alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-profile-accept-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "min_liquidity_usd":500.0,
                            "max_spread_bps":2.5,
                            "min_reward_score":0.2,
                            "max_exposure_pct_nav":25.0
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
        assert_eq!(payload["cluster_id"], "cluster_alpha");
        assert_eq!(payload["reason_code"], "market_policy_profile_updated");
        assert_eq!(payload["actor_id"], "ops-1");
        assert!(payload["error_code"].is_null());
    }

    #[tokio::test]
    async fn market_policy_profile_route_rejects_invalid_threshold_payload_with_field_errors() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/profiles/cluster_alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-invalid-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "min_liquidity_usd":500.0,
                            "max_spread_bps":2.5,
                            "min_reward_score":0.2,
                            "max_exposure_pct_nav":100.1
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
        assert_eq!(payload["error_code"], "market_policy_invalid_payload");
        assert_eq!(payload["action"], "market_policy_profile_update");
        assert_eq!(payload["field_errors"][0]["field"], "max_exposure_pct_nav");
        assert!(payload["security_signal"].is_null());
    }

    #[tokio::test]
    async fn market_policy_profile_route_surfaces_service_unavailable_machine_error() {
        let app =
            test_app_with_market_policy_orchestrator(Arc::new(StubMarketPolicyOrchestrator {
                profile_error: Some((
                    "market_policy_persistence_unavailable",
                    "market policy repository unavailable",
                )),
                ..Default::default()
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/profiles/cluster_alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-profile-failure-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "min_liquidity_usd":500.0,
                            "max_spread_bps":2.5,
                            "min_reward_score":0.2,
                            "max_exposure_pct_nav":25.0
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
        assert_eq!(
            payload["error_code"],
            "market_policy_persistence_unavailable"
        );
        assert_eq!(payload["action"], "market_policy_profile_update");
    }

    #[tokio::test]
    async fn market_policy_profile_route_surfaces_unknown_service_error_as_internal_server_error() {
        let app =
            test_app_with_market_policy_orchestrator(Arc::new(StubMarketPolicyOrchestrator {
                profile_error: Some(("market_policy_unclassified_failure", "opaque failure")),
                ..Default::default()
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/profiles/cluster_alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-profile-failure-002")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "min_liquidity_usd":500.0,
                            "max_spread_bps":2.5,
                            "min_reward_score":0.2,
                            "max_exposure_pct_nav":25.0
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["error_code"], "market_policy_unclassified_failure");
        assert_eq!(payload["action"], "market_policy_profile_update");
    }

    #[tokio::test]
    async fn market_policy_cluster_toggle_route_supports_runtime_state_updates_without_redeploy() {
        let app = test_app();
        let disable_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/clusters/cluster_alpha/toggle")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-toggle-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "is_enabled": false,
                            "reason_code": "market_policy_cluster_disabled_by_operator"
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(disable_response.status(), StatusCode::ACCEPTED);
        let disable_payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(disable_response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(
            disable_payload["reason_code"],
            "market_policy_cluster_disabled_by_operator"
        );
        assert_eq!(disable_payload["is_enabled"], false);

        let enable_response = app
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/clusters/cluster_alpha/toggle")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-toggle-002")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "is_enabled": true,
                            "reason_code": "market_policy_cluster_enabled"
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(enable_response.status(), StatusCode::ACCEPTED);
        let enable_payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(enable_response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(
            enable_payload["reason_code"],
            "market_policy_cluster_enabled"
        );
        assert_eq!(enable_payload["is_enabled"], true);
    }

    #[tokio::test]
    async fn market_policy_cluster_toggle_route_surfaces_conflict_machine_error() {
        let app =
            test_app_with_market_policy_orchestrator(Arc::new(StubMarketPolicyOrchestrator {
                toggle_error: Some((
                    "market_policy_constraint_violation",
                    "active profile state prevents this toggle",
                )),
                ..Default::default()
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/market-policy/clusters/cluster_alpha/toggle")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-market-policy-toggle-failure-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "is_enabled": false,
                            "reason_code": "market_policy_cluster_disabled_by_operator"
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
        assert_eq!(payload["error_code"], "market_policy_constraint_violation");
        assert_eq!(payload["action"], "market_policy_cluster_toggle");
    }

    #[tokio::test]
    async fn risk_limit_profile_route_reuses_authorization_guard_for_unauthorized_role() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/risk-limits/profiles/default")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-risk-limit-authz-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "version": 1,
                            "portfolio_scope_id": "portfolio::default",
                            "market_scope_id": "market::sports",
                            "strategy_scope_id": "strategy::maker",
                            "portfolio_max_notional_usd": 1000.0,
                            "market_max_notional_usd": 600.0,
                            "strategy_max_notional_usd": 300.0,
                            "portfolio_max_inventory_units": 800.0,
                            "market_max_inventory_units": 400.0,
                            "strategy_max_inventory_units": 200.0,
                            "portfolio_max_concentration_pct_nav": 45.0,
                            "market_max_concentration_pct_nav": 30.0,
                            "strategy_max_concentration_pct_nav": 20.0,
                            "inventory_rules": []
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
    async fn risk_limit_profile_route_returns_machine_readable_active_evidence() {
        let app =
            test_app_with_risk_limit_orchestrator(Arc::new(StubRiskLimitOrchestrator::default()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/risk-limits/profiles/default")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-risk-limit-accept-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "version": 1,
                            "portfolio_scope_id": "portfolio::default",
                            "market_scope_id": "market::sports",
                            "strategy_scope_id": "strategy::maker",
                            "portfolio_max_notional_usd": 1000.0,
                            "market_max_notional_usd": 600.0,
                            "strategy_max_notional_usd": 300.0,
                            "portfolio_max_inventory_units": 800.0,
                            "market_max_inventory_units": 400.0,
                            "strategy_max_inventory_units": 200.0,
                            "portfolio_max_concentration_pct_nav": 45.0,
                            "market_max_concentration_pct_nav": 30.0,
                            "strategy_max_concentration_pct_nav": 20.0,
                            "inventory_rules": [
                                {
                                    "scope": "market",
                                    "scope_id": "market::sports",
                                    "max_position_units": 300.0,
                                    "max_order_size_units": 40.0,
                                    "max_concentration_pct_nav": 25.0
                                },
                                {
                                    "scope": "strategy",
                                    "scope_id": "strategy::maker",
                                    "max_position_units": 150.0,
                                    "max_order_size_units": 20.0,
                                    "max_concentration_pct_nav": 15.0
                                }
                            ]
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
        assert_eq!(payload["approval_status"], "active");
        assert_eq!(payload["reason_code"], "risk_limit_profile_applied");
        assert_eq!(payload["inventory_rule_count"], 2);
    }

    #[tokio::test]
    async fn risk_limit_profile_route_returns_pending_without_synthetic_approval_reference() {
        let app = test_app_with_risk_limit_orchestrator(Arc::new(StubRiskLimitOrchestrator {
            force_pending: true,
            ..Default::default()
        }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/risk-limits/profiles/default")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-risk-limit-pending-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "version": 2,
                            "portfolio_scope_id": "portfolio::default",
                            "market_scope_id": "market::sports",
                            "strategy_scope_id": "strategy::maker",
                            "portfolio_max_notional_usd": 1000.0,
                            "market_max_notional_usd": 650.0,
                            "strategy_max_notional_usd": 300.0,
                            "portfolio_max_inventory_units": 800.0,
                            "market_max_inventory_units": 450.0,
                            "strategy_max_inventory_units": 200.0,
                            "portfolio_max_concentration_pct_nav": 45.0,
                            "market_max_concentration_pct_nav": 35.0,
                            "strategy_max_concentration_pct_nav": 20.0,
                            "inventory_rules": []
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
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["approval_status"], "pending");
        assert_eq!(payload["reason_code"], "risk_limit_approval_required");
        assert!(payload["approval_reference"].is_null());
    }

    #[tokio::test]
    async fn risk_limit_pending_route_returns_queryable_pending_evidence() {
        let app =
            test_app_with_risk_limit_orchestrator(Arc::new(StubRiskLimitOrchestrator::default()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/risk-limits/pending")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-risk-limit-query-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["action"], "risk_limit_pending_query");
        assert_eq!(
            payload["pending_profiles"][0]["action_type"],
            "risk_limit_profile_update"
        );
        assert_eq!(payload["pending_profiles"][0]["approval_status"], "pending");
        assert_eq!(
            payload["pending_profiles"][0]["reason_code"],
            "risk_limit_approval_required"
        );
    }

    #[tokio::test]
    async fn risk_limit_pending_route_authorization_audit_records_get_http_method() {
        let audit_appender = Arc::new(CapturingAuditAppender::default());
        let app = test_app_with_audit_appender(audit_appender.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/risk-limits/pending")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-risk-limit-pending-audit-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);

        let records = audit_appender.snapshot();
        let auth_record = records
            .iter()
            .find(|record| record.action_type == "execute_control_plane_action")
            .expect("authorization audit record should exist");
        assert_eq!(
            auth_record.parameters["endpoint"],
            "/control/risk-limits/pending"
        );
        assert_eq!(auth_record.parameters["http_method"], "GET");
    }

    #[tokio::test]
    async fn risk_limit_profile_route_surfaces_service_unavailable_machine_error() {
        let app = test_app_with_risk_limit_orchestrator(Arc::new(StubRiskLimitOrchestrator {
            upsert_error: Some((
                "risk_limit_persistence_unavailable",
                "risk limit repository unavailable",
            )),
            ..Default::default()
        }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/risk-limits/profiles/default")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-risk-limit-failure-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "version": 1,
                            "portfolio_scope_id": "portfolio::default",
                            "market_scope_id": "market::sports",
                            "strategy_scope_id": "strategy::maker",
                            "portfolio_max_notional_usd": 1000.0,
                            "market_max_notional_usd": 600.0,
                            "strategy_max_notional_usd": 300.0,
                            "portfolio_max_inventory_units": 800.0,
                            "market_max_inventory_units": 400.0,
                            "strategy_max_inventory_units": 200.0,
                            "portfolio_max_concentration_pct_nav": 45.0,
                            "market_max_concentration_pct_nav": 30.0,
                            "strategy_max_concentration_pct_nav": 20.0,
                            "inventory_rules": []
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
        assert_eq!(payload["error_code"], "risk_limit_persistence_unavailable");
        assert_eq!(payload["action"], "risk_limit_profile_update");
    }

    #[tokio::test]
    async fn allocation_policy_route_returns_pending_machine_readable_evidence() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/allocation-policies/portfolio-default")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-allocation-policy-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "version": 1,
                            "portfolio_scope_id": "portfolio::default",
                            "target_exposure_pct_nav": 32.5,
                            "target_relative_alpha_weight": 1.15
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
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["approval_status"], "pending");
        assert_eq!(payload["reason_code"], "allocation_policy_pending_approval");
        assert_eq!(payload["policy_key"], "portfolio-default");
    }

    #[tokio::test]
    async fn allocation_policy_route_rejects_client_supplied_approval_reference_without_request() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/allocation-policies/portfolio-default")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-allocation-policy-invalid-approval-ref-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "version": 2,
                            "portfolio_scope_id": "portfolio::default",
                            "target_exposure_pct_nav": 35.0,
                            "target_relative_alpha_weight": 1.20,
                            "approval_reference": "apr-unsafe-client-value"
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
        assert_eq!(payload["error_code"], "rebalance_invalid_payload");
        assert_eq!(payload["action"], "allocation_policy_update");
        assert_eq!(payload["field_errors"][0]["field"], "approval_reference");
    }

    #[tokio::test]
    async fn rebalance_route_returns_recommendation_context_for_drift_exceedance() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
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
                    .header("x-correlation-id", "corr-rebalance-route-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "policy_key": "portfolio-default",
                            "exposure_drift_pct": 12.0,
                            "relative_alpha_drift_pct": 11.0,
                            "require_execution": false
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
        assert_eq!(payload["recommendation_status"], "proposed");
        assert_eq!(payload["approval_status"], "not_required");
        assert_eq!(payload["reason_code"], "rebalance_recommendation_proposed");
        assert_eq!(payload["action_type"], "recommend");
        assert!(
            payload["rationale"]
                .as_str()
                .expect("rationale should be a string")
                .contains("Drift exceeded threshold")
        );
    }

    #[tokio::test]
    async fn rebalance_route_rejects_client_supplied_approval_reference_without_request() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
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
                    .header("x-correlation-id", "corr-rebalance-invalid-approval-ref-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "policy_key": "portfolio-default",
                            "exposure_drift_pct": 18.0,
                            "relative_alpha_drift_pct": 22.0,
                            "require_execution": true,
                            "approval_reference": "apr-unsafe-client-value"
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
        assert_eq!(payload["error_code"], "rebalance_invalid_payload");
        assert_eq!(payload["action"], "rebalance_recommendation_evaluate");
        assert_eq!(payload["field_errors"][0]["field"], "approval_reference");
    }

    #[tokio::test]
    async fn rebalance_pending_route_returns_queryable_pending_recommendation_evidence() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance/recommendations/pending?policy_key=portfolio-default")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rebalance-pending-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["action"], "rebalance_pending_query");
        assert_eq!(
            payload["pending_recommendations"][0]["recommendation_status"],
            "pending_approval"
        );
        assert_eq!(
            payload["pending_recommendations"][0]["reason_code"],
            "rebalance_approval_required"
        );
    }

    #[tokio::test]
    async fn rebalance_execute_route_returns_executed_machine_readable_evidence() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance/recommendations/reco-001/execute")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("admin-1", "administrative_actions", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rebalance-execute-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{}"#))
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
        assert_eq!(payload["recommendation_status"], "executed");
        assert_eq!(payload["approval_status"], "not_required");
        assert_eq!(payload["reason_code"], "rebalance_recommendation_executed");
        assert_eq!(payload["approval_reference"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn rebalance_execute_route_rejects_client_supplied_approval_reference_without_request() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/rebalance/recommendations/reco-001/execute")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("admin-1", "administrative_actions", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-rebalance-execute-invalid-approval-ref-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"approval_reference":"apr-unsafe-client-value"}"#))
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
        assert_eq!(payload["error_code"], "rebalance_invalid_payload");
        assert_eq!(payload["action"], "rebalance_recommendation_execute");
        assert_eq!(payload["field_errors"][0]["field"], "approval_reference");
    }

    #[tokio::test]
    async fn rebalance_route_surfaces_service_unavailable_machine_error() {
        let app = test_app_with_allocation_policy_orchestrator(Arc::new(
            StubAllocationPolicyOrchestrator {
                evaluate_error: Some((
                    "rebalance_persistence_unavailable",
                    "allocation persistence dependency unavailable",
                )),
                ..Default::default()
            },
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
                    .header("x-correlation-id", "corr-rebalance-error-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "policy_key": "portfolio-default",
                            "exposure_drift_pct": 18.0,
                            "relative_alpha_drift_pct": 22.0,
                            "require_execution": true
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
        assert_eq!(payload["error_code"], "rebalance_persistence_unavailable");
        assert_eq!(payload["action"], "rebalance_recommendation_evaluate");
    }

    #[tokio::test]
    async fn attribution_route_returns_cost_aware_rows_with_traceable_metadata() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/portfolio/attribution?period=24h")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-attribution-route-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["action"], "attribution_query");
        assert_eq!(payload["period"], "24h");
        assert_eq!(payload["data_state"], "ready");
        assert_eq!(payload["rows"][0]["market_id"], "market-btc-election");
        assert!(payload["rows"][0]["as_of_utc"].is_string());
        assert_eq!(payload["rows"][0]["source"], "reconciliation.exposure.v1");
        assert_eq!(payload["rows"][0]["reason_code"], "attribution_ready");
        assert_eq!(payload["rows"][0]["correlation_id"], "corr-attribution-route-001");
    }

    #[tokio::test]
    async fn attribution_route_keeps_response_timestamp_server_generated() {
        let requested_as_of = "2026-03-01T00:00:00Z";
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/portfolio/attribution?period=24h&as_of_utc=2026-03-01T00:00:00Z")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-attribution-route-002")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["as_of_utc"], requested_as_of);
        assert_ne!(payload["timestamp_utc"], requested_as_of);
    }

    #[tokio::test]
    async fn attribution_route_returns_actionable_empty_state_for_zero_activity_windows() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/portfolio/attribution?period=1h&market_id=market-no-activity")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-attribution-empty-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["data_state"], "empty");
        assert_eq!(payload["reason_code"], "attribution_empty_window");
        assert!(payload["rows"].as_array().expect("rows should be array").is_empty());
        assert!(
            payload["recommended_next_action"]
                .as_str()
                .expect("recommended action should be string")
                .contains("Expand to a wider period")
        );
    }

    #[tokio::test]
    async fn attribution_route_rejects_unsupported_period_filter() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/portfolio/attribution?period=7d")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-attribution-invalid-period-001")
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

        assert_eq!(payload["error_code"], "attribution_invalid_payload");
        assert_eq!(payload["action"], "attribution_query");
        assert_eq!(payload["field_errors"][0]["field"], "period");
    }

    #[tokio::test]
    async fn attribution_route_surfaces_projection_unavailable_machine_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/control/portfolio/attribution?period=24h&dependency_state=projection_unavailable",
                    )
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-attribution-projection-001")
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

        assert_eq!(payload["error_code"], "attribution_projection_unavailable");
        assert_eq!(payload["action"], "attribution_query");
    }

    #[tokio::test]
    async fn attribution_route_returns_attribution_unauthorized_for_denied_reads() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/portfolio/attribution?period=24h")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-attribution-denied-001")
                    .body(Body::empty())
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

        assert_eq!(payload["error_code"], "attribution_unauthorized");
        assert_eq!(payload["action"], "attribution_query");
    }

    #[tokio::test]
    async fn incident_forensics_route_returns_causal_timeline_with_traceable_metadata() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/forensics?market_id=market-btc-election&start_ts=2025-01-01T00:00:00Z&end_ts=2030-01-01T00:00:00Z")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-incident-route-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["action"], "incident_query");
        assert_eq!(payload["data_state"], "ready");
        assert_eq!(payload["reason_code"], "incident_ready");
        assert_eq!(payload["filters"]["market_id"], "market-btc-election");
        assert_eq!(payload["p95_latency_target_ms"], 5_000);
        assert!(
            payload["query_latency_ms"]
                .as_i64()
                .expect("query latency should be integer")
                <= 5_000
        );
        assert!(payload["events"].as_array().is_some_and(|events| !events.is_empty()));
        assert!(payload["events"][0]["occurred_at"].is_string());
        assert!(payload["events"][0]["reason_code"].is_string());
        assert!(payload["events"][0]["correlation_id"].is_string());
        assert!(payload["causal_flow"]["trigger"].is_string());
        assert!(payload["causal_flow"]["context"].is_string());
        assert!(payload["causal_flow"]["action"].is_string());
        assert!(payload["causal_flow"]["verification"].is_string());
    }

    #[tokio::test]
    async fn incident_forensics_route_measures_p95_latency_within_target_for_repeated_queries() {
        let mut latencies = Vec::new();

        for index in 0..20 {
            let response = test_app()
                .oneshot(
                    Request::builder()
                        .uri("/control/incidents/forensics?market_id=market-btc-election&start_ts=2025-01-01T00:00:00Z&end_ts=2030-01-01T00:00:00Z")
                        .method("GET")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", format!("corr-incident-p95-{index:03}"))
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("request should complete");

            assert_eq!(response.status(), StatusCode::OK);
            let payload: serde_json::Value = serde_json::from_slice(
                &to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("body should be readable"),
            )
            .expect("payload should be valid json");
            latencies.push(
                payload["query_latency_ms"]
                    .as_i64()
                    .expect("query_latency_ms should be integer"),
            );
        }

        latencies.sort_unstable();
        let p95_index = (latencies.len() * 95).div_ceil(100) - 1;
        let p95_latency_ms = latencies[p95_index];
        assert!(p95_latency_ms <= 5_000);
    }

    #[test]
    fn incident_forensics_causal_flow_scope_uses_single_incident_chain() {
        let events = vec![
            IncidentTimelineEvent {
                event_id: "incident::signal::run-a".to_string(),
                occurred_at: "2026-04-06T15:00:00Z".to_string(),
                stage: IncidentTimelineStage::Signal,
                source: "reconciliation.runs.v1".to_string(),
                reason_code: "incident_ready".to_string(),
                correlation_id: "corr-a".to_string(),
                summary: "signal-a".to_string(),
                recommended_next_action: "act-a".to_string(),
                severity: "warning".to_string(),
                market_id: Some("market-btc-election".to_string()),
                order_id: None,
                alpha_id: Some("alpha-momentum".to_string()),
                actor_id: Some("ops-1".to_string()),
                run_id: Some("run-a".to_string()),
                snapshot_id: None,
            },
            IncidentTimelineEvent {
                event_id: "incident::order::run-b".to_string(),
                occurred_at: "2026-04-06T14:59:00Z".to_string(),
                stage: IncidentTimelineStage::Order,
                source: "reconciliation.diffs.v1".to_string(),
                reason_code: "incident_ready".to_string(),
                correlation_id: "corr-b".to_string(),
                summary: "order-b".to_string(),
                recommended_next_action: "act-b".to_string(),
                severity: "warning".to_string(),
                market_id: Some("market-btc-election".to_string()),
                order_id: Some("order-b".to_string()),
                alpha_id: Some("alpha-momentum".to_string()),
                actor_id: Some("ops-2".to_string()),
                run_id: Some("run-b".to_string()),
                snapshot_id: None,
            },
            IncidentTimelineEvent {
                event_id: "incident::order::run-a".to_string(),
                occurred_at: "2026-04-06T14:58:00Z".to_string(),
                stage: IncidentTimelineStage::Order,
                source: "reconciliation.diffs.v1".to_string(),
                reason_code: "incident_ready".to_string(),
                correlation_id: "corr-a".to_string(),
                summary: "order-a".to_string(),
                recommended_next_action: "act-a".to_string(),
                severity: "warning".to_string(),
                market_id: Some("market-btc-election".to_string()),
                order_id: Some("order-a".to_string()),
                alpha_id: Some("alpha-momentum".to_string()),
                actor_id: Some("ops-1".to_string()),
                run_id: Some("run-a".to_string()),
                snapshot_id: None,
            },
        ];

        let scoped = select_causal_flow_scope(&events);
        assert!(!scoped.is_empty());
        assert!(
            scoped
                .iter()
                .all(|event| event.run_id.as_deref() == Some("run-a"))
        );
    }

    #[test]
    fn incident_highest_severity_preserves_degraded_without_critical() {
        let events = vec![IncidentTimelineEvent {
            event_id: "incident::pnl::run-a".to_string(),
            occurred_at: "2026-04-06T15:00:00Z".to_string(),
            stage: IncidentTimelineStage::Pnl,
            source: "incident.forensics.v1".to_string(),
            reason_code: "incident_ready".to_string(),
            correlation_id: "corr-a".to_string(),
            summary: "degraded sample".to_string(),
            recommended_next_action: "inspect freshness".to_string(),
            severity: "degraded".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: Some("order-a".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-a".to_string()),
            snapshot_id: None,
        }];

        assert_eq!(incident_highest_severity(&events), "degraded");
    }

    #[tokio::test]
    async fn incident_forensics_route_returns_actionable_empty_state_for_no_match_windows() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/forensics?market_id=market-no-activity&start_ts=2025-01-01T00:00:00Z&end_ts=2030-01-01T00:00:00Z")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-incident-empty-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");

        assert_eq!(payload["data_state"], "empty");
        assert_eq!(payload["reason_code"], "incident_empty_window");
        assert!(payload["events"].as_array().expect("events should be array").is_empty());
        assert!(
            payload["recommended_next_action"]
                .as_str()
                .expect("recommended action should be string")
                .contains("Broaden filters")
        );
    }

    #[tokio::test]
    async fn incident_forensics_route_rejects_half_open_time_window() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/forensics?start_ts=2026-04-06T15:00:00Z")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-incident-invalid-window-001")
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

        assert_eq!(payload["error_code"], "incident_invalid_payload");
        assert_eq!(payload["action"], "incident_query");
        assert!(payload["field_errors"]
            .as_array()
            .expect("field_errors should be array")
            .iter()
            .any(|item| item["field"] == "start_ts"));
        assert!(payload["field_errors"]
            .as_array()
            .expect("field_errors should be array")
            .iter()
            .any(|item| item["field"] == "end_ts"));
    }

    #[tokio::test]
    async fn incident_forensics_route_surfaces_dependency_unavailable_machine_error() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/forensics?start_ts=2025-01-01T00:00:00Z&end_ts=2030-01-01T00:00:00Z&dependency_state=dependency_unavailable")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-incident-dependency-001")
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
        assert_eq!(payload["error_code"], "incident_dependency_unavailable");
        assert_eq!(payload["reason_code"], "incident_dependency_unavailable");
        assert_eq!(payload["action"], "incident_query");
    }

    #[tokio::test]
    async fn incident_forensics_route_returns_incident_unauthorized_for_denied_reads() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/forensics?start_ts=2025-01-01T00:00:00Z&end_ts=2030-01-01T00:00:00Z")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-incident-denied-001")
                    .body(Body::empty())
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
        assert_eq!(payload["error_code"], "incident_unauthorized");
        assert_eq!(payload["action"], "incident_query");
    }

    #[tokio::test]
    async fn emergency_pause_route_returns_machine_readable_accepted_evidence() {
        let app = test_app_with_safety_control_orchestrator(Arc::new(
            StubSafetyControlOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/pause")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-pause-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"audit_reference":"ticket-123"}"#))
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
        assert_eq!(payload["action"], "pause");
        assert_eq!(payload["source"], "manual");
        assert_eq!(payload["trigger_source"], "operator_command");
        assert_eq!(payload["resulting_mode"], "paused");
        assert_eq!(payload["reason_code"], "emergency_control_pause_activated");
        assert_eq!(payload["actor_id"], "ops-1");
        assert_eq!(payload["actor_role"], "operational_control");
        assert_eq!(payload["audit_reference"], "ticket-123");
    }

    #[tokio::test]
    async fn emergency_reduce_only_route_returns_reduce_only_mode_evidence() {
        let app = test_app_with_safety_control_orchestrator(Arc::new(
            StubSafetyControlOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/reduce-only")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-reduce-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"audit_reference":"ticket-456"}"#))
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

        assert_eq!(payload["action"], "reduce_only");
        assert_eq!(payload["resulting_mode"], "reduce_only");
        assert_eq!(
            payload["reason_code"],
            "emergency_control_reduce_only_activated"
        );
    }

    #[tokio::test]
    async fn emergency_pause_route_reuses_authorization_guard_for_unauthorized_role() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/pause")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-authz-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"audit_reference":"ticket-unauthorized"}"#))
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
    async fn emergency_pause_route_surfaces_malformed_payload_machine_error() {
        let app =
            test_app_with_safety_control_orchestrator(Arc::new(StubSafetyControlOrchestrator {
                manual_error: Some((
                    EmergencyControlReasonCode::InvalidPayload.code(),
                    "request payload failed validation",
                )),
                ..Default::default()
            }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/pause")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-invalid-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"audit_reference":"ticket-invalid"}"#))
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
        assert_eq!(payload["error_code"], "emergency_control_invalid_payload");
        assert_eq!(payload["action"], "emergency_control_pause");
    }

    #[tokio::test]
    async fn emergency_cancel_all_route_surfaces_service_unavailable_machine_error() {
        let app =
            test_app_with_safety_control_orchestrator(Arc::new(StubSafetyControlOrchestrator {
                manual_error: Some((
                    EmergencyControlReasonCode::OrchestrationUnavailable.code(),
                    "execution containment unavailable",
                )),
                ..Default::default()
            }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/cancel-all")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-cancel-failure-001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"audit_reference":"ticket-789"}"#))
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
        assert_eq!(
            payload["error_code"],
            "emergency_control_orchestration_unavailable"
        );
        assert_eq!(payload["action"], "emergency_control_cancel_all");
    }

    #[test]
    fn emergency_constraint_violations_map_to_conflict_status() {
        assert_eq!(
            emergency_control_service_error_status("safety_control_constraint_violation"),
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn emergency_action_query_route_returns_record_and_machine_readable_fields() {
        let app = test_app_with_safety_control_orchestrator(Arc::new(
            StubSafetyControlOrchestrator::default(),
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/actions/action::stale-feed")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-query-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["action_id"], "action::stale-feed");
        assert_eq!(payload["source"], "automatic");
        assert_eq!(payload["trigger_source"], "stale_feed");
        assert_eq!(payload["resulting_mode"], "paused");
        assert_eq!(
            payload["reason_code"],
            "emergency_control_stale_feed_triggered"
        );
    }

    #[tokio::test]
    async fn emergency_action_query_route_surfaces_not_found_machine_error() {
        let app =
            test_app_with_safety_control_orchestrator(Arc::new(StubSafetyControlOrchestrator {
                query_error: Some((
                    EmergencyControlReasonCode::NotFound.code(),
                    "action result not found",
                )),
                ..Default::default()
            }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/emergency/actions/action::missing")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-emergency-query-missing-001")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(
            payload["error_code"],
            EmergencyControlReasonCode::NotFound.code()
        );
        assert_eq!(payload["action"], "emergency_control_action_query");
        assert_eq!(
            payload["endpoint"],
            "/control/emergency/actions/action::missing"
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
