use crate::middleware::{
    AuthenticatedActor, ControlApiState, audit_append_failure_status, require_authenticated_actor,
};
use axum::{
    Router,
    extract::{Extension, Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    middleware as axum_middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::alerts::{
    AlertContractError, AlertDeliveryAttempt, AlertDeliveryChannel, AlertDeliveryOutcome,
    AlertDispatchStatus, AlertReasonCode, AlertSeverity, AlertTriggerDecision, AlertTriggerInput,
    AlertValidationIssue, IncidentAlert, compose_alert_identifier, critical_dispatch_within_sla,
    evaluate_fr29_trigger, normalize_alert_identifier, should_emit_alert,
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
use domain::recovery::RecoveryReasonCode;
use domain::recovery_rehearsal::{
    BackupIntegrityCheckItem, RestoreRehearsalRunEvidence, RestoreRehearsalStatus,
};
use domain::reporting_export::{
    ExportArtifactRecord, ExportJobRecord, ReportingExportReasonCode,
    ReportingExportValidationIssue, parse_utc_timestamp as parse_export_utc_timestamp,
};
use domain::reporting_schedule::{
    ReportRunRecord, ReportingScheduleReasonCode, ReportingScheduleValidationIssue,
    parse_utc_timestamp,
};
use domain::risk::{
    EmergencyControlAction, EmergencyControlReasonCode, MarketPolicyReasonCode,
    MarketPolicyValidationIssue, MarketSnapshot, ParticipationGuardrailReasonCode,
    RegimeShiftReasonCode, RegimeShiftThresholds, RewardRiskReasonCode, RewardRiskValidationIssue,
    RiskLimitReasonCode, RiskLimitScope, RiskLimitValidationIssue, SafetyControlActionRecord,
    VenueEligibilityState, evaluate_fr40_regime_shift,
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
use governance_service::recovery::{
    EvaluateRecoveryReadinessInput, ExecuteRecoveryResumeInput, ExecuteRestoreRehearsalInput,
    QueryRecoveryGateRunInput, QueryRestoreRehearsalByRunIdInput, QueryRestoreRehearsalsInput,
    RecoveryResumeExecutionEvidence,
};
use governance_service::reward_risk::{
    ReadRewardRiskPolicyInput, RewardRiskPolicyEvidence, UpsertRewardRiskPolicyInput,
};
use governance_service::risk_limits::{
    PendingRiskLimitProfilesInput, RiskLimitProfileMutationEvidence, RiskLimitRuleInput,
    UpsertRiskLimitProfileInput,
};
use governance_service::safety_controls::ExecuteManualSafetyControlInput;
use persistence::postgres::attribution_snapshots::load_latest_attribution_snapshots;
use persistence::postgres::incident_alerts::{
    append_alert_delivery_attempt, create_incident_alert, load_alert_delivery_attempts,
    load_recent_incident_alerts, update_incident_alert_status,
};
use persistence::postgres::incident_query_views::load_incident_forensics_timeline;
use persistence::postgres::participation_guardrail_events::load_participation_guardrail_events;
use persistence::postgres::regime_shift_alerts::{
    RegimeShiftAlertRecord, create_regime_shift_alert, load_regime_shift_alerts,
};
use reporting_service::exports::scheduling::{
    PauseReportScheduleInput, QueryReportRunHistoryInput, ReportScheduleMutationEvidence,
    ResumeReportScheduleInput, UpsertReportScheduleInput,
};
use reporting_service::exports::workflows::{
    GetExportArtifactInput, ListExportArtifactsInput, QueryExportJobInput, ReportExportJobEvidence,
    TriggerIncidentExportInput, TriggerOnDemandExportInput,
};
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
    let reward_risk_routes = Router::new()
        .route(
            "/control/reward-risk/policies/{policy_key}",
            post(upsert_reward_risk_policy).get(read_reward_risk_policy),
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
        .route(
            "/control/portfolio/attribution",
            get(read_portfolio_attribution),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let incident_forensics_routes = Router::new()
        .route("/control/incidents/forensics", get(read_incident_forensics))
        .route("/control/incidents/alerts", get(list_incident_alerts))
        .route(
            "/control/incidents/alerts/dispatch",
            post(dispatch_incident_alert),
        )
        .route(
            "/control/incidents/regime-shifts",
            get(list_regime_shift_alerts),
        )
        .route(
            "/control/incidents/participation-guardrails",
            get(list_participation_guardrail_events),
        )
        .route(
            "/control/incidents/regime-shifts/dispatch",
            post(dispatch_regime_shift_alerts),
        )
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
    let recovery_routes = Router::new()
        .route(
            "/control/recovery/readiness/evaluate",
            post(evaluate_recovery_readiness),
        )
        .route("/control/recovery/resume", post(execute_recovery_resume))
        .route(
            "/control/recovery/rehearsals",
            post(execute_restore_rehearsal).get(query_restore_rehearsals),
        )
        .route(
            "/control/recovery/rehearsals/{run_id}",
            get(query_restore_rehearsal_by_run_id),
        )
        .route(
            "/control/recovery/runs/{run_id}",
            get(query_recovery_gate_run_by_run_id),
        )
        .route("/control/recovery/runs", get(query_recovery_gate_run))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let report_schedule_routes = Router::new()
        .route(
            "/control/report-schedules/{schedule_id}",
            post(upsert_report_schedule),
        )
        .route(
            "/control/report-schedules/{schedule_id}/pause",
            post(pause_report_schedule),
        )
        .route(
            "/control/report-schedules/{schedule_id}/resume",
            post(resume_report_schedule),
        )
        .route(
            "/control/report-schedules/{schedule_id}/runs",
            get(query_report_schedule_runs),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_authenticated_actor,
        ));
    let report_export_routes = Router::new()
        .route(
            "/control/report-exports/on-demand",
            post(trigger_on_demand_report_export),
        )
        .route(
            "/control/report-exports/incidents/{incident_id}",
            post(trigger_incident_report_export),
        )
        .route(
            "/control/report-exports/{job_id}",
            get(query_report_export_job),
        )
        .route(
            "/control/report-exports/{job_id}/artifacts",
            get(list_report_export_artifacts),
        )
        .route(
            "/control/report-exports/{job_id}/artifacts/{artifact_id}",
            get(read_report_export_artifact),
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
        .merge(reward_risk_routes)
        .merge(allocation_policy_routes)
        .merge(attribution_routes)
        .merge(incident_forensics_routes)
        .merge(emergency_control_routes)
        .merge(recovery_routes)
        .merge(report_schedule_routes)
        .merge(report_export_routes)
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
    if payload.require_execution
        && let Some(request_id) = approval_request_id
    {
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

pub async fn upsert_reward_risk_policy(
    State(state): State<ControlApiState>,
    Path(policy_key): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<RewardRiskPolicyPayload>,
) -> Response {
    let endpoint = format!("/control/reward-risk/policies/{policy_key}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let evidence = match state.reward_risk_orchestrator.upsert_reward_risk_policy(
        UpsertRewardRiskPolicyInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            policy_key,
            strategy_key: payload.strategy_key,
            min_reward_per_risk: payload.min_reward_per_risk,
            correlation_id: actor.correlation_id.clone(),
            updated_at_utc: authorization.timestamp_utc.clone(),
        },
    ) {
        Ok(evidence) => evidence,
        Err(error) => {
            return reward_risk_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "reward_risk_policy_upsert",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    reward_risk_policy_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "POST",
        "reward_risk_policy_upsert",
    )
}

pub async fn read_reward_risk_policy(
    State(state): State<ControlApiState>,
    Path(policy_key): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/reward-risk/policies/{policy_key}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let evidence =
        match state
            .reward_risk_orchestrator
            .read_reward_risk_policy(ReadRewardRiskPolicyInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                policy_key,
                correlation_id: actor.correlation_id.clone(),
                queried_at_utc: authorization.timestamp_utc.clone(),
            }) {
            Ok(evidence) => evidence,
            Err(error) => {
                return reward_risk_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "reward_risk_policy_read",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };

    reward_risk_policy_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "GET",
        "reward_risk_policy_read",
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
    let dependency_state =
        match parse_attribution_dependency_state(query.dependency_state.as_deref()) {
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

    let dependency_state = match parse_incident_dependency_state(query.dependency_state.as_deref())
    {
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
        let synthetic =
            synthetic_incident_timeline(&authorization.timestamp_utc, &actor.correlation_id);
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

pub async fn list_incident_alerts(
    State(state): State<ControlApiState>,
    Query(query): Query<IncidentAlertsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/incidents/alerts".to_string();
    let authorization = match authorize_incident_alert_action(
        &state,
        &actor,
        &endpoint,
        "GET",
        "incident_alerts_query",
    ) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let limit = match parse_alert_limit(query.limit) {
        Ok(limit) => limit,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let dependency_state = match parse_incident_dependency_state(query.dependency_state.as_deref())
    {
        Ok(state) => state,
        Err(error) => {
            return incident_alert_service_error_response(
                AlertReasonCode::InvalidPayload.code(),
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "incident_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    match dependency_state {
        IncidentDependencyState::DependencyUnavailable => {
            return incident_alert_service_error_response(
                AlertReasonCode::DependencyUnavailable.code(),
                "incident alert dependencies are unavailable".to_string(),
                Vec::new(),
                "incident_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::StaleEvidence => {
            return incident_alert_service_error_response(
                AlertReasonCode::StaleEvidence.code(),
                "incident alert evidence is stale".to_string(),
                Vec::new(),
                "incident_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::Healthy => {}
    }

    let mut alerts = if let Some(pool) = state.attribution_pool.as_ref() {
        match load_recent_incident_alerts(pool, limit).await {
            Ok(alerts) => alerts,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "incident_alerts_query",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
    } else {
        synthetic_incident_alerts(&authorization.timestamp_utc, &actor.correlation_id)
    };

    let mut alert_items = Vec::new();
    for alert in alerts.drain(..) {
        let attempts = if let Some(pool) = state.attribution_pool.as_ref() {
            match load_alert_delivery_attempts(pool, &alert.alert_id, 16).await {
                Ok(attempts) => attempts,
                Err(error) => {
                    return incident_alert_service_error_response(
                        error.code,
                        error.message,
                        error.field_errors,
                        "incident_alerts_query",
                        &actor,
                        authorization.timestamp_utc.clone(),
                        endpoint,
                    );
                }
            }
        } else {
            synthetic_alert_delivery_attempts(&alert)
        };

        alert_items.push(IncidentAlertItem {
            alert_id: alert.alert_id,
            severity: alert.severity.as_str().to_string(),
            impacted_subsystem: alert.impacted_subsystem,
            cause: alert.cause,
            recommended_next_action: alert.recommended_next_action,
            evidence_link: alert.evidence_link,
            issued_at: alert.issued_at,
            correlation_id: alert.correlation_id,
            reason_code: alert.reason_code,
            status: alert.status.as_str().to_string(),
            delivered_at: alert.delivered_at,
            failed_at: alert.failed_at,
            attempts: attempts
                .into_iter()
                .map(|attempt| IncidentAlertDeliveryAttemptItem {
                    attempt_number: i64::from(attempt.attempt_number),
                    channel: attempt.channel.as_str().to_string(),
                    outcome: attempt.outcome.as_str().to_string(),
                    reason_code: attempt.reason_code,
                    attempted_at: attempt.attempted_at,
                    delivered_at: attempt.delivered_at,
                    failed_at: attempt.failed_at,
                })
                .collect(),
        });
    }

    incident_alert_query_response(
        &state,
        &actor,
        alert_items,
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

pub async fn dispatch_incident_alert(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<IncidentAlertDispatchPayload>,
) -> Response {
    let endpoint = "/control/incidents/alerts/dispatch".to_string();
    let authorization = match authorize_incident_alert_action(
        &state,
        &actor,
        &endpoint,
        "POST",
        "incident_alert_dispatch",
    ) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let dependency_state =
        match parse_incident_dependency_state(payload.dependency_state.as_deref()) {
            Ok(state) => state,
            Err(error) => {
                return incident_alert_service_error_response(
                    AlertReasonCode::InvalidPayload.code(),
                    error.message,
                    error
                        .field_errors
                        .into_iter()
                        .map(|issue| AlertValidationIssue {
                            field: issue.field,
                            code: issue.code,
                            message: issue.message,
                        })
                        .collect(),
                    "incident_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };

    match dependency_state {
        IncidentDependencyState::DependencyUnavailable => {
            return incident_alert_service_error_response(
                AlertReasonCode::DependencyUnavailable.code(),
                "incident alert dispatch dependencies are unavailable".to_string(),
                Vec::new(),
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::StaleEvidence => {
            return incident_alert_service_error_response(
                AlertReasonCode::StaleEvidence.code(),
                "incident alert dispatch evidence is stale".to_string(),
                Vec::new(),
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::Healthy => {}
    }

    let correlation_id = payload
        .correlation_id
        .as_deref()
        .map(normalize_alert_identifier)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_alert_identifier(&actor.correlation_id));
    let observed_at = payload
        .observed_at
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let trigger_input = AlertTriggerInput {
        drawdown_pct_of_daily_limit: payload.drawdown_pct_of_daily_limit.unwrap_or(0.0),
        stream_disconnect_seconds: payload.stream_disconnect_seconds.unwrap_or(0),
        reconciliation_lag_seconds: payload.reconciliation_lag_seconds.unwrap_or(0),
        stale_data_detected: payload.stale_data_detected.unwrap_or(false),
        policy_bypass_attempt: payload.policy_bypass_attempt.unwrap_or(false),
        correlation_id: correlation_id.clone(),
        observed_at,
    };
    let trigger_decision = match evaluate_fr29_trigger(&trigger_input) {
        Ok(Some(decision)) => decision,
        Ok(None) => {
            return incident_alert_service_error_response(
                AlertReasonCode::NoTrigger.code(),
                "no FR29 trigger threshold was breached; alert dispatch not executed".to_string(),
                Vec::new(),
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let issued_at = payload
        .issued_at
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let reason_code = trigger_decision.reason_code;
    let alert_id = match compose_alert_identifier(&correlation_id, reason_code, &issued_at) {
        Ok(identifier) => identifier,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    let effective_decision = AlertTriggerDecision {
        severity: trigger_decision.severity,
        reason_code: trigger_decision.reason_code,
        impacted_subsystem: payload
            .impacted_subsystem
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(trigger_decision.impacted_subsystem.as_str())
            .to_string(),
        cause: payload
            .cause
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(trigger_decision.cause.as_str())
            .to_string(),
        recommended_next_action: payload
            .recommended_next_action
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(trigger_decision.recommended_next_action.as_str())
            .to_string(),
    };

    let evidence_link = payload
        .evidence_link
        .as_deref()
        .unwrap_or("https://docs.example.com/operations/severity-alert-delivery")
        .to_string();
    let mut alert = match domain::alerts::build_incident_alert(
        &effective_decision,
        &alert_id,
        &issued_at,
        &correlation_id,
        &evidence_link,
        Some(&effective_decision.recommended_next_action),
    ) {
        Ok(alert) => alert,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };
    alert.cause = effective_decision.cause.clone();

    let dedupe_window_seconds = payload.dedupe_window_seconds.unwrap_or(300);
    let prior_alerts = if let Some(pool) = state.attribution_pool.as_ref() {
        match load_recent_incident_alerts(pool, 200).await {
            Ok(alerts) => alerts,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "incident_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
    } else {
        Vec::new()
    };

    let should_emit = match should_emit_alert(
        &prior_alerts,
        alert.reason_code.as_str(),
        &alert.correlation_id,
        &alert.issued_at,
        dedupe_window_seconds,
    ) {
        Ok(value) => value,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };
    if !should_emit {
        return incident_alert_service_error_response(
            AlertReasonCode::DuplicateSuppressed.code(),
            "duplicate alert suppressed for this correlation and dedupe window".to_string(),
            Vec::new(),
            "incident_alert_dispatch",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    }

    if let Some(pool) = state.attribution_pool.as_ref() {
        if let Err(error) = create_incident_alert(pool, &alert).await {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    }

    let simulation = match simulate_alert_dispatch(&alert, &payload) {
        Ok(simulation) => simulation,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    if let Some(pool) = state.attribution_pool.as_ref() {
        for attempt in &simulation.attempts {
            if let Err(error) = append_alert_delivery_attempt(pool, attempt).await {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "incident_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
        if let Err(error) = update_incident_alert_status(
            pool,
            &simulation.alert.alert_id,
            simulation.alert.status,
            &simulation.alert.reason_code,
            simulation.alert.delivered_at.as_deref(),
            simulation.alert.failed_at.as_deref(),
        )
        .await
        {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "incident_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    }

    if let Some((failure_code, failure_message)) = simulation.failure {
        return incident_alert_service_error_response(
            failure_code.code(),
            failure_message,
            Vec::new(),
            "incident_alert_dispatch",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    }

    incident_alert_dispatch_response(
        &state,
        &actor,
        simulation,
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

pub async fn list_regime_shift_alerts(
    State(state): State<ControlApiState>,
    Query(query): Query<RegimeShiftAlertsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/incidents/regime-shifts".to_string();
    let authorization = match authorize_incident_alert_action(
        &state,
        &actor,
        &endpoint,
        "GET",
        "regime_shift_alerts_query",
    ) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let limit = match parse_alert_limit(query.limit) {
        Ok(limit) => limit,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "regime_shift_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };
    let dependency_state = match parse_incident_dependency_state(query.dependency_state.as_deref())
    {
        Ok(state) => state,
        Err(error) => {
            return incident_alert_service_error_response(
                AlertReasonCode::InvalidPayload.code(),
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "regime_shift_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    match dependency_state {
        IncidentDependencyState::DependencyUnavailable => {
            return incident_alert_service_error_response(
                AlertReasonCode::DependencyUnavailable.code(),
                "regime-shift alert dependencies are unavailable".to_string(),
                Vec::new(),
                "regime_shift_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::StaleEvidence => {
            return incident_alert_service_error_response(
                AlertReasonCode::StaleEvidence.code(),
                "regime-shift alert evidence is stale".to_string(),
                Vec::new(),
                "regime_shift_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::Healthy => {}
    }

    let Some(pool) = state.attribution_pool.as_ref() else {
        return incident_alert_service_error_response(
            RegimeShiftReasonCode::DependencyUnavailable.code(),
            "regime-shift evidence persistence dependency is unavailable".to_string(),
            Vec::new(),
            "regime_shift_alerts_query",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    };
    let alerts = match load_regime_shift_alerts(
        pool,
        query.market_id.as_deref(),
        query.reason_code.as_deref(),
        query.correlation_id.as_deref(),
        query.start_ts.as_deref(),
        query.end_ts.as_deref(),
        limit,
    )
    .await
    {
        Ok(alerts) => alerts
            .into_iter()
            .map(to_regime_shift_alert_item)
            .collect::<Vec<_>>(),
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "regime_shift_alerts_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    regime_shift_alert_query_response(
        &state,
        &actor,
        alerts,
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

pub async fn list_participation_guardrail_events(
    State(state): State<ControlApiState>,
    Query(query): Query<ParticipationGuardrailEventsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/incidents/participation-guardrails".to_string();
    let authorization = match authorize_incident_alert_action(
        &state,
        &actor,
        &endpoint,
        "GET",
        "participation_guardrail_events_query",
    ) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let limit = match parse_alert_limit(query.limit) {
        Ok(limit) => limit,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "participation_guardrail_events_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };
    let dependency_state = match parse_incident_dependency_state(query.dependency_state.as_deref())
    {
        Ok(state) => state,
        Err(error) => {
            return incident_alert_service_error_response(
                AlertReasonCode::InvalidPayload.code(),
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "participation_guardrail_events_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    match dependency_state {
        IncidentDependencyState::DependencyUnavailable => {
            return incident_alert_service_error_response(
                AlertReasonCode::DependencyUnavailable.code(),
                "participation guardrail dependencies are unavailable".to_string(),
                Vec::new(),
                "participation_guardrail_events_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::StaleEvidence => {
            return incident_alert_service_error_response(
                AlertReasonCode::StaleEvidence.code(),
                "participation guardrail evidence is stale".to_string(),
                Vec::new(),
                "participation_guardrail_events_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::Healthy => {}
    }

    let Some(pool) = state.attribution_pool.as_ref() else {
        return incident_alert_service_error_response(
            ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
            "participation guardrail evidence persistence dependency is unavailable".to_string(),
            Vec::new(),
            "participation_guardrail_events_query",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    };
    let events = match load_participation_guardrail_events(
        pool,
        query.market_id.as_deref(),
        query.reason_code.as_deref(),
        query.correlation_id.as_deref(),
        query.start_ts.as_deref(),
        query.end_ts.as_deref(),
        limit,
    )
    .await
    {
        Ok(events) => events
            .into_iter()
            .map(to_participation_guardrail_event_item)
            .collect::<Vec<_>>(),
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "participation_guardrail_events_query",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };

    participation_guardrail_query_response(
        &state,
        &actor,
        events,
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

pub async fn dispatch_regime_shift_alerts(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<RegimeShiftAlertDispatchPayload>,
) -> Response {
    let endpoint = "/control/incidents/regime-shifts/dispatch".to_string();
    let authorization = match authorize_incident_alert_action(
        &state,
        &actor,
        &endpoint,
        "POST",
        "regime_shift_alert_dispatch",
    ) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let dependency_state =
        match parse_incident_dependency_state(payload.dependency_state.as_deref()) {
            Ok(state) => state,
            Err(error) => {
                return incident_alert_service_error_response(
                    AlertReasonCode::InvalidPayload.code(),
                    error.message,
                    error
                        .field_errors
                        .into_iter()
                        .map(|issue| AlertValidationIssue {
                            field: issue.field,
                            code: issue.code,
                            message: issue.message,
                        })
                        .collect(),
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
    match dependency_state {
        IncidentDependencyState::DependencyUnavailable => {
            return incident_alert_service_error_response(
                AlertReasonCode::DependencyUnavailable.code(),
                "regime-shift alert dispatch dependencies are unavailable".to_string(),
                Vec::new(),
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::StaleEvidence => {
            return incident_alert_service_error_response(
                AlertReasonCode::StaleEvidence.code(),
                "regime-shift alert dispatch evidence is stale".to_string(),
                Vec::new(),
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        IncidentDependencyState::Healthy => {}
    }

    let base_correlation_id = payload
        .correlation_id
        .as_deref()
        .map(normalize_alert_identifier)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_alert_identifier(&actor.correlation_id));
    let observed_at = payload
        .observed_at
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let issued_at = payload
        .issued_at
        .clone()
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let market_id = payload.market_id.trim().to_ascii_lowercase();
    let cluster_id = payload.cluster_id.trim().to_ascii_lowercase();
    let scoped_correlation =
        normalize_alert_identifier(&format!("{base_correlation_id}:{market_id}:{cluster_id}"));
    let correlation_id = if scoped_correlation.len() <= 120 {
        scoped_correlation
    } else {
        base_correlation_id
    };

    let previous_eligibility_state =
        match VenueEligibilityState::parse(&payload.previous_eligibility_state) {
            Ok(state) => state,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error
                        .field_errors
                        .into_iter()
                        .map(|issue| AlertValidationIssue {
                            field: issue.field,
                            code: issue.code,
                            message: issue.message,
                        })
                        .collect(),
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
    let current_eligibility_state =
        match VenueEligibilityState::parse(&payload.current_eligibility_state) {
            Ok(state) => state,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error
                        .field_errors
                        .into_iter()
                        .map(|issue| AlertValidationIssue {
                            field: issue.field,
                            code: issue.code,
                            message: issue.message,
                        })
                        .collect(),
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };

    let previous_snapshot = MarketSnapshot {
        market_id: market_id.clone(),
        cluster_id: cluster_id.clone(),
        liquidity_depth_usd: payload.previous_liquidity_depth_usd.unwrap_or(0.0),
        inactivity_gap_seconds: None,
        spread_bps: payload.previous_spread_bps,
        reward_score: 0.0,
        expected_reward_bps: None,
        maker_rebate_bps: Some(payload.previous_maker_rebate_bps),
        expected_cost_bps: None,
        expected_volatility_bps: None,
        venue_eligibility_state: Some(previous_eligibility_state),
        projected_exposure_pct_nav: payload.previous_projected_exposure_pct_nav.unwrap_or(0.0),
        observed_at_utc: payload
            .previous_observed_at
            .clone()
            .unwrap_or_else(|| observed_at.clone()),
    };
    let current_snapshot = MarketSnapshot {
        market_id: market_id.clone(),
        cluster_id: cluster_id.clone(),
        liquidity_depth_usd: payload.current_liquidity_depth_usd.unwrap_or(0.0),
        inactivity_gap_seconds: None,
        spread_bps: payload.current_spread_bps,
        reward_score: 0.0,
        expected_reward_bps: None,
        maker_rebate_bps: Some(payload.current_maker_rebate_bps),
        expected_cost_bps: None,
        expected_volatility_bps: None,
        venue_eligibility_state: Some(current_eligibility_state),
        projected_exposure_pct_nav: payload.current_projected_exposure_pct_nav.unwrap_or(0.0),
        observed_at_utc: observed_at.clone(),
    };
    let thresholds = RegimeShiftThresholds {
        rebate_delta_bps: payload.rebate_delta_threshold_bps.unwrap_or(20.0),
        spread_widening_bps: payload.spread_widening_threshold_bps.unwrap_or(50.0),
    };
    let detections = match evaluate_fr40_regime_shift(
        &previous_snapshot,
        &current_snapshot,
        &correlation_id,
        Some(&thresholds),
    ) {
        Ok(detections) => detections,
        Err(error) => {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    };
    if detections.is_empty() {
        return incident_alert_service_error_response(
            AlertReasonCode::NoTrigger.code(),
            "no FR40 regime-shift threshold was breached; alert dispatch not executed".to_string(),
            Vec::new(),
            "regime_shift_alert_dispatch",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    }
    let Some(pool) = state.attribution_pool.as_ref() else {
        return incident_alert_service_error_response(
            RegimeShiftReasonCode::DependencyUnavailable.code(),
            "regime-shift alert persistence dependency is unavailable".to_string(),
            Vec::new(),
            "regime_shift_alert_dispatch",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    };

    let simulation_payload = IncidentAlertDispatchPayload {
        correlation_id: Some(correlation_id.clone()),
        issued_at: Some(issued_at.clone()),
        observed_at: Some(observed_at.clone()),
        impacted_subsystem: None,
        cause: None,
        recommended_next_action: payload.recommended_next_action.clone(),
        evidence_link: payload.evidence_link.clone(),
        drawdown_pct_of_daily_limit: None,
        stream_disconnect_seconds: None,
        reconciliation_lag_seconds: None,
        stale_data_detected: None,
        policy_bypass_attempt: None,
        simulate_primary_failure: payload.simulate_primary_failure,
        simulate_fallback_failure: payload.simulate_fallback_failure,
        simulated_delivery_delay_seconds: payload.simulated_delivery_delay_seconds,
        dedupe_window_seconds: payload.dedupe_window_seconds,
        primary_channel: payload.primary_channel.clone(),
        fallback_channel: payload.fallback_channel.clone(),
        dependency_state: payload.dependency_state.clone(),
    };
    let dedupe_window_seconds = payload.dedupe_window_seconds.unwrap_or(300);
    let mut dispatched = Vec::new();
    let mut duplicate_suppressed_count = 0usize;
    for detection in detections {
        let alert_reason = match map_regime_reason_to_alert_reason(&detection.reason_code) {
            Ok(reason) => reason,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        let effective_decision = AlertTriggerDecision {
            severity: AlertSeverity::Critical,
            reason_code: alert_reason,
            impacted_subsystem: "market-economics".to_string(),
            cause: regime_shift_cause(&detection),
            recommended_next_action: payload
                .recommended_next_action
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| regime_shift_recommended_action(alert_reason).to_string()),
        };
        let alert_id = match compose_alert_identifier(&correlation_id, alert_reason, &issued_at) {
            Ok(identifier) => identifier,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        let evidence_link = payload
            .evidence_link
            .as_deref()
            .unwrap_or("https://docs.example.com/operations/incentive-regime-shift-alerts")
            .to_string();
        let mut alert = match domain::alerts::build_incident_alert(
            &effective_decision,
            &alert_id,
            &issued_at,
            &correlation_id,
            &evidence_link,
            Some(&effective_decision.recommended_next_action),
        ) {
            Ok(alert) => alert,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        alert.cause = effective_decision.cause.clone();

        let prior_alerts = match load_recent_incident_alerts(pool, 200).await {
            Ok(alerts) => alerts,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        let should_emit = match should_emit_alert(
            &prior_alerts,
            alert.reason_code.as_str(),
            &alert.correlation_id,
            &alert.issued_at,
            dedupe_window_seconds,
        ) {
            Ok(value) => value,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        if !should_emit {
            duplicate_suppressed_count += 1;
            continue;
        }
        let simulation = match simulate_alert_dispatch(&alert, &simulation_payload) {
            Ok(simulation) => simulation,
            Err(error) => {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        let record = RegimeShiftAlertRecord {
            alert_id: simulation.alert.alert_id.clone(),
            market_id: detection.market_id.clone(),
            cluster_id: detection.cluster_id.clone(),
            reason_code: detection.reason_code.clone(),
            severity: AlertSeverity::Critical,
            correlation_id: simulation.alert.correlation_id.clone(),
            observed_at: detection.observed_at_utc.clone(),
            issued_at: simulation.alert.issued_at.clone(),
            dispatch_status: simulation.alert.status,
            dispatch_reason_code: simulation.alert.reason_code.clone(),
            recommended_next_action: simulation.alert.recommended_next_action.clone(),
            evidence_link: simulation.alert.evidence_link.clone(),
            previous_maker_rebate_bps: detection.previous_maker_rebate_bps,
            current_maker_rebate_bps: detection.current_maker_rebate_bps,
            rebate_delta_bps: detection.rebate_delta_bps,
            previous_spread_bps: detection.previous_spread_bps,
            current_spread_bps: detection.current_spread_bps,
            spread_widening_bps: detection.spread_widening_bps,
            previous_eligibility_state: detection.previous_eligibility_state,
            current_eligibility_state: detection.current_eligibility_state,
            threshold_rebate_delta_bps: detection.threshold_rebate_delta_bps,
            threshold_spread_widening_bps: detection.threshold_spread_widening_bps,
        };
        let mut transaction = match pool.begin().await {
            Ok(transaction) => transaction,
            Err(error) => {
                return incident_alert_service_error_response(
                    RegimeShiftReasonCode::PersistenceUnavailable.code(),
                    format!("unable to begin regime-shift persistence transaction: {error}"),
                    Vec::new(),
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        };
        if let Err(error) = create_incident_alert(&mut *transaction, &alert).await {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        for attempt in &simulation.attempts {
            if let Err(error) = append_alert_delivery_attempt(&mut *transaction, attempt).await {
                return incident_alert_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "regime_shift_alert_dispatch",
                    &actor,
                    authorization.timestamp_utc.clone(),
                    endpoint,
                );
            }
        }
        if let Err(error) = update_incident_alert_status(
            &mut *transaction,
            &simulation.alert.alert_id,
            simulation.alert.status,
            &simulation.alert.reason_code,
            simulation.alert.delivered_at.as_deref(),
            simulation.alert.failed_at.as_deref(),
        )
        .await
        {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        if let Err(error) = create_regime_shift_alert(&mut *transaction, &record).await {
            return incident_alert_service_error_response(
                error.code,
                error.message,
                error
                    .field_errors
                    .into_iter()
                    .map(|issue| AlertValidationIssue {
                        field: issue.field,
                        code: issue.code,
                        message: issue.message,
                    })
                    .collect(),
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        if let Err(error) = transaction.commit().await {
            return incident_alert_service_error_response(
                RegimeShiftReasonCode::PersistenceUnavailable.code(),
                format!("unable to commit regime-shift persistence transaction: {error}"),
                Vec::new(),
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
        if let Some((failure_code, failure_message)) = simulation.failure.clone() {
            return incident_alert_service_error_response(
                failure_code.code(),
                failure_message,
                Vec::new(),
                "regime_shift_alert_dispatch",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }

        dispatched.push(to_regime_shift_alert_dispatch_item(
            &detection,
            &simulation,
            &effective_decision,
        ));
    }

    if dispatched.is_empty() && duplicate_suppressed_count > 0 {
        return incident_alert_service_error_response(
            AlertReasonCode::DuplicateSuppressed.code(),
            "duplicate regime-shift alerts were suppressed for this correlation and dedupe window"
                .to_string(),
            Vec::new(),
            "regime_shift_alert_dispatch",
            &actor,
            authorization.timestamp_utc.clone(),
            endpoint,
        );
    }

    regime_shift_alert_dispatch_response(
        &state,
        &actor,
        dispatched,
        duplicate_suppressed_count,
        authorization.timestamp_utc.clone(),
        endpoint,
    )
}

fn map_regime_reason_to_alert_reason(
    reason_code: &str,
) -> Result<AlertReasonCode, AlertContractError> {
    match RegimeShiftReasonCode::parse(reason_code) {
        Ok(RegimeShiftReasonCode::RebateDeltaExceeded) => {
            Ok(AlertReasonCode::RegimeRebateDeltaExceeded)
        }
        Ok(RegimeShiftReasonCode::SpreadWideningExceeded) => {
            Ok(AlertReasonCode::RegimeSpreadWideningExceeded)
        }
        Ok(RegimeShiftReasonCode::EligibilityTransition) => {
            Ok(AlertReasonCode::RegimeEligibilityTransition)
        }
        Ok(_) => Err(AlertContractError::invalid_payload_with_issues(
            format!("unsupported FR40 regime reason code `{reason_code}` for dispatch"),
            vec![AlertValidationIssue {
                field: "reason_code",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "reason_code must map to an FR40 dispatch reason".to_string(),
            }],
        )),
        Err(error) => Err(AlertContractError::invalid_payload_with_issues(
            error.message,
            vec![AlertValidationIssue {
                field: "reason_code",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "reason_code must be a supported FR40 regime-shift reason".to_string(),
            }],
        )),
    }
}

fn regime_shift_cause(detection: &domain::risk::RegimeShiftDetection) -> String {
    match RegimeShiftReasonCode::parse(&detection.reason_code) {
        Ok(RegimeShiftReasonCode::RebateDeltaExceeded) => format!(
            "Maker rebate delta exceeded FR40 threshold: delta={}bps (> {}bps).",
            detection.rebate_delta_bps.unwrap_or(0.0),
            detection.threshold_rebate_delta_bps
        ),
        Ok(RegimeShiftReasonCode::SpreadWideningExceeded) => format!(
            "Spread widening exceeded FR40 threshold: widening={}bps (> {}bps).",
            detection.spread_widening_bps.unwrap_or(0.0),
            detection.threshold_spread_widening_bps
        ),
        Ok(RegimeShiftReasonCode::EligibilityTransition) => format!(
            "Venue eligibility shifted from `{}` to `{}`.",
            detection
                .previous_eligibility_state
                .map(|state| state.as_str())
                .unwrap_or("unknown"),
            detection
                .current_eligibility_state
                .map(|state| state.as_str())
                .unwrap_or("unknown")
        ),
        _ => "FR40 regime shift detected in venue economics.".to_string(),
    }
}

fn regime_shift_recommended_action(reason_code: AlertReasonCode) -> &'static str {
    match reason_code {
        AlertReasonCode::RegimeRebateDeltaExceeded => {
            "Review maker incentive assumptions and adjust participation weights before next cycle."
        }
        AlertReasonCode::RegimeSpreadWideningExceeded => {
            "Reduce aggressive participation, confirm liquidity quality, and reassess spread budgets."
        }
        AlertReasonCode::RegimeEligibilityTransition => {
            "Confirm venue eligibility constraints and move impacted markets into restricted handling."
        }
        _ => "Inspect FR40 evidence and execute the linked regime-shift runbook steps.",
    }
}

struct AlertDispatchSimulation {
    alert: IncidentAlert,
    attempts: Vec<AlertDeliveryAttempt>,
    fallback_used: bool,
    dispatch_latency_seconds: i64,
    failure: Option<(AlertReasonCode, String)>,
}

fn simulate_alert_dispatch(
    alert: &IncidentAlert,
    payload: &IncidentAlertDispatchPayload,
) -> Result<AlertDispatchSimulation, AlertContractError> {
    let primary_channel = payload.primary_channel.as_deref().unwrap_or("pagerduty");
    let primary_channel = AlertDeliveryChannel::parse(primary_channel)?;
    let fallback_channel = payload.fallback_channel.as_deref().unwrap_or("slack");
    let fallback_channel = AlertDeliveryChannel::parse(fallback_channel)?;
    let simulated_delay_seconds = payload.simulated_delivery_delay_seconds.unwrap_or(5);
    if simulated_delay_seconds < 0 {
        return Err(AlertContractError::invalid_payload_with_issues(
            "simulated_delivery_delay_seconds must be >= 0",
            vec![AlertValidationIssue {
                field: "simulated_delivery_delay_seconds",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "simulated_delivery_delay_seconds must be >= 0".to_string(),
            }],
        ));
    }

    let issued = OffsetDateTime::parse(&alert.issued_at, &Rfc3339).map_err(|_| {
        AlertContractError::invalid_payload_with_issues(
            "issued_at must be an RFC3339 UTC timestamp",
            vec![AlertValidationIssue {
                field: "issued_at",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "issued_at must be an RFC3339 UTC timestamp".to_string(),
            }],
        )
    })?;

    let primary_fails = payload.simulate_primary_failure.unwrap_or(false)
        || (alert.severity == AlertSeverity::Critical && simulated_delay_seconds > 30);
    let fallback_fails = payload.simulate_fallback_failure.unwrap_or(false);
    let primary_attempt_time = issued + Duration::seconds(1);
    let primary_attempt_time = format_timestamp(primary_attempt_time);

    let mut attempts = Vec::new();
    let mut outcome_alert = alert.clone();
    let mut fallback_used = false;
    let mut failure = None;

    if primary_fails {
        let primary_failure_code = if payload.simulate_primary_failure.unwrap_or(false) {
            AlertReasonCode::DeliveryPrimaryFailed
        } else {
            AlertReasonCode::DeliverySlaBreached
        };
        attempts.push(AlertDeliveryAttempt {
            alert_id: alert.alert_id.clone(),
            attempt_number: 1,
            channel: primary_channel,
            outcome: AlertDeliveryOutcome::Failed,
            reason_code: primary_failure_code.code().to_string(),
            correlation_id: alert.correlation_id.clone(),
            attempted_at: primary_attempt_time.clone(),
            delivered_at: None,
            failed_at: Some(primary_attempt_time),
        });
        fallback_used = true;

        let fallback_attempt_time = format_timestamp(issued + Duration::seconds(20));
        if fallback_fails {
            attempts.push(AlertDeliveryAttempt {
                alert_id: alert.alert_id.clone(),
                attempt_number: 2,
                channel: fallback_channel,
                outcome: AlertDeliveryOutcome::Failed,
                reason_code: AlertReasonCode::DeliveryFallbackFailed.code().to_string(),
                correlation_id: alert.correlation_id.clone(),
                attempted_at: fallback_attempt_time.clone(),
                delivered_at: None,
                failed_at: Some(fallback_attempt_time.clone()),
            });
            outcome_alert.status = AlertDispatchStatus::Failed;
            outcome_alert.failed_at = Some(fallback_attempt_time);
            outcome_alert.delivered_at = None;
            outcome_alert.reason_code = AlertReasonCode::DeliveryFallbackFailed.code().to_string();
            failure = Some((
                AlertReasonCode::DeliveryFallbackFailed,
                "primary channel failed and fallback delivery failed".to_string(),
            ));
        } else {
            let fallback_delivered_at = format_timestamp(issued + Duration::seconds(25));
            attempts.push(AlertDeliveryAttempt {
                alert_id: alert.alert_id.clone(),
                attempt_number: 2,
                channel: fallback_channel,
                outcome: AlertDeliveryOutcome::Delivered,
                reason_code: AlertReasonCode::Ready.code().to_string(),
                correlation_id: alert.correlation_id.clone(),
                attempted_at: fallback_attempt_time,
                delivered_at: Some(fallback_delivered_at.clone()),
                failed_at: None,
            });
            if !critical_dispatch_within_sla(&outcome_alert, &fallback_delivered_at, 30)? {
                outcome_alert.status = AlertDispatchStatus::Failed;
                outcome_alert.failed_at = Some(fallback_delivered_at.clone());
                outcome_alert.delivered_at = None;
                outcome_alert.reason_code = AlertReasonCode::DeliverySlaBreached.code().to_string();
                failure = Some((
                    AlertReasonCode::DeliverySlaBreached,
                    "critical alert dispatch breached 30-second SLA".to_string(),
                ));
            } else {
                outcome_alert.status = AlertDispatchStatus::Delivered;
                outcome_alert.delivered_at = Some(fallback_delivered_at);
                outcome_alert.failed_at = None;
            }
        }
    } else {
        let delivered_at = format_timestamp(issued + Duration::seconds(simulated_delay_seconds));
        attempts.push(AlertDeliveryAttempt {
            alert_id: alert.alert_id.clone(),
            attempt_number: 1,
            channel: primary_channel,
            outcome: AlertDeliveryOutcome::Delivered,
            reason_code: AlertReasonCode::Ready.code().to_string(),
            correlation_id: alert.correlation_id.clone(),
            attempted_at: primary_attempt_time,
            delivered_at: Some(delivered_at.clone()),
            failed_at: None,
        });
        outcome_alert.status = AlertDispatchStatus::Delivered;
        outcome_alert.delivered_at = Some(delivered_at.clone());
        outcome_alert.failed_at = None;
        if !critical_dispatch_within_sla(&outcome_alert, &delivered_at, 30)? {
            outcome_alert.status = AlertDispatchStatus::Failed;
            outcome_alert.delivered_at = None;
            outcome_alert.failed_at = Some(delivered_at.clone());
            outcome_alert.reason_code = AlertReasonCode::DeliverySlaBreached.code().to_string();
            failure = Some((
                AlertReasonCode::DeliverySlaBreached,
                "critical alert dispatch breached 30-second SLA".to_string(),
            ));
        }
    }

    for attempt in &attempts {
        domain::alerts::validate_alert_delivery_attempt(attempt)?;
    }
    domain::alerts::validate_incident_alert(&outcome_alert)?;

    let terminal_time = outcome_alert
        .delivered_at
        .clone()
        .or_else(|| outcome_alert.failed_at.clone())
        .unwrap_or_else(|| outcome_alert.issued_at.clone());
    let terminal = OffsetDateTime::parse(&terminal_time, &Rfc3339).map_err(|_| {
        AlertContractError::invalid_payload_with_issues(
            "terminal dispatch timestamp must be an RFC3339 UTC timestamp",
            vec![AlertValidationIssue {
                field: "terminal_time",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "terminal dispatch timestamp must be an RFC3339 UTC timestamp".to_string(),
            }],
        )
    })?;
    let dispatch_latency_seconds = (terminal - issued).whole_seconds();

    Ok(AlertDispatchSimulation {
        alert: outcome_alert,
        attempts,
        fallback_used,
        dispatch_latency_seconds,
        failure,
    })
}

fn parse_alert_limit(limit: Option<i64>) -> Result<i64, AlertContractError> {
    let limit = limit.unwrap_or(25);
    if !(1..=200).contains(&limit) {
        return Err(AlertContractError::invalid_payload_with_issues(
            "limit must be between 1 and 200",
            vec![AlertValidationIssue {
                field: "limit",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "limit must be between 1 and 200".to_string(),
            }],
        ));
    }
    Ok(limit)
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .expect("RFC3339 formatting for UTC timestamp must succeed")
}

fn synthetic_incident_alerts(as_of_utc: &str, correlation_id: &str) -> Vec<IncidentAlert> {
    let as_of = OffsetDateTime::parse(as_of_utc, &Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .to_offset(UtcOffset::UTC);
    let issued_critical = format_timestamp(as_of - Duration::seconds(20));
    let issued_warning = format_timestamp(as_of - Duration::minutes(3));

    vec![
        IncidentAlert {
            alert_id: compose_alert_identifier(
                correlation_id,
                AlertReasonCode::StreamDisconnectExceeded,
                &issued_critical,
            )
            .unwrap_or_else(|_| "incident::alert::stream::synthetic::critical".to_string()),
            severity: AlertSeverity::Critical,
            impacted_subsystem: "market-stream".to_string(),
            cause: "Market stream disconnect exceeded 5 minutes in active trading window."
                .to_string(),
            recommended_next_action:
                "Pause submissions and validate stream recovery before resuming privileged actions."
                    .to_string(),
            evidence_link:
                "https://docs.example.com/operations/severity-alert-delivery#stream-disconnect"
                    .to_string(),
            issued_at: issued_critical.clone(),
            correlation_id: normalize_alert_identifier(correlation_id),
            reason_code: AlertReasonCode::StreamDisconnectExceeded.code().to_string(),
            status: AlertDispatchStatus::Delivered,
            delivered_at: Some(format_timestamp(as_of - Duration::seconds(10))),
            failed_at: None,
        },
        IncidentAlert {
            alert_id: compose_alert_identifier(
                correlation_id,
                AlertReasonCode::ReconciliationLagExceeded,
                &issued_warning,
            )
            .unwrap_or_else(|_| "incident::alert::reconciliation::synthetic::warning".to_string()),
            severity: AlertSeverity::Warning,
            impacted_subsystem: "reconciliation".to_string(),
            cause: "Reconciliation lag exceeded 60 seconds for active portfolios.".to_string(),
            recommended_next_action:
                "Inspect reconciliation backlog and verify lifecycle parity before escalation."
                    .to_string(),
            evidence_link:
                "https://docs.example.com/operations/severity-alert-delivery#reconciliation-lag"
                    .to_string(),
            issued_at: issued_warning,
            correlation_id: normalize_alert_identifier(correlation_id),
            reason_code: AlertReasonCode::ReconciliationLagExceeded
                .code()
                .to_string(),
            status: AlertDispatchStatus::Delivered,
            delivered_at: Some(format_timestamp(as_of - Duration::minutes(2))),
            failed_at: None,
        },
    ]
}

fn synthetic_alert_delivery_attempts(alert: &IncidentAlert) -> Vec<AlertDeliveryAttempt> {
    let issued = OffsetDateTime::parse(&alert.issued_at, &Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .to_offset(UtcOffset::UTC);
    let first_attempt_time = format_timestamp(issued + Duration::seconds(1));
    if alert.status == AlertDispatchStatus::Delivered {
        let delivered_at = alert
            .delivered_at
            .clone()
            .unwrap_or_else(|| format_timestamp(issued + Duration::seconds(5)));
        return vec![AlertDeliveryAttempt {
            alert_id: alert.alert_id.clone(),
            attempt_number: 1,
            channel: AlertDeliveryChannel::PagerDuty,
            outcome: AlertDeliveryOutcome::Delivered,
            reason_code: AlertReasonCode::Ready.code().to_string(),
            correlation_id: alert.correlation_id.clone(),
            attempted_at: first_attempt_time,
            delivered_at: Some(delivered_at),
            failed_at: None,
        }];
    }

    vec![
        AlertDeliveryAttempt {
            alert_id: alert.alert_id.clone(),
            attempt_number: 1,
            channel: AlertDeliveryChannel::PagerDuty,
            outcome: AlertDeliveryOutcome::Failed,
            reason_code: AlertReasonCode::DeliveryPrimaryFailed.code().to_string(),
            correlation_id: alert.correlation_id.clone(),
            attempted_at: first_attempt_time.clone(),
            delivered_at: None,
            failed_at: Some(first_attempt_time),
        },
        AlertDeliveryAttempt {
            alert_id: alert.alert_id.clone(),
            attempt_number: 2,
            channel: AlertDeliveryChannel::Slack,
            outcome: AlertDeliveryOutcome::Failed,
            reason_code: AlertReasonCode::DeliveryFallbackFailed.code().to_string(),
            correlation_id: alert.correlation_id.clone(),
            attempted_at: alert
                .failed_at
                .clone()
                .unwrap_or_else(|| format_timestamp(issued + Duration::seconds(8))),
            delivered_at: None,
            failed_at: alert.failed_at.clone(),
        },
    ]
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

pub async fn evaluate_recovery_readiness(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<RecoveryReadinessEvaluatePayload>,
) -> Response {
    let endpoint = "/control/recovery/readiness/evaluate".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let run = match state.recovery_orchestrator.evaluate_recovery_readiness(
        EvaluateRecoveryReadinessInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            requested_at_utc: authorization.timestamp_utc.clone(),
            profile_key: payload.profile_key,
            reconciliation_run_id: payload.reconciliation_run_id,
            approved_checksum: payload.approved_checksum,
            signoff_intent: payload.signoff_intent,
            audit_reference: payload.audit_reference,
        },
    ) {
        Ok(run) => run,
        Err(error) => {
            return recovery_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "recovery_readiness_evaluate",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    recovery_readiness_response(
        &state,
        &actor,
        run,
        endpoint,
        "recovery_readiness_evaluate",
        "POST",
        StatusCode::OK,
    )
}

pub async fn execute_recovery_resume(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    axum::Json(payload): axum::Json<RecoveryResumePayload>,
) -> Response {
    let endpoint = "/control/recovery/resume".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let resumed_at_utc = payload
        .resumed_at_utc
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    let decision =
        match state
            .recovery_orchestrator
            .execute_recovery_resume(ExecuteRecoveryResumeInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                correlation_id: actor.correlation_id.clone(),
                resumed_at_utc: resumed_at_utc.clone(),
                run_id: payload.run_id,
                incident_correlation_id: payload.incident_correlation_id,
                artifact_id: payload.artifact_id,
                incident_severity: payload.incident_severity,
                rehearsal_run_id: payload.rehearsal_run_id,
            }) {
            Ok(decision) => decision,
            Err(error) => {
                return recovery_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "recovery_resume_execute",
                    &actor,
                    resumed_at_utc,
                    endpoint,
                );
            }
        };

    recovery_resume_response(
        &state,
        &actor,
        decision,
        endpoint,
        "recovery_resume_execute",
        "POST",
    )
}

pub async fn execute_restore_rehearsal(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    payload: Result<axum::Json<RecoveryRehearsalExecutePayload>, JsonRejection>,
) -> Response {
    let endpoint = "/control/recovery/rehearsals".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let payload = match payload {
        Ok(axum::Json(payload)) => payload,
        Err(rejection) => {
            let rejection_message = rejection.body_text();
            return recovery_service_error_response(
                RecoveryReasonCode::InvalidPayload.code(),
                format!("invalid rehearsal payload: {rejection_message}"),
                vec![domain::recovery::RecoveryValidationIssue {
                    field: "payload",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: rejection_message,
                }],
                "recovery_rehearsal_execute",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    let run =
        match state
            .recovery_orchestrator
            .execute_restore_rehearsal(ExecuteRestoreRehearsalInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                correlation_id: actor.correlation_id.clone(),
                requested_at_utc: authorization.timestamp_utc.clone(),
                artifact_id: payload.artifact_id,
                artifact_checksum: payload.artifact_checksum,
                restore_target: payload.restore_target,
                reconciliation_run_id: payload.reconciliation_run_id,
                restore_output: payload.restore_output,
                observed_checksum: payload.observed_checksum,
                incident_correlation_id: payload.incident_correlation_id,
                incident_severity: payload.incident_severity,
                audit_reference: payload.audit_reference,
            }) {
            Ok(run) => run,
            Err(error) => {
                return recovery_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "recovery_rehearsal_execute",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    recovery_rehearsal_response(
        &state,
        &actor,
        run,
        endpoint,
        "recovery_rehearsal_execute",
        "POST",
        StatusCode::ACCEPTED,
    )
}

pub async fn query_restore_rehearsal_by_run_id(
    State(state): State<ControlApiState>,
    Path(run_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/recovery/rehearsals/{run_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let run = match state
        .recovery_orchestrator
        .query_restore_rehearsal_by_run_id(QueryRestoreRehearsalByRunIdInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            queried_at_utc: authorization.timestamp_utc.clone(),
            run_id,
        }) {
        Ok(run) => run,
        Err(error) => {
            return recovery_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "recovery_rehearsal_query",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    recovery_rehearsal_response(
        &state,
        &actor,
        run,
        endpoint,
        "recovery_rehearsal_query",
        "GET",
        StatusCode::OK,
    )
}

pub async fn query_restore_rehearsals(
    State(state): State<ControlApiState>,
    Query(query): Query<RecoveryRehearsalQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/recovery/rehearsals".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let runs =
        match state
            .recovery_orchestrator
            .query_restore_rehearsals(QueryRestoreRehearsalsInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                correlation_id: actor.correlation_id.clone(),
                queried_at_utc: authorization.timestamp_utc.clone(),
                artifact_id: query.artifact_id,
                query_correlation_id: query.correlation_id,
                limit: query.limit,
            }) {
            Ok(runs) => runs,
            Err(error) => {
                return recovery_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "recovery_rehearsal_query",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    recovery_rehearsal_query_response(
        &state,
        &actor,
        runs,
        endpoint,
        "recovery_rehearsal_query",
        "GET",
        authorization.timestamp_utc,
    )
}

pub async fn query_recovery_gate_run_by_run_id(
    State(state): State<ControlApiState>,
    Path(run_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/recovery/runs/{run_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let run = match state
        .recovery_orchestrator
        .query_recovery_gate_run(QueryRecoveryGateRunInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            queried_at_utc: authorization.timestamp_utc.clone(),
            run_id: Some(run_id),
            query_correlation_id: None,
        }) {
        Ok(run) => run,
        Err(error) => {
            return recovery_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "recovery_readiness_query",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    recovery_readiness_response(
        &state,
        &actor,
        run,
        endpoint,
        "recovery_readiness_query",
        "GET",
        StatusCode::OK,
    )
}

pub async fn query_recovery_gate_run(
    State(state): State<ControlApiState>,
    Query(query): Query<RecoveryGateRunQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = "/control/recovery/runs".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let run = match state
        .recovery_orchestrator
        .query_recovery_gate_run(QueryRecoveryGateRunInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            queried_at_utc: authorization.timestamp_utc.clone(),
            run_id: None,
            query_correlation_id: query.correlation_id,
        }) {
        Ok(run) => run,
        Err(error) => {
            return recovery_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "recovery_readiness_query",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    recovery_readiness_response(
        &state,
        &actor,
        run,
        endpoint,
        "recovery_readiness_query",
        "GET",
        StatusCode::OK,
    )
}

pub async fn upsert_report_schedule(
    State(state): State<ControlApiState>,
    Path(schedule_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    maybe_payload: Option<axum::Json<ReportScheduleMutationPayload>>,
) -> Response {
    let endpoint = format!("/control/report-schedules/{schedule_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let Some(axum::Json(payload)) = maybe_payload else {
        return report_schedule_service_error_response(
            ReportingScheduleReasonCode::InvalidPayload.code(),
            "request body is required".to_string(),
            vec![ReportingScheduleValidationIssue {
                field: "body",
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: "request body is required".to_string(),
            }],
            "report_schedule_upsert",
            &actor,
            authorization.timestamp_utc,
            endpoint,
        );
    };

    let cadence = payload
        .cadence
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if cadence.is_empty() {
        return report_schedule_service_error_response(
            ReportingScheduleReasonCode::InvalidPayload.code(),
            "cadence is required".to_string(),
            vec![ReportingScheduleValidationIssue {
                field: "cadence",
                code: ReportingScheduleReasonCode::InvalidPayload.code(),
                message: "cadence is required".to_string(),
            }],
            "report_schedule_upsert",
            &actor,
            authorization.timestamp_utc,
            endpoint,
        );
    }
    if let Some(scheduled_at_utc) = payload
        .scheduled_at_utc
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Err(error) = parse_utc_timestamp("scheduled_at_utc", scheduled_at_utc) {
            return report_schedule_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "report_schedule_upsert",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    }

    let evidence =
        match state
            .report_schedule_orchestrator
            .upsert_schedule(UpsertReportScheduleInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                schedule_id,
                cadence,
                reason_code: payload.reason_code,
                correlation_id: payload
                    .correlation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| actor.correlation_id.clone()),
                timestamp_utc: authorization.timestamp_utc.clone(),
                runbook_url: payload.runbook_url,
            }) {
            Ok(evidence) => evidence,
            Err(error) => {
                return report_schedule_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_schedule_upsert",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_schedule_mutation_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "report_schedule_upsert",
        "POST",
        StatusCode::ACCEPTED,
    )
}

pub async fn pause_report_schedule(
    State(state): State<ControlApiState>,
    Path(schedule_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    maybe_payload: Option<axum::Json<ReportScheduleActionPayload>>,
) -> Response {
    let endpoint = format!("/control/report-schedules/{schedule_id}/pause");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let payload = maybe_payload
        .map(|axum::Json(payload)| payload)
        .unwrap_or_default();
    if let Some(observed_at_utc) = payload
        .observed_at_utc
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Err(error) = parse_utc_timestamp("observed_at_utc", observed_at_utc) {
            return report_schedule_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "report_schedule_pause",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    }

    let evidence =
        match state
            .report_schedule_orchestrator
            .pause_schedule(PauseReportScheduleInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                schedule_id,
                reason_code: payload.reason_code,
                correlation_id: payload
                    .correlation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| actor.correlation_id.clone()),
                timestamp_utc: authorization.timestamp_utc.clone(),
            }) {
            Ok(evidence) => evidence,
            Err(error) => {
                return report_schedule_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_schedule_pause",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_schedule_mutation_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "report_schedule_pause",
        "POST",
        StatusCode::ACCEPTED,
    )
}

pub async fn resume_report_schedule(
    State(state): State<ControlApiState>,
    Path(schedule_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    maybe_payload: Option<axum::Json<ReportScheduleActionPayload>>,
) -> Response {
    let endpoint = format!("/control/report-schedules/{schedule_id}/resume");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let payload = maybe_payload
        .map(|axum::Json(payload)| payload)
        .unwrap_or_default();
    if let Some(observed_at_utc) = payload
        .observed_at_utc
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Err(error) = parse_utc_timestamp("observed_at_utc", observed_at_utc) {
            return report_schedule_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "report_schedule_resume",
                &actor,
                authorization.timestamp_utc.clone(),
                endpoint,
            );
        }
    }

    let evidence =
        match state
            .report_schedule_orchestrator
            .resume_schedule(ResumeReportScheduleInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                schedule_id,
                reason_code: payload.reason_code,
                correlation_id: payload
                    .correlation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| actor.correlation_id.clone()),
                timestamp_utc: authorization.timestamp_utc.clone(),
            }) {
            Ok(evidence) => evidence,
            Err(error) => {
                return report_schedule_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_schedule_resume",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_schedule_mutation_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "report_schedule_resume",
        "POST",
        StatusCode::ACCEPTED,
    )
}

pub async fn query_report_schedule_runs(
    State(state): State<ControlApiState>,
    Path(schedule_id): Path<String>,
    Query(query): Query<ReportScheduleRunsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/report-schedules/{schedule_id}/runs");
    let authorization = match authorize_report_schedule_read(&state, &actor, &endpoint) {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let queried_schedule_id = schedule_id.clone();

    let runs =
        match state
            .report_schedule_orchestrator
            .query_run_history(QueryReportRunHistoryInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                schedule_id: queried_schedule_id.clone(),
                correlation_id: query
                    .correlation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| actor.correlation_id.clone()),
                queried_at_utc: authorization.timestamp_utc.clone(),
                limit: query.limit,
            }) {
            Ok(runs) => runs,
            Err(error) => {
                return report_schedule_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_schedule_run_history_query",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_schedule_run_history_response(
        &state,
        &actor,
        queried_schedule_id,
        runs,
        endpoint,
        authorization.timestamp_utc,
    )
}

pub async fn trigger_on_demand_report_export(
    State(state): State<ControlApiState>,
    Extension(actor): Extension<AuthenticatedActor>,
    maybe_payload: Option<axum::Json<ReportExportTriggerPayload>>,
) -> Response {
    let endpoint = "/control/report-exports/on-demand".to_string();
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let payload = maybe_payload
        .map(|axum::Json(payload)| payload)
        .unwrap_or_default();

    let requested_at_utc = authorization.timestamp_utc.clone();
    let as_of_utc = payload
        .as_of_utc
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| requested_at_utc.clone());
    if let Err(error) = parse_export_utc_timestamp("as_of_utc", &as_of_utc) {
        return report_export_service_error_response(
            error.code,
            error.message,
            error.field_errors,
            "report_export_on_demand_trigger",
            &actor,
            requested_at_utc,
            endpoint,
        );
    }

    let evidence = match state.report_export_orchestrator.trigger_on_demand_export(
        TriggerOnDemandExportInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: payload
                .correlation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| actor.correlation_id.clone()),
            requested_at_utc: authorization.timestamp_utc.clone(),
            as_of_utc,
            reason_code: payload.reason_code,
            unavailable_artifact_types: payload.unavailable_artifact_types,
        },
    ) {
        Ok(evidence) => evidence,
        Err(error) => {
            return report_export_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "report_export_on_demand_trigger",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    report_export_trigger_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "report_export_on_demand_trigger",
        "POST",
    )
}

pub async fn trigger_incident_report_export(
    State(state): State<ControlApiState>,
    Path(incident_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
    maybe_payload: Option<axum::Json<ReportExportIncidentTriggerPayload>>,
) -> Response {
    let endpoint = format!("/control/report-exports/incidents/{incident_id}");
    let authorization = match authorize_critical_action(&state, &actor, &endpoint, "POST") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let payload = maybe_payload
        .map(|axum::Json(payload)| payload)
        .unwrap_or_default();

    let incident_severity = payload
        .incident_severity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if incident_severity.is_empty() {
        return report_export_service_error_response(
            ReportingExportReasonCode::MissingIncidentContext.code(),
            "incident_severity is required for incident-triggered exports".to_string(),
            vec![ReportingExportValidationIssue {
                field: "incident_severity",
                code: ReportingExportReasonCode::MissingIncidentContext.code(),
                message: "incident_severity is required for incident-triggered exports".to_string(),
            }],
            "report_export_incident_trigger",
            &actor,
            authorization.timestamp_utc,
            endpoint,
        );
    }

    let as_of_utc = payload
        .as_of_utc
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| authorization.timestamp_utc.clone());
    if let Err(error) = parse_export_utc_timestamp("as_of_utc", &as_of_utc) {
        return report_export_service_error_response(
            error.code,
            error.message,
            error.field_errors,
            "report_export_incident_trigger",
            &actor,
            authorization.timestamp_utc,
            endpoint,
        );
    }

    let evidence =
        match state
            .report_export_orchestrator
            .trigger_incident_export(TriggerIncidentExportInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                incident_id,
                incident_severity,
                correlation_id: payload
                    .correlation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| actor.correlation_id.clone()),
                requested_at_utc: authorization.timestamp_utc.clone(),
                as_of_utc,
                reason_code: payload.reason_code,
                unavailable_artifact_types: payload.unavailable_artifact_types,
                impacted_system: payload.impacted_system,
                runbook_url: payload.runbook_url,
            }) {
            Ok(evidence) => evidence,
            Err(error) => {
                return report_export_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_export_incident_trigger",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_export_trigger_response(
        &state,
        &actor,
        evidence,
        endpoint,
        "report_export_incident_trigger",
        "POST",
    )
}

pub async fn query_report_export_job(
    State(state): State<ControlApiState>,
    Path(job_id): Path<String>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/report-exports/{job_id}");
    let authorization = match authorize_report_export_read(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };

    let job = match state
        .report_export_orchestrator
        .query_export_job(QueryExportJobInput {
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            job_id: job_id.clone(),
            correlation_id: actor.correlation_id.clone(),
            queried_at_utc: authorization.timestamp_utc.clone(),
        }) {
        Ok(job) => job,
        Err(error) => {
            return report_export_service_error_response(
                error.code,
                error.message,
                error.field_errors,
                "report_export_job_query",
                &actor,
                authorization.timestamp_utc,
                endpoint,
            );
        }
    };

    report_export_job_response(
        &state,
        &actor,
        job,
        endpoint,
        "report_export_job_query",
        authorization.timestamp_utc,
    )
}

pub async fn list_report_export_artifacts(
    State(state): State<ControlApiState>,
    Path(job_id): Path<String>,
    Query(query): Query<ReportExportArtifactsQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/report-exports/{job_id}/artifacts");
    let authorization = match authorize_report_export_read(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let effective_correlation_id = query
        .correlation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| actor.correlation_id.clone());

    let artifacts =
        match state
            .report_export_orchestrator
            .list_export_artifacts(ListExportArtifactsInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                job_id: job_id.clone(),
                correlation_id: effective_correlation_id.clone(),
                queried_at_utc: authorization.timestamp_utc.clone(),
                limit: query.limit,
            }) {
            Ok(artifacts) => artifacts,
            Err(error) => {
                return report_export_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_export_artifact_list",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_export_artifact_list_response(
        &state,
        &actor,
        job_id,
        artifacts,
        endpoint,
        "report_export_artifact_list",
        effective_correlation_id,
        authorization.timestamp_utc,
    )
}

pub async fn read_report_export_artifact(
    State(state): State<ControlApiState>,
    Path((job_id, artifact_id)): Path<(String, String)>,
    Query(query): Query<ReportExportArtifactQuery>,
    Extension(actor): Extension<AuthenticatedActor>,
) -> Response {
    let endpoint = format!("/control/report-exports/{job_id}/artifacts/{artifact_id}");
    let authorization = match authorize_report_export_read(&state, &actor, &endpoint, "GET") {
        Ok(decision) => decision,
        Err(response) => return *response,
    };
    let effective_correlation_id = query
        .correlation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| actor.correlation_id.clone());

    let artifact =
        match state
            .report_export_orchestrator
            .get_export_artifact(GetExportArtifactInput {
                actor_id: actor.actor_id.clone(),
                actor_role: actor.role.clone(),
                job_id: job_id.clone(),
                artifact_id: artifact_id.clone(),
                correlation_id: effective_correlation_id.clone(),
                queried_at_utc: authorization.timestamp_utc.clone(),
            }) {
            Ok(artifact) => artifact,
            Err(error) => {
                return report_export_service_error_response(
                    error.code,
                    error.message,
                    error.field_errors,
                    "report_export_artifact_read",
                    &actor,
                    authorization.timestamp_utc,
                    endpoint,
                );
            }
        };

    report_export_artifact_response(
        &state,
        &actor,
        artifact,
        endpoint,
        "report_export_artifact_read",
        effective_correlation_id,
        authorization.timestamp_utc,
    )
}

fn report_export_trigger_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    evidence: ReportExportJobEvidence,
    endpoint: String,
    action: &'static str,
    http_method: &'static str,
) -> Response {
    let trigger_source = evidence.trigger_source.clone();
    let status = evidence.status.clone();
    let reason_code = evidence.reason_code.clone();
    let correlation_id = evidence.correlation_id.clone();
    let timestamp_utc = evidence.updated_at_utc.clone();
    let job_id = evidence.job_id.clone();
    let response_data = ReportExportTriggerData {
        job: ReportExportJobItem {
            job_id: job_id.clone(),
            trigger_source: evidence.trigger_source,
            status: evidence.status,
            reason_code: evidence.reason_code,
            package_reference: evidence.package_reference,
            package_checksum: evidence.package_checksum,
            requested_at_utc: evidence.requested_at_utc,
            started_at_utc: evidence.started_at_utc,
            finished_at_utc: evidence.finished_at_utc,
            schedule_id: None,
            schedule_window_key: None,
            report_run_id: None,
            incident_id: None,
            incident_severity: None,
            failure_metadata: None,
        },
        artifact_count: evidence.artifact_count,
        missing_artifact_types: evidence.missing_artifact_types,
    };

    report_export_success_response(
        state,
        actor,
        response_data,
        ReportExportSuccessContext {
            endpoint,
            action,
            http_method,
            status: StatusCode::ACCEPTED,
            reason_code,
            correlation_id,
            timestamp_utc,
            signal_name: "report_export_trigger_applied_v1",
            job_id,
            trigger_source,
            lifecycle_status: status,
        },
    )
}

fn report_export_job_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    job: ExportJobRecord,
    endpoint: String,
    action: &'static str,
    timestamp_utc: String,
) -> Response {
    let reason_code = ReportingExportReasonCode::Ready.code().to_string();
    let correlation_id = actor.correlation_id.clone();
    let job_id = job.job_id.clone();
    let trigger_source = job.trigger_source.as_str().to_string();
    let lifecycle_status = job.status.as_str().to_string();
    report_export_success_response(
        state,
        actor,
        ReportExportJobData {
            job: to_report_export_job_item(job),
        },
        ReportExportSuccessContext {
            endpoint,
            action,
            http_method: "GET",
            status: StatusCode::OK,
            reason_code,
            correlation_id,
            timestamp_utc,
            signal_name: "report_export_job_query_v1",
            job_id,
            trigger_source,
            lifecycle_status,
        },
    )
}

fn report_export_artifact_list_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    job_id: String,
    artifacts: Vec<ExportArtifactRecord>,
    endpoint: String,
    action: &'static str,
    correlation_id: String,
    timestamp_utc: String,
) -> Response {
    let response_artifacts = artifacts
        .into_iter()
        .map(to_report_export_artifact_item)
        .collect::<Vec<_>>();
    report_export_success_response(
        state,
        actor,
        ReportExportArtifactsData {
            job_id: job_id.clone(),
            artifacts: response_artifacts,
        },
        ReportExportSuccessContext {
            endpoint,
            action,
            http_method: "GET",
            status: StatusCode::OK,
            reason_code: ReportingExportReasonCode::Ready.code().to_string(),
            correlation_id,
            timestamp_utc,
            signal_name: "report_export_artifact_list_v1",
            job_id,
            trigger_source: "mixed".to_string(),
            lifecycle_status: "query".to_string(),
        },
    )
}

fn report_export_artifact_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    artifact: ExportArtifactRecord,
    endpoint: String,
    action: &'static str,
    correlation_id: String,
    timestamp_utc: String,
) -> Response {
    report_export_success_response(
        state,
        actor,
        ReportExportArtifactData {
            artifact: to_report_export_artifact_item(artifact.clone()),
        },
        ReportExportSuccessContext {
            endpoint,
            action,
            http_method: "GET",
            status: StatusCode::OK,
            reason_code: ReportingExportReasonCode::Ready.code().to_string(),
            correlation_id,
            timestamp_utc,
            signal_name: "report_export_artifact_read_v1",
            job_id: artifact.job_id,
            trigger_source: "artifact".to_string(),
            lifecycle_status: "query".to_string(),
        },
    )
}

fn report_export_success_response<T: Serialize>(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    data: T,
    context: ReportExportSuccessContext,
) -> Response {
    let ReportExportSuccessContext {
        endpoint,
        action,
        http_method,
        status,
        reason_code,
        correlation_id,
        timestamp_utc,
        signal_name,
        job_id,
        trigger_source,
        lifecycle_status,
    } = context;
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action.to_string(),
        parameters: json!({
            "endpoint": endpoint.clone(),
            "http_method": http_method,
            "job_id": job_id.clone(),
            "trigger_source": trigger_source.clone(),
            "status": lifecycle_status.clone(),
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
            action.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            correlation_id,
            timestamp_utc,
        );
    }

    emit_report_export_route_telemetry(ReportExportRouteTelemetryEvent {
        event_name: "control_api_report_export_transition_v1",
        signal_name,
        alert_compatible: false,
        alert_target_seconds: 30,
        action,
        actor_id: &actor.actor_id,
        role: &actor.role,
        correlation_id: &correlation_id,
        job_id: &job_id,
        trigger_source: &trigger_source,
        status: &lifecycle_status,
        reason_code: &reason_code,
        timestamp_utc: &timestamp_utc,
    });

    (
        status,
        axum::Json(ReportExportEnvelope {
            data: Some(data),
            meta: ReportExportMeta {
                action: action.to_string(),
                actor_id: actor.actor_id.clone(),
                role: actor.role.clone(),
                correlation_id,
                timestamp_utc,
                endpoint,
            },
            error: None,
        }),
    )
        .into_response()
}

fn authorize_report_export_read(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    endpoint: &str,
    http_method: &'static str,
) -> Result<AuthorizationDecision, Box<Response>> {
    let decision = state
        .authorization_guard
        .evaluate(actor, ControlAction::ReadAnalyticsDashboard);
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
    Err(Box::new(report_export_service_error_response(
        ReportingExportReasonCode::Unauthorized.code(),
        machine_error.message,
        Vec::new(),
        "report_export_read",
        actor,
        decision.timestamp_utc,
        endpoint.to_string(),
    )))
}

fn report_export_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<ReportingExportValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let is_unauthorized = error_code == ReportingExportReasonCode::Unauthorized.code();
    let security_signal = if is_unauthorized {
        Some(ReportExportSecuritySignal {
            name: if action.contains("trigger") {
                "unauthorized_report_export_mutation_attempt_v1"
            } else {
                "unauthorized_report_export_read_attempt_v1"
            },
            severity: "high",
            alert_compatible: true,
            alert_target_seconds: 30,
        })
    } else {
        None
    };

    emit_report_export_route_telemetry(ReportExportRouteTelemetryEvent {
        event_name: "control_api_report_export_transition_v1",
        signal_name: if is_unauthorized {
            "report_export_transition_rejected_v1"
        } else {
            "report_export_transition_failed_v1"
        },
        alert_compatible: is_unauthorized,
        alert_target_seconds: 30,
        action,
        actor_id: &actor.actor_id,
        role: &actor.role,
        correlation_id: &actor.correlation_id,
        job_id: "n/a",
        trigger_source: "n/a",
        status: "error",
        reason_code: error_code,
        timestamp_utc: &timestamp_utc,
    });

    (
        report_export_service_error_status(error_code),
        axum::Json(ReportExportEnvelope::<serde_json::Value> {
            data: None,
            meta: ReportExportMeta {
                action: action.to_string(),
                actor_id: actor.actor_id.clone(),
                role: actor.role.clone(),
                correlation_id: actor.correlation_id.clone(),
                timestamp_utc,
                endpoint,
            },
            error: Some(ReportExportEnvelopeError {
                error_code: error_code.to_string(),
                message,
                field_errors: field_errors
                    .into_iter()
                    .map(|issue| ReportExportFieldError {
                        field: issue.field.to_string(),
                        code: issue.code.to_string(),
                        message: issue.message,
                    })
                    .collect(),
                security_signal,
            }),
        }),
    )
        .into_response()
}

fn report_export_service_error_status(code: &str) -> StatusCode {
    match code {
        value if value == ReportingExportReasonCode::InvalidPayload.code() => {
            StatusCode::BAD_REQUEST
        }
        value if value == ReportingExportReasonCode::Unauthorized.code() => StatusCode::FORBIDDEN,
        value if value == ReportingExportReasonCode::NotFound.code() => StatusCode::NOT_FOUND,
        value if value == ReportingExportReasonCode::MissingIncidentContext.code() => {
            StatusCode::BAD_REQUEST
        }
        value
            if value == ReportingExportReasonCode::DependencyUnavailable.code()
                || value == ReportingExportReasonCode::StaleEvidence.code()
                || value == ReportingExportReasonCode::IntegrityMismatch.code()
                || value == ReportingExportReasonCode::ArtifactUnavailable.code()
                || value == "report_export_constraint_violation" =>
        {
            StatusCode::CONFLICT
        }
        value
            if value == ReportingExportReasonCode::PersistenceUnavailable.code()
                || value == "report_export_query_failed"
                || value == "report_export_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn emit_report_export_route_telemetry(event: ReportExportRouteTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event)
            .expect("report export route telemetry should always serialize")
    );
}

fn to_report_export_job_item(job: ExportJobRecord) -> ReportExportJobItem {
    ReportExportJobItem {
        job_id: job.job_id,
        trigger_source: job.trigger_source.as_str().to_string(),
        status: job.status.as_str().to_string(),
        reason_code: job.reason_code,
        package_reference: job.package_reference,
        package_checksum: job.package_checksum,
        requested_at_utc: job.requested_at_utc,
        started_at_utc: job.started_at_utc,
        finished_at_utc: job.finished_at_utc,
        schedule_id: job.schedule_id,
        schedule_window_key: job.schedule_window_key,
        report_run_id: job.report_run_id,
        incident_id: job.incident_id,
        incident_severity: job.incident_severity,
        failure_metadata: job.failure_metadata,
    }
}

fn to_report_export_artifact_item(artifact: ExportArtifactRecord) -> ReportExportArtifactItem {
    ReportExportArtifactItem {
        artifact_id: artifact.artifact_id,
        job_id: artifact.job_id,
        artifact_type: artifact.artifact_type.as_str().to_string(),
        source: artifact.source,
        as_of_utc: artifact.as_of_utc,
        reason_code: artifact.reason_code,
        correlation_id: artifact.correlation_id,
        checksum: artifact.checksum,
        retrieval_reference: artifact.retrieval_reference,
        is_available: artifact.is_available,
        updated_at_utc: artifact.updated_at_utc,
    }
}

struct ReportExportSuccessContext {
    endpoint: String,
    action: &'static str,
    http_method: &'static str,
    status: StatusCode,
    reason_code: String,
    correlation_id: String,
    timestamp_utc: String,
    signal_name: &'static str,
    job_id: String,
    trigger_source: String,
    lifecycle_status: String,
}

fn report_schedule_mutation_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    evidence: ReportScheduleMutationEvidence,
    endpoint: String,
    action: &'static str,
    http_method: &'static str,
    status: StatusCode,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "schedule_id": evidence.schedule_id.clone(),
            "cadence": evidence.cadence.clone(),
            "status": evidence.status.clone(),
            "next_run_at_utc": evidence.next_run_at_utc.clone(),
        }),
        approval_reference: None,
        timestamp: evidence.timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: evidence.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: evidence.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            evidence.correlation_id.clone(),
            evidence.timestamp_utc.clone(),
        );
    }

    emit_report_schedule_route_telemetry(ReportScheduleRouteTelemetryEvent {
        event_name: "control_api_report_schedule_transition_v1",
        signal_name: "report_schedule_transition_applied_v1",
        alert_compatible: false,
        alert_target_seconds: 30,
        action,
        actor_id: &actor.actor_id,
        role: &actor.role,
        correlation_id: &evidence.correlation_id,
        schedule_id: &evidence.schedule_id,
        cadence: &evidence.cadence,
        status: &evidence.status,
        reason_code: &evidence.reason_code,
        timestamp_utc: &evidence.timestamp_utc,
    });

    (
        status,
        axum::Json(ReportScheduleMutationResponse {
            status: "accepted",
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: evidence.correlation_id.clone(),
            timestamp_utc: evidence.timestamp_utc.clone(),
            schedule: ReportScheduleMutationItem {
                schedule_id: evidence.schedule_id,
                cadence: evidence.cadence,
                status: evidence.status,
                next_run_at_utc: evidence.next_run_at_utc,
                reason_code: evidence.reason_code,
                runbook_url: evidence.runbook_url,
            },
        }),
    )
        .into_response()
}

fn report_schedule_run_history_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    schedule_id: String,
    runs: Vec<ReportRunRecord>,
    endpoint: String,
    timestamp_utc: String,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "report_schedule_run_history_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "run_count": runs.len(),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: ReportingScheduleReasonCode::Ready.code().to_string(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "report_schedule_run_history_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc,
        );
    }

    let cadence = runs
        .first()
        .map(|first| {
            let first_cadence = first.cadence.as_str();
            if runs.iter().all(|run| run.cadence == first.cadence) {
                first_cadence
            } else {
                "mixed"
            }
        })
        .unwrap_or("none");

    emit_report_schedule_route_telemetry(ReportScheduleRouteTelemetryEvent {
        event_name: "control_api_report_schedule_transition_v1",
        signal_name: "report_schedule_run_history_query_v1",
        alert_compatible: false,
        alert_target_seconds: 30,
        action: "report_schedule_run_history_query",
        actor_id: &actor.actor_id,
        role: &actor.role,
        correlation_id: &actor.correlation_id,
        schedule_id: &schedule_id,
        cadence,
        status: "query",
        reason_code: ReportingScheduleReasonCode::Ready.code(),
        timestamp_utc: &timestamp_utc,
    });

    (
        StatusCode::OK,
        axum::Json(ReportScheduleRunHistoryResponse {
            status: "ok",
            action: "report_schedule_run_history_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            runs: runs
                .into_iter()
                .map(|run| ReportScheduleRunItem {
                    run_id: run.run_id,
                    schedule_id: run.schedule_id,
                    cadence: run.cadence.as_str().to_string(),
                    window_key: run.window_key,
                    window_started_at_utc: run.window_started_at_utc,
                    window_ended_at_utc: run.window_ended_at_utc,
                    status: run.status.as_str().to_string(),
                    reason_code: run.reason_code,
                    correlation_id: run.correlation_id,
                    run_started_at_utc: run.run_started_at_utc,
                    actor_id: run.actor_id,
                    source_context: run.source_context,
                    run_finished_at_utc: run.run_finished_at_utc,
                    alert_emitted_at_utc: run.alert_emitted_at_utc,
                    runbook_url: run.runbook_url,
                    impacted_system: run.impacted_system,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn authorize_report_schedule_read(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    endpoint: &str,
) -> Result<AuthorizationDecision, Box<Response>> {
    let decision = state
        .authorization_guard
        .evaluate(actor, ControlAction::ReadAnalyticsDashboard);
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
    Err(Box::new(report_schedule_service_error_response(
        ReportingScheduleReasonCode::Unauthorized.code(),
        machine_error.message,
        Vec::new(),
        "report_schedule_run_history_query",
        actor,
        decision.timestamp_utc,
        endpoint.to_string(),
    )))
}

fn report_schedule_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<ReportingScheduleValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let security_signal = if error_code == ReportingScheduleReasonCode::Unauthorized.code() {
        Some(ReportScheduleSecuritySignal {
            name: "unauthorized_report_schedule_mutation_attempt_v1",
            severity: "high",
            alert_compatible: true,
            alert_target_seconds: 30,
        })
    } else {
        None
    };

    emit_report_schedule_route_telemetry(ReportScheduleRouteTelemetryEvent {
        event_name: "control_api_report_schedule_transition_v1",
        signal_name: "report_schedule_transition_rejected_v1",
        alert_compatible: security_signal.is_some(),
        alert_target_seconds: 30,
        action,
        actor_id: &actor.actor_id,
        role: &actor.role,
        correlation_id: &actor.correlation_id,
        schedule_id: "unknown",
        cadence: "unknown",
        status: "rejected",
        reason_code: error_code,
        timestamp_utc: &timestamp_utc,
    });

    (
        report_schedule_service_error_status(error_code),
        axum::Json(ReportScheduleServiceErrorResponse {
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
                .map(|issue| ReportScheduleFieldError {
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

fn report_schedule_service_error_status(code: &str) -> StatusCode {
    match code {
        value if value == ReportingScheduleReasonCode::InvalidPayload.code() => {
            StatusCode::BAD_REQUEST
        }
        value if value == ReportingScheduleReasonCode::Unauthorized.code() => StatusCode::FORBIDDEN,
        value if value == ReportingScheduleReasonCode::NotFound.code() => StatusCode::NOT_FOUND,
        value if value == ReportingScheduleReasonCode::StaleEvidence.code() => StatusCode::CONFLICT,
        "report_schedule_constraint_violation" => StatusCode::CONFLICT,
        value
            if value == ReportingScheduleReasonCode::DependencyUnavailable.code()
                || value == ReportingScheduleReasonCode::PersistenceUnavailable.code()
                || value == ReportingScheduleReasonCode::AlertUnavailable.code()
                || value == "report_schedule_query_failed"
                || value == "report_schedule_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn emit_report_schedule_route_telemetry(event: ReportScheduleRouteTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event)
            .expect("report schedule route telemetry event should always serialize")
    );
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

fn authorize_incident_alert_action(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    endpoint: &str,
    http_method: &'static str,
    action: &'static str,
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
            "action": action,
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
    Err(Box::new(incident_alert_service_error_response(
        AlertReasonCode::Unauthorized.code(),
        machine_error.message,
        Vec::new(),
        action,
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

fn reward_risk_policy_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    evidence: RewardRiskPolicyEvidence,
    endpoint: String,
    http_method: &'static str,
    action: &'static str,
) -> Response {
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "policy_key": evidence.policy_key,
            "strategy_key": evidence.strategy_key,
            "min_reward_per_risk": evidence.min_reward_per_risk,
            "default_threshold_applied": evidence.default_threshold_applied,
        }),
        approval_reference: None,
        timestamp: evidence.updated_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: evidence.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: evidence.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            evidence.correlation_id.clone(),
            evidence.updated_at_utc,
        );
    }

    (
        if http_method == "GET" {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        axum::Json(RewardRiskPolicyDecisionResponse {
            status: "accepted",
            error_code: None,
            message: None,
            policy_key: evidence.policy_key,
            strategy_key: evidence.strategy_key,
            min_reward_per_risk: evidence.min_reward_per_risk,
            default_threshold_applied: evidence.default_threshold_applied,
            actor_id: evidence.actor_id,
            role: actor.role.clone(),
            reason_code: evidence.reason_code,
            correlation_id: evidence.correlation_id,
            timestamp_utc: evidence.updated_at_utc,
            security_signal: None,
        }),
    )
        .into_response()
}

fn reward_risk_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<RewardRiskValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let security_signal = if error_code == "reward_risk_unauthorized_role" {
        Some(RewardRiskSecuritySignal {
            name: "unauthorized_reward_risk_mutation_attempt_v1",
            severity: "high",
            alert_compatible: true,
            alert_target_seconds: 30,
        })
    } else {
        None
    };

    (
        reward_risk_service_error_status(error_code),
        axum::Json(RewardRiskServiceErrorResponse {
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
                .map(|issue| RewardRiskFieldError {
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

fn reward_risk_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == RewardRiskReasonCode::InvalidPayload.code()
            || code == RewardRiskReasonCode::InvalidPolicyKey.code()
            || code == RewardRiskReasonCode::InvalidThreshold.code() =>
        {
            StatusCode::BAD_REQUEST
        }
        "reward_risk_unauthorized_role" => StatusCode::FORBIDDEN,
        "reward_risk_constraint_violation" => StatusCode::CONFLICT,
        code if code == RewardRiskReasonCode::PersistenceUnavailable.code()
            || code == "reward_risk_query_failed"
            || code == "reward_risk_row_decode_failed" =>
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

fn synthetic_incident_timeline(
    as_of_utc: &str,
    correlation_id: &str,
) -> Vec<IncidentTimelineEvent> {
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
) -> Result<
    Vec<domain::attribution::AttributionObservation>,
    domain::attribution::AttributionContractError,
> {
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

fn to_regime_shift_alert_item(alert: RegimeShiftAlertRecord) -> RegimeShiftAlertItem {
    RegimeShiftAlertItem {
        alert_id: alert.alert_id,
        market_id: alert.market_id,
        cluster_id: alert.cluster_id,
        reason_code: alert.reason_code,
        severity: alert.severity.as_str().to_string(),
        correlation_id: alert.correlation_id,
        observed_at: alert.observed_at,
        issued_at: alert.issued_at,
        dispatch_status: alert.dispatch_status.as_str().to_string(),
        dispatch_reason_code: alert.dispatch_reason_code,
        recommended_next_action: alert.recommended_next_action,
        evidence_link: alert.evidence_link,
        previous_maker_rebate_bps: alert.previous_maker_rebate_bps,
        current_maker_rebate_bps: alert.current_maker_rebate_bps,
        rebate_delta_bps: alert.rebate_delta_bps,
        previous_spread_bps: alert.previous_spread_bps,
        current_spread_bps: alert.current_spread_bps,
        spread_widening_bps: alert.spread_widening_bps,
        previous_eligibility_state: alert
            .previous_eligibility_state
            .map(|state| state.as_str().to_string()),
        current_eligibility_state: alert
            .current_eligibility_state
            .map(|state| state.as_str().to_string()),
        threshold_rebate_delta_bps: alert.threshold_rebate_delta_bps,
        threshold_spread_widening_bps: alert.threshold_spread_widening_bps,
        attempts: Vec::new(),
        dispatch_latency_seconds: None,
        fallback_used: None,
    }
}

fn to_participation_guardrail_event_item(
    event: domain::risk::PreTradeParticipationGuardrailEvidence,
) -> ParticipationGuardrailEventItem {
    ParticipationGuardrailEventItem {
        event_id: event.event_id,
        guardrail_mode: event.guardrail_mode,
        reason_code: event.reason_code,
        market_id: event.market_id,
        cluster_id: event.cluster_id,
        correlation_id: event.correlation_id,
        observed_at_utc: event.observed_at_utc,
        evaluated_at_utc: event.evaluated_at_utc,
        liquidity_depth_usd: event.liquidity_depth_usd,
        inactivity_gap_seconds: event.inactivity_gap_seconds,
        threshold_liquidity_depth_usd: event.threshold_liquidity_depth_usd,
        threshold_inactivity_pause_seconds: event.threshold_inactivity_pause_seconds,
        threshold_overnight_gap_seconds: event.threshold_overnight_gap_seconds,
        normal_max_order_size_units: event.normal_max_order_size_units,
        capped_max_order_size_units: event.capped_max_order_size_units,
    }
}

fn to_regime_shift_alert_dispatch_item(
    detection: &domain::risk::RegimeShiftDetection,
    simulation: &AlertDispatchSimulation,
    _decision: &AlertTriggerDecision,
) -> RegimeShiftAlertItem {
    RegimeShiftAlertItem {
        alert_id: simulation.alert.alert_id.clone(),
        market_id: detection.market_id.clone(),
        cluster_id: detection.cluster_id.clone(),
        reason_code: detection.reason_code.clone(),
        severity: simulation.alert.severity.as_str().to_string(),
        correlation_id: simulation.alert.correlation_id.clone(),
        observed_at: detection.observed_at_utc.clone(),
        issued_at: simulation.alert.issued_at.clone(),
        dispatch_status: simulation.alert.status.as_str().to_string(),
        dispatch_reason_code: simulation.alert.reason_code.clone(),
        recommended_next_action: simulation.alert.recommended_next_action.clone(),
        evidence_link: simulation.alert.evidence_link.clone(),
        previous_maker_rebate_bps: detection.previous_maker_rebate_bps,
        current_maker_rebate_bps: detection.current_maker_rebate_bps,
        rebate_delta_bps: detection.rebate_delta_bps,
        previous_spread_bps: detection.previous_spread_bps,
        current_spread_bps: detection.current_spread_bps,
        spread_widening_bps: detection.spread_widening_bps,
        previous_eligibility_state: detection
            .previous_eligibility_state
            .map(|state| state.as_str().to_string()),
        current_eligibility_state: detection
            .current_eligibility_state
            .map(|state| state.as_str().to_string()),
        threshold_rebate_delta_bps: detection.threshold_rebate_delta_bps,
        threshold_spread_widening_bps: detection.threshold_spread_widening_bps,
        attempts: simulation
            .attempts
            .iter()
            .map(|attempt| IncidentAlertDeliveryAttemptItem {
                attempt_number: i64::from(attempt.attempt_number),
                channel: attempt.channel.as_str().to_string(),
                outcome: attempt.outcome.as_str().to_string(),
                reason_code: attempt.reason_code.clone(),
                attempted_at: attempt.attempted_at.clone(),
                delivered_at: attempt.delivered_at.clone(),
                failed_at: attempt.failed_at.clone(),
            })
            .collect(),
        dispatch_latency_seconds: Some(simulation.dispatch_latency_seconds),
        fallback_used: Some(simulation.fallback_used),
    }
}

fn regime_shift_alert_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    alerts: Vec<RegimeShiftAlertItem>,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let data_state = if alerts.is_empty() { "empty" } else { "ready" };
    let reason_code = if alerts.is_empty() {
        AlertReasonCode::NoTrigger.code().to_string()
    } else {
        AlertReasonCode::Ready.code().to_string()
    };
    let recommended_next_action = if alerts.is_empty() {
        "No active FR40 regime-shift evidence rows. Continue monitoring venue economics."
            .to_string()
    } else {
        "Review FR40 reason codes, confirm market/cluster impact, and execute runbook guidance."
            .to_string()
    };
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "regime_shift_alerts_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "alert_count": alerts.len(),
            "data_state": data_state,
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "regime_shift_alerts_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc,
        );
    }
    (
        StatusCode::OK,
        axum::Json(RegimeShiftAlertsQueryResponse {
            status: "accepted",
            action: "regime_shift_alerts_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            source: "control-api.regime-shift-alerts.v1".to_string(),
            reason_code,
            data_state: data_state.to_string(),
            recommended_next_action,
            alerts,
        }),
    )
        .into_response()
}

fn participation_guardrail_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    events: Vec<ParticipationGuardrailEventItem>,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let data_state = if events.is_empty() { "empty" } else { "ready" };
    let reason_code = if events.is_empty() {
        AlertReasonCode::NoTrigger.code().to_string()
    } else {
        AlertReasonCode::Ready.code().to_string()
    };
    let recommended_next_action = if events.is_empty() {
        "No FR41 participation guardrail events matched this query. Continue monitoring liquidity and inactivity windows."
            .to_string()
    } else {
        "Review FR41 guardrail modes and reason codes, then confirm operational thresholds and execution-size controls."
            .to_string()
    };
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "participation_guardrail_events_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "event_count": events.len(),
            "data_state": data_state,
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "participation_guardrail_events_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc,
        );
    }
    (
        StatusCode::OK,
        axum::Json(ParticipationGuardrailEventsQueryResponse {
            status: "accepted",
            action: "participation_guardrail_events_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            source: "control-api.participation-guardrails.v1".to_string(),
            reason_code,
            data_state: data_state.to_string(),
            recommended_next_action,
            events,
        }),
    )
        .into_response()
}

fn regime_shift_alert_dispatch_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    alerts: Vec<RegimeShiftAlertItem>,
    duplicate_suppressed_count: usize,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let reason_code = if duplicate_suppressed_count > 0 && alerts.is_empty() {
        AlertReasonCode::DuplicateSuppressed.code().to_string()
    } else if alerts.is_empty() {
        AlertReasonCode::NoTrigger.code().to_string()
    } else {
        AlertReasonCode::Ready.code().to_string()
    };
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "regime_shift_alert_dispatch".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "POST",
            "emitted_alert_count": alerts.len(),
            "duplicate_suppressed_count": duplicate_suppressed_count,
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "regime_shift_alert_dispatch".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc.clone(),
        );
    }

    (
        StatusCode::OK,
        axum::Json(RegimeShiftAlertDispatchResponse {
            status: "accepted",
            action: "regime_shift_alert_dispatch".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            source: "control-api.regime-shift-alerts.v1".to_string(),
            reason_code,
            duplicate_suppressed_count,
            alerts,
        }),
    )
        .into_response()
}

fn incident_alert_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    alerts: Vec<IncidentAlertItem>,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let data_state = if alerts.is_empty() { "empty" } else { "ready" };
    let reason_code = if alerts.is_empty() {
        AlertReasonCode::NoTrigger.code().to_string()
    } else {
        AlertReasonCode::Ready.code().to_string()
    };
    let highest_severity = incident_alert_highest_severity(&alerts);
    let recommended_next_action = if alerts.is_empty() {
        "No active warning/critical alerts. Continue monitoring trigger evidence.".to_string()
    } else if highest_severity == "critical" {
        "Critical alert present: execute containment controls and follow runbook evidence immediately."
            .to_string()
    } else {
        "Review warning alerts and apply the recommended next action for each impacted subsystem."
            .to_string()
    };

    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "incident_alerts_query".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "GET",
            "alert_count": alerts.len(),
            "data_state": data_state,
            "highest_severity": highest_severity,
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "incident_alerts_query".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc,
        );
    }

    (
        StatusCode::OK,
        axum::Json(IncidentAlertsQueryResponse {
            status: "accepted",
            action: "incident_alerts_query".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            source: "control-api.incident-alerts.v1".to_string(),
            reason_code,
            data_state: data_state.to_string(),
            recommended_next_action,
            alerts,
        }),
    )
        .into_response()
}

fn incident_alert_dispatch_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    simulation: AlertDispatchSimulation,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    let AlertDispatchSimulation {
        alert,
        attempts,
        fallback_used,
        dispatch_latency_seconds,
        failure: _,
    } = simulation;
    let reason_code = alert.reason_code.clone();
    let status = alert.status.as_str().to_string();

    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: "incident_alert_dispatch".to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": "POST",
            "alert_id": alert.alert_id,
            "severity": alert.severity.as_str(),
            "status": status,
            "fallback_used": fallback_used,
            "dispatch_latency_seconds": dispatch_latency_seconds,
            "attempt_count": attempts.len(),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            "incident_alert_dispatch".to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc.clone(),
        );
    }

    (
        StatusCode::OK,
        axum::Json(IncidentAlertDispatchResponse {
            status: "accepted",
            action: "incident_alert_dispatch".to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            source: "control-api.incident-alerts.v1".to_string(),
            reason_code,
            fallback_used,
            dispatch_latency_seconds,
            alert: IncidentAlertItem {
                alert_id: alert.alert_id,
                severity: alert.severity.as_str().to_string(),
                impacted_subsystem: alert.impacted_subsystem,
                cause: alert.cause,
                recommended_next_action: alert.recommended_next_action,
                evidence_link: alert.evidence_link,
                issued_at: alert.issued_at,
                correlation_id: alert.correlation_id,
                reason_code: alert.reason_code,
                status: alert.status.as_str().to_string(),
                delivered_at: alert.delivered_at,
                failed_at: alert.failed_at,
                attempts: attempts
                    .into_iter()
                    .map(|attempt| IncidentAlertDeliveryAttemptItem {
                        attempt_number: i64::from(attempt.attempt_number),
                        channel: attempt.channel.as_str().to_string(),
                        outcome: attempt.outcome.as_str().to_string(),
                        reason_code: attempt.reason_code,
                        attempted_at: attempt.attempted_at,
                        delivered_at: attempt.delivered_at,
                        failed_at: attempt.failed_at,
                    })
                    .collect(),
            },
        }),
    )
        .into_response()
}

fn incident_alert_highest_severity(alerts: &[IncidentAlertItem]) -> &'static str {
    if alerts.is_empty() {
        return "normal";
    }
    if alerts
        .iter()
        .any(|alert| alert.severity == AlertSeverity::Critical.as_str())
    {
        AlertSeverity::Critical.as_str()
    } else {
        AlertSeverity::Warning.as_str()
    }
}

fn incident_alert_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<AlertValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        incident_alert_service_error_status(error_code),
        axum::Json(IncidentAlertsServiceErrorResponse {
            error_code,
            reason_code: error_code.to_string(),
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc: timestamp_utc.clone(),
            occurred_at: timestamp_utc,
            source: "control-api.incident-alerts.v1".to_string(),
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| IncidentAlertFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn incident_alert_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == AlertReasonCode::InvalidPayload.code() => StatusCode::BAD_REQUEST,
        code if code == AlertReasonCode::NoTrigger.code() => StatusCode::BAD_REQUEST,
        code if code == RegimeShiftReasonCode::InvalidPayload.code() => StatusCode::BAD_REQUEST,
        code if code == ParticipationGuardrailReasonCode::InvalidPayload.code() => {
            StatusCode::BAD_REQUEST
        }
        code if code == AlertReasonCode::Unauthorized.code() => StatusCode::FORBIDDEN,
        code if code == AlertReasonCode::DuplicateSuppressed.code() => StatusCode::CONFLICT,
        code if code == AlertReasonCode::DependencyUnavailable.code()
            || code == AlertReasonCode::StaleEvidence.code()
            || code == AlertReasonCode::DeliveryPrimaryFailed.code()
            || code == AlertReasonCode::DeliveryFallbackFailed.code()
            || code == AlertReasonCode::DeliverySlaBreached.code()
            || code == RegimeShiftReasonCode::DependencyUnavailable.code()
            || code == RegimeShiftReasonCode::PersistenceUnavailable.code()
            || code == ParticipationGuardrailReasonCode::DependencyUnavailable.code()
            || code == ParticipationGuardrailReasonCode::PersistenceUnavailable.code()
            || code == "regime_shift_query_failed"
            || code == "regime_shift_row_decode_failed"
            || code == "participation_guardrail_query_failed"
            || code == "participation_guardrail_row_decode_failed"
            || code == "alert_query_failed"
            || code == "alert_row_decode_failed" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        "regime_shift_constraint_violation" => StatusCode::CONFLICT,
        "participation_guardrail_constraint_violation" => StatusCode::CONFLICT,
        "alert_constraint_violation" => StatusCode::CONFLICT,
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
        .unwrap_or_else(|| {
            "No trigger evidence available for selected incident scope.".to_string()
        });
    let context = causal_scope
        .iter()
        .find(|event| event.stage == IncidentTimelineStage::Order)
        .map(|event| event.summary.clone())
        .unwrap_or_else(|| {
            "No order-context evidence available for selected incident scope.".to_string()
        });
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

fn recovery_readiness_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    run: domain::recovery::RecoveryGateRunEvidence,
    endpoint: String,
    action_type: &'static str,
    http_method: &'static str,
    status: StatusCode,
) -> Response {
    let recommended_next_action = if run.readiness_status.as_str() == "approved" {
        "Resume verification is approved; execute controlled recovery resume with recorded evidence."
            .to_string()
    } else {
        "Remain in containment and resolve failed recovery gates before resuming.".to_string()
    };
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action_type.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "run_id": run.run_id.clone(),
            "profile_key": run.profile_key.clone(),
            "readiness_status": run.readiness_status.as_str(),
            "reason_code": run.reason_code.clone(),
        }),
        approval_reference: run.audit_reference.clone(),
        timestamp: run.evaluated_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: run.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: run.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action_type.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            run.correlation_id.clone(),
            run.evaluated_at_utc.clone(),
        );
    }

    (
        status,
        axum::Json(RecoveryReadinessDecisionResponse {
            status: "accepted",
            action: action_type.to_string(),
            run_id: run.run_id,
            readiness_status: run.readiness_status.as_str().to_string(),
            reason_code: run.reason_code,
            profile_key: run.profile_key,
            actor_id: run.actor_id,
            actor_role: run.actor_role,
            correlation_id: run.correlation_id,
            requested_at_utc: run.requested_at_utc,
            evaluated_at_utc: run.evaluated_at_utc.clone(),
            resumed_at_utc: run.resumed_at_utc,
            freshness_age_seconds: run.freshness_age_seconds,
            freshness_observed_at_utc: run.freshness_observed_at_utc,
            reconciliation_run_id: run.reconciliation_run_id,
            reconciliation_mismatch_rate: run.reconciliation_mismatch_rate,
            approved_checksum: run.approved_checksum,
            computed_checksum: run.computed_checksum,
            failing_gate_codes: run.failing_gate_codes,
            gate_outcomes: run
                .gate_outcomes
                .into_iter()
                .map(recovery_gate_outcome_item)
                .collect(),
            recommended_next_action,
            audit_reference: run.audit_reference,
            timestamp_utc: run.evaluated_at_utc,
        }),
    )
        .into_response()
}

fn recovery_resume_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    decision: RecoveryResumeExecutionEvidence,
    endpoint: String,
    action_type: &'static str,
    http_method: &'static str,
) -> Response {
    let run = decision.run;
    let verification = decision.verification;
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action_type.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "run_id": run.run_id.clone(),
            "verification_reason_code": verification.reason_code.clone(),
            "readiness_status": run.readiness_status.as_str(),
        }),
        approval_reference: run.audit_reference.clone(),
        timestamp: verification.verified_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: verification.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: run.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action_type.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            run.correlation_id.clone(),
            verification.verified_at_utc.clone(),
        );
    }
    (
        StatusCode::ACCEPTED,
        axum::Json(RecoveryResumeDecisionResponse {
            status: "accepted",
            action: action_type.to_string(),
            run_id: run.run_id,
            readiness_status: run.readiness_status.as_str().to_string(),
            reason_code: run.reason_code,
            verification_reason_code: verification.reason_code,
            actor_id: run.actor_id,
            actor_role: run.actor_role,
            correlation_id: run.correlation_id,
            resumed_at_utc: verification.verified_at_utc.clone(),
            verification_timestamp_utc: verification.verified_at_utc,
            timestamp_utc: run.evaluated_at_utc,
            audit_reference: run.audit_reference,
        }),
    )
        .into_response()
}

fn recovery_rehearsal_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    run: RestoreRehearsalRunEvidence,
    endpoint: String,
    action_type: &'static str,
    http_method: &'static str,
    status: StatusCode,
) -> Response {
    let recommended_next_action = recovery_rehearsal_next_action(&run);
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action_type.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "run_id": run.run_id.clone(),
            "artifact_id": run.artifact_id.clone(),
            "rehearsal_status": run.status.as_str(),
            "reason_code": run.reason_code.clone(),
        }),
        approval_reference: run.audit_reference.clone(),
        timestamp: run.completed_at_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code: run.reason_code.clone(),
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: run.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action_type.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            run.correlation_id.clone(),
            run.completed_at_utc.clone(),
        );
    }

    (
        status,
        axum::Json(RecoveryRehearsalDecisionResponse {
            status: "accepted",
            action: action_type.to_string(),
            run_id: run.run_id,
            rehearsal_status: run.status.as_str().to_string(),
            reason_code: run.reason_code,
            actor_id: actor.actor_id.clone(),
            actor_role: actor.role.clone(),
            correlation_id: run.correlation_id,
            artifact_id: run.artifact_id,
            artifact_checksum: run.artifact_checksum,
            observed_checksum: run.observed_checksum,
            restore_target: run.restore_target,
            reconciliation_run_id: run.reconciliation_run_id,
            reconciliation_mismatch_rate: run.reconciliation_mismatch_rate,
            reconciliation_passed: run.reconciliation_passed,
            requested_at_utc: run.requested_at_utc,
            started_at_utc: run.started_at_utc,
            completed_at_utc: run.completed_at_utc.clone(),
            integrity_checks: run
                .integrity_checks
                .into_iter()
                .map(recovery_rehearsal_integrity_check_item)
                .collect(),
            deterministic_signature: recovery_deterministic_signature_response(
                run.deterministic_signature,
            ),
            recommended_next_action,
            incident_correlation_id: run.incident_correlation_id,
            incident_severity: run.incident_severity,
            audit_reference: run.audit_reference,
            timestamp_utc: run.completed_at_utc,
        }),
    )
        .into_response()
}

fn recovery_rehearsal_query_response(
    state: &ControlApiState,
    actor: &AuthenticatedActor,
    runs: Vec<RestoreRehearsalRunEvidence>,
    endpoint: String,
    action_type: &'static str,
    http_method: &'static str,
    timestamp_utc: String,
) -> Response {
    let reason_code = runs
        .first()
        .map(|run| run.reason_code.clone())
        .unwrap_or_else(|| RecoveryReasonCode::RehearsalSuccess.code().to_string());
    let audit_record = PrivilegedAuditRecord {
        actor_id: actor.actor_id.clone(),
        role: actor.role.clone(),
        action_type: action_type.to_string(),
        parameters: json!({
            "endpoint": endpoint,
            "http_method": http_method,
            "result_count": runs.len(),
        }),
        approval_reference: None,
        timestamp: timestamp_utc.clone(),
        outcome: PrivilegedAuditOutcome::Allow,
        reason_code,
        authentication_outcome: actor.authentication_outcome.as_str().to_string(),
        correlation_id: actor.correlation_id.clone(),
    };
    if let Err(audit_error) = state.audit_appender.append_privileged_audit(audit_record) {
        return audit_append_failure_response(
            audit_error,
            action_type.to_string(),
            actor.actor_id.clone(),
            actor.role.clone(),
            actor.authentication_outcome.as_str(),
            actor.correlation_id.clone(),
            timestamp_utc.clone(),
        );
    }

    (
        StatusCode::OK,
        axum::Json(RecoveryRehearsalQueryResponse {
            status: "accepted",
            action: action_type.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            rehearsals: runs.into_iter().map(recovery_rehearsal_run_item).collect(),
        }),
    )
        .into_response()
}

fn recovery_rehearsal_run_item(run: RestoreRehearsalRunEvidence) -> RecoveryRehearsalRunItem {
    let recommended_next_action = recovery_rehearsal_next_action(&run);
    let failing_checks = run
        .integrity_checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.check_name.clone())
        .collect::<Vec<_>>();
    RecoveryRehearsalRunItem {
        run_id: run.run_id,
        rehearsal_status: run.status.as_str().to_string(),
        reason_code: run.reason_code,
        artifact_id: run.artifact_id,
        correlation_id: run.correlation_id,
        completed_at_utc: run.completed_at_utc,
        failing_checks,
        recommended_next_action,
    }
}

fn recovery_rehearsal_integrity_check_item(
    check: BackupIntegrityCheckItem,
) -> RecoveryRehearsalIntegrityCheckResponseItem {
    RecoveryRehearsalIntegrityCheckResponseItem {
        check_name: check.check_name,
        passed: check.passed,
        reason_code: check.reason_code,
        expected_value: check.expected_value,
        observed_value: check.observed_value,
        details: check.details,
    }
}

fn recovery_deterministic_signature_response(
    signature: domain::recovery_rehearsal::DeterministicReplaySignatureEvidence,
) -> RecoveryDeterministicSignatureResponse {
    RecoveryDeterministicSignatureResponse {
        deterministic_signature: signature.deterministic_signature,
        prior_signature: signature.prior_deterministic_signature,
        deterministic_match: signature.deterministic_match,
        mismatch_summary: signature.mismatch_summary,
    }
}

fn recovery_rehearsal_next_action(run: &RestoreRehearsalRunEvidence) -> String {
    if run.status == RestoreRehearsalStatus::Passed {
        "Restore rehearsal passed with deterministic integrity checks; severe-incident resume may proceed when readiness gates are approved.".to_string()
    } else {
        "Restore rehearsal failed; keep production resume blocked and rerun rehearsal after resolving checksum/reconciliation/deterministic mismatches.".to_string()
    }
}

fn recovery_service_error_response(
    error_code: &'static str,
    message: String,
    field_errors: Vec<domain::recovery::RecoveryValidationIssue>,
    action: &'static str,
    actor: &AuthenticatedActor,
    timestamp_utc: String,
    endpoint: String,
) -> Response {
    (
        recovery_service_error_status(error_code),
        axum::Json(RecoveryServiceErrorResponse {
            error_code,
            reason_code: error_code.to_string(),
            message,
            action: action.to_string(),
            actor_id: actor.actor_id.clone(),
            role: actor.role.clone(),
            correlation_id: actor.correlation_id.clone(),
            timestamp_utc,
            endpoint,
            field_errors: field_errors
                .into_iter()
                .map(|issue| RecoveryFieldError {
                    field: issue.field.to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                })
                .collect(),
        }),
    )
        .into_response()
}

fn recovery_service_error_status(code: &str) -> StatusCode {
    match code {
        code if code == RecoveryReasonCode::InvalidPayload.code() => StatusCode::BAD_REQUEST,
        code if code == RecoveryReasonCode::RehearsalSignatureContractError.code() => {
            StatusCode::BAD_REQUEST
        }
        code if code == RecoveryReasonCode::Unauthorized.code() => StatusCode::FORBIDDEN,
        code if code == RecoveryReasonCode::NotFound.code() => StatusCode::NOT_FOUND,
        code if code == RecoveryReasonCode::StaleEvidence.code()
            || code == RecoveryReasonCode::RehearsalMissingOrFailed.code() =>
        {
            StatusCode::CONFLICT
        }
        code if code == RecoveryReasonCode::DependencyUnavailable.code()
            || code == RecoveryReasonCode::PersistenceUnavailable.code()
            || code == "recovery_gate_query_failed"
            || code == "recovery_gate_row_decode_failed"
            || code == "recovery_gate_runtime_unavailable"
            || code == "restore_rehearsal_query_failed"
            || code == "restore_rehearsal_row_decode_failed"
            || code == "restore_rehearsal_runtime_unavailable" =>
        {
            StatusCode::SERVICE_UNAVAILABLE
        }
        "recovery_gate_constraint_violation" | "restore_rehearsal_constraint_violation" => {
            StatusCode::CONFLICT
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn recovery_gate_outcome_item(
    outcome: domain::recovery::RecoveryGateOutcome,
) -> RecoveryGateOutcomeItem {
    RecoveryGateOutcomeItem {
        gate: outcome.gate.as_str().to_string(),
        passed: outcome.passed,
        reason_code: outcome.reason_code,
        trigger: outcome.trigger,
        context: outcome.context,
        action: outcome.action,
        verification: outcome.verification,
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
pub struct IncidentAlertsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub dependency_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegimeShiftAlertsQuery {
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub start_ts: Option<String>,
    #[serde(default)]
    pub end_ts: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub dependency_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ParticipationGuardrailEventsQuery {
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub start_ts: Option<String>,
    #[serde(default)]
    pub end_ts: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub dependency_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegimeShiftAlertDispatchPayload {
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub issued_at: Option<String>,
    #[serde(default)]
    pub observed_at: Option<String>,
    pub market_id: String,
    pub cluster_id: String,
    pub previous_maker_rebate_bps: f64,
    pub current_maker_rebate_bps: f64,
    pub previous_spread_bps: f64,
    pub current_spread_bps: f64,
    pub previous_eligibility_state: String,
    pub current_eligibility_state: String,
    #[serde(default)]
    pub previous_observed_at: Option<String>,
    #[serde(default)]
    pub previous_liquidity_depth_usd: Option<f64>,
    #[serde(default)]
    pub current_liquidity_depth_usd: Option<f64>,
    #[serde(default)]
    pub previous_projected_exposure_pct_nav: Option<f64>,
    #[serde(default)]
    pub current_projected_exposure_pct_nav: Option<f64>,
    #[serde(default)]
    pub rebate_delta_threshold_bps: Option<f64>,
    #[serde(default)]
    pub spread_widening_threshold_bps: Option<f64>,
    #[serde(default)]
    pub recommended_next_action: Option<String>,
    #[serde(default)]
    pub evidence_link: Option<String>,
    #[serde(default)]
    pub simulate_primary_failure: Option<bool>,
    #[serde(default)]
    pub simulate_fallback_failure: Option<bool>,
    #[serde(default)]
    pub simulated_delivery_delay_seconds: Option<i64>,
    #[serde(default)]
    pub dedupe_window_seconds: Option<i64>,
    #[serde(default)]
    pub primary_channel: Option<String>,
    #[serde(default)]
    pub fallback_channel: Option<String>,
    #[serde(default)]
    pub dependency_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IncidentAlertDispatchPayload {
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub issued_at: Option<String>,
    #[serde(default)]
    pub observed_at: Option<String>,
    #[serde(default)]
    pub impacted_subsystem: Option<String>,
    #[serde(default)]
    pub cause: Option<String>,
    #[serde(default)]
    pub recommended_next_action: Option<String>,
    #[serde(default)]
    pub evidence_link: Option<String>,
    #[serde(default)]
    pub drawdown_pct_of_daily_limit: Option<f64>,
    #[serde(default)]
    pub stream_disconnect_seconds: Option<i64>,
    #[serde(default)]
    pub reconciliation_lag_seconds: Option<i64>,
    #[serde(default)]
    pub stale_data_detected: Option<bool>,
    #[serde(default)]
    pub policy_bypass_attempt: Option<bool>,
    #[serde(default)]
    pub simulate_primary_failure: Option<bool>,
    #[serde(default)]
    pub simulate_fallback_failure: Option<bool>,
    #[serde(default)]
    pub simulated_delivery_delay_seconds: Option<i64>,
    #[serde(default)]
    pub dedupe_window_seconds: Option<i64>,
    #[serde(default)]
    pub primary_channel: Option<String>,
    #[serde(default)]
    pub fallback_channel: Option<String>,
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
pub struct RewardRiskPolicyPayload {
    pub strategy_key: String,
    pub min_reward_per_risk: f64,
}

#[derive(Debug, Deserialize)]
pub struct EmergencyControlPayload {
    #[serde(default)]
    pub audit_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecoveryReadinessEvaluatePayload {
    pub profile_key: String,
    pub reconciliation_run_id: String,
    pub approved_checksum: String,
    pub signoff_intent: String,
    #[serde(default)]
    pub audit_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecoveryResumePayload {
    pub run_id: String,
    #[serde(default)]
    pub resumed_at_utc: Option<String>,
    #[serde(default)]
    pub incident_correlation_id: Option<String>,
    #[serde(default)]
    pub artifact_id: Option<String>,
    #[serde(default)]
    pub incident_severity: Option<String>,
    #[serde(default)]
    pub rehearsal_run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecoveryRehearsalExecutePayload {
    pub artifact_id: String,
    pub artifact_checksum: String,
    pub restore_target: String,
    pub reconciliation_run_id: String,
    pub restore_output: serde_json::Value,
    #[serde(default)]
    pub observed_checksum: Option<String>,
    #[serde(default)]
    pub incident_correlation_id: Option<String>,
    #[serde(default)]
    pub incident_severity: Option<String>,
    #[serde(default)]
    pub audit_reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecoveryRehearsalQuery {
    #[serde(default)]
    pub artifact_id: Option<String>,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RecoveryGateRunQuery {
    #[serde(default)]
    pub correlation_id: Option<String>,
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
pub struct RegimeShiftAlertsQueryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub source: String,
    pub reason_code: String,
    pub data_state: String,
    pub recommended_next_action: String,
    pub alerts: Vec<RegimeShiftAlertItem>,
}

#[derive(Debug, Serialize)]
pub struct RegimeShiftAlertDispatchResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub source: String,
    pub reason_code: String,
    pub duplicate_suppressed_count: usize,
    pub alerts: Vec<RegimeShiftAlertItem>,
}

#[derive(Debug, Serialize)]
pub struct RegimeShiftAlertItem {
    pub alert_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub reason_code: String,
    pub severity: String,
    pub correlation_id: String,
    pub observed_at: String,
    pub issued_at: String,
    pub dispatch_status: String,
    pub dispatch_reason_code: String,
    pub recommended_next_action: String,
    pub evidence_link: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_maker_rebate_bps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_maker_rebate_bps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebate_delta_bps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_spread_bps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_spread_bps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spread_widening_bps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_eligibility_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_eligibility_state: Option<String>,
    pub threshold_rebate_delta_bps: f64,
    pub threshold_spread_widening_bps: f64,
    pub attempts: Vec<IncidentAlertDeliveryAttemptItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_latency_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_used: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ParticipationGuardrailEventsQueryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub source: String,
    pub reason_code: String,
    pub data_state: String,
    pub recommended_next_action: String,
    pub events: Vec<ParticipationGuardrailEventItem>,
}

#[derive(Debug, Serialize)]
pub struct ParticipationGuardrailEventItem {
    pub event_id: String,
    pub guardrail_mode: String,
    pub reason_code: String,
    pub market_id: String,
    pub cluster_id: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub evaluated_at_utc: String,
    pub liquidity_depth_usd: f64,
    pub inactivity_gap_seconds: f64,
    pub threshold_liquidity_depth_usd: f64,
    pub threshold_inactivity_pause_seconds: f64,
    pub threshold_overnight_gap_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal_max_order_size_units: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capped_max_order_size_units: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct IncidentAlertsQueryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub source: String,
    pub reason_code: String,
    pub data_state: String,
    pub recommended_next_action: String,
    pub alerts: Vec<IncidentAlertItem>,
}

#[derive(Debug, Serialize)]
pub struct IncidentAlertDispatchResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub source: String,
    pub reason_code: String,
    pub fallback_used: bool,
    pub dispatch_latency_seconds: i64,
    pub alert: IncidentAlertItem,
}

#[derive(Debug, Serialize)]
pub struct IncidentAlertItem {
    pub alert_id: String,
    pub severity: String,
    pub impacted_subsystem: String,
    pub cause: String,
    pub recommended_next_action: String,
    pub evidence_link: String,
    pub issued_at: String,
    pub correlation_id: String,
    pub reason_code: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<String>,
    pub attempts: Vec<IncidentAlertDeliveryAttemptItem>,
}

#[derive(Debug, Serialize)]
pub struct IncidentAlertDeliveryAttemptItem {
    pub attempt_number: i64,
    pub channel: String,
    pub outcome: String,
    pub reason_code: String,
    pub attempted_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IncidentAlertsServiceErrorResponse {
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
    pub field_errors: Vec<IncidentAlertFieldError>,
}

#[derive(Debug, Serialize)]
pub struct IncidentAlertFieldError {
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
pub struct RewardRiskPolicyDecisionResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub policy_key: String,
    pub strategy_key: String,
    pub min_reward_per_risk: f64,
    pub default_threshold_applied: bool,
    pub actor_id: String,
    pub role: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<RewardRiskSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct RewardRiskServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<RewardRiskFieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<RewardRiskSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct RewardRiskFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RewardRiskSecuritySignal {
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

#[derive(Debug, Serialize)]
pub struct RecoveryReadinessDecisionResponse {
    pub status: &'static str,
    pub action: String,
    pub run_id: String,
    pub readiness_status: String,
    pub reason_code: String,
    pub profile_key: String,
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub evaluated_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumed_at_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness_age_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness_observed_at_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciliation_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciliation_mismatch_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failing_gate_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gate_outcomes: Vec<RecoveryGateOutcomeItem>,
    pub recommended_next_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_reference: Option<String>,
    pub timestamp_utc: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryResumeDecisionResponse {
    pub status: &'static str,
    pub action: String,
    pub run_id: String,
    pub readiness_status: String,
    pub reason_code: String,
    pub verification_reason_code: String,
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub resumed_at_utc: String,
    pub verification_timestamp_utc: String,
    pub timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryRehearsalDecisionResponse {
    pub status: &'static str,
    pub action: String,
    pub run_id: String,
    pub rehearsal_status: String,
    pub reason_code: String,
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub artifact_id: String,
    pub artifact_checksum: String,
    pub observed_checksum: String,
    pub restore_target: String,
    pub reconciliation_run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciliation_mismatch_rate: Option<f64>,
    pub reconciliation_passed: bool,
    pub requested_at_utc: String,
    pub started_at_utc: String,
    pub completed_at_utc: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub integrity_checks: Vec<RecoveryRehearsalIntegrityCheckResponseItem>,
    pub deterministic_signature: RecoveryDeterministicSignatureResponse,
    pub recommended_next_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_reference: Option<String>,
    pub timestamp_utc: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryRehearsalQueryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub rehearsals: Vec<RecoveryRehearsalRunItem>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryRehearsalRunItem {
    pub run_id: String,
    pub rehearsal_status: String,
    pub reason_code: String,
    pub artifact_id: String,
    pub correlation_id: String,
    pub completed_at_utc: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failing_checks: Vec<String>,
    pub recommended_next_action: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryRehearsalIntegrityCheckResponseItem {
    pub check_name: String,
    pub passed: bool,
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_value: Option<String>,
    pub details: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryDeterministicSignatureResponse {
    pub deterministic_signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prior_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deterministic_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mismatch_summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryServiceErrorResponse {
    pub error_code: &'static str,
    pub reason_code: String,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<RecoveryFieldError>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryGateOutcomeItem {
    pub gate: String,
    pub passed: bool,
    pub reason_code: String,
    pub trigger: String,
    pub context: String,
    pub action: String,
    pub verification: String,
}

#[derive(Debug, Deserialize)]
pub struct ReportScheduleMutationPayload {
    pub cadence: Option<String>,
    pub reason_code: Option<String>,
    pub correlation_id: Option<String>,
    pub scheduled_at_utc: Option<String>,
    pub runbook_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportScheduleActionPayload {
    pub reason_code: Option<String>,
    pub correlation_id: Option<String>,
    pub observed_at_utc: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportScheduleRunsQuery {
    pub correlation_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleMutationResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub schedule: ReportScheduleMutationItem,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleMutationItem {
    pub schedule_id: String,
    pub cadence: String,
    pub status: String,
    pub next_run_at_utc: String,
    pub reason_code: String,
    pub runbook_url: String,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleRunHistoryResponse {
    pub status: &'static str,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub runs: Vec<ReportScheduleRunItem>,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleRunItem {
    pub run_id: String,
    pub schedule_id: String,
    pub cadence: String,
    pub window_key: String,
    pub window_started_at_utc: String,
    pub window_ended_at_utc: String,
    pub status: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub run_started_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    pub source_context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_finished_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert_emitted_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runbook_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impacted_system: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleServiceErrorResponse {
    pub error_code: &'static str,
    pub message: String,
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ReportScheduleFieldError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<ReportScheduleSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ReportScheduleSecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u32,
}

#[derive(Debug, Serialize)]
struct ReportScheduleRouteTelemetryEvent<'a> {
    event_name: &'a str,
    signal_name: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u32,
    action: &'a str,
    actor_id: &'a str,
    role: &'a str,
    correlation_id: &'a str,
    schedule_id: &'a str,
    cadence: &'a str,
    status: &'a str,
    reason_code: &'a str,
    timestamp_utc: &'a str,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportExportTriggerPayload {
    pub reason_code: Option<String>,
    pub correlation_id: Option<String>,
    pub as_of_utc: Option<String>,
    #[serde(default)]
    pub unavailable_artifact_types: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportExportIncidentTriggerPayload {
    pub reason_code: Option<String>,
    pub correlation_id: Option<String>,
    pub as_of_utc: Option<String>,
    pub incident_severity: Option<String>,
    pub impacted_system: Option<String>,
    pub runbook_url: Option<String>,
    #[serde(default)]
    pub unavailable_artifact_types: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportExportArtifactsQuery {
    pub correlation_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportExportArtifactQuery {
    pub correlation_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportExportEnvelope<T: Serialize> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub meta: ReportExportMeta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ReportExportEnvelopeError>,
}

#[derive(Debug, Serialize)]
pub struct ReportExportMeta {
    pub action: String,
    pub actor_id: String,
    pub role: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
    pub endpoint: String,
}

#[derive(Debug, Serialize)]
pub struct ReportExportEnvelopeError {
    pub error_code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<ReportExportFieldError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_signal: Option<ReportExportSecuritySignal>,
}

#[derive(Debug, Serialize)]
pub struct ReportExportFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ReportExportSecuritySignal {
    pub name: &'static str,
    pub severity: &'static str,
    pub alert_compatible: bool,
    pub alert_target_seconds: u32,
}

#[derive(Debug, Serialize)]
pub struct ReportExportTriggerData {
    pub job: ReportExportJobItem,
    pub artifact_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_artifact_types: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportExportJobData {
    pub job: ReportExportJobItem,
}

#[derive(Debug, Serialize)]
pub struct ReportExportArtifactsData {
    pub job_id: String,
    pub artifacts: Vec<ReportExportArtifactItem>,
}

#[derive(Debug, Serialize)]
pub struct ReportExportArtifactData {
    pub artifact: ReportExportArtifactItem,
}

#[derive(Debug, Serialize)]
pub struct ReportExportJobItem {
    pub job_id: String,
    pub trigger_source: String,
    pub status: String,
    pub reason_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_checksum: Option<String>,
    pub requested_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_window_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident_severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_metadata: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportExportArtifactItem {
    pub artifact_id: String,
    pub job_id: String,
    pub artifact_type: String,
    pub source: String,
    pub as_of_utc: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub checksum: String,
    pub retrieval_reference: String,
    pub is_available: bool,
    pub updated_at_utc: String,
}

#[derive(Debug, Serialize)]
struct ReportExportRouteTelemetryEvent<'a> {
    event_name: &'a str,
    signal_name: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u32,
    action: &'a str,
    actor_id: &'a str,
    role: &'a str,
    correlation_id: &'a str,
    job_id: &'a str,
    trigger_source: &'a str,
    status: &'a str,
    reason_code: &'a str,
    timestamp_utc: &'a str,
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
    use domain::recovery::{
        RecoveryGateName, RecoveryGateOutcome, RecoveryGateRunEvidence, RecoveryOperatorSignoff,
        RecoveryReadinessStatus, RecoveryReasonCode, RecoveryResumeVerificationEnvelope,
    };
    use domain::recovery_rehearsal::{
        BackupIntegrityCheckItem, DeterministicReplaySignatureEvidence,
        RestoreRehearsalRunEvidence, RestoreRehearsalStatus,
    };
    use domain::reporting_export::{
        ExportArtifactRecord, ExportJobRecord, ReportingExportArtifactType,
        ReportingExportJobState, ReportingExportReasonCode, ReportingExportTriggerSource,
    };
    use domain::reporting_schedule::{ReportingCadence, ReportingRunState};
    use domain::risk::{
        EmergencyControlMode, EmergencyControlReasonCode, EmergencyControlSource,
        EmergencyControlTriggerSource, REWARD_RISK_DEFAULT_THRESHOLD,
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
        recovery::{
            EvaluateRecoveryReadinessInput, ExecuteRecoveryResumeInput,
            ExecuteRestoreRehearsalInput, QueryRecoveryGateRunInput,
            QueryRestoreRehearsalByRunIdInput, QueryRestoreRehearsalsInput, RecoveryOrchestrator,
            RecoveryResumeExecutionEvidence, RecoveryService, RecoveryServiceError,
        },
        reward_risk::{
            ReadRewardRiskPolicyInput, RewardRiskOrchestrator, RewardRiskPolicyEvidence,
            RewardRiskServiceError, UpsertRewardRiskPolicyInput,
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
    use reporting_service::exports::scheduling::{
        PauseReportScheduleInput, QueryReportRunHistoryInput, ReportScheduleMutationEvidence,
        ReportScheduleOrchestrator, ReportSchedulingServiceError, ResumeReportScheduleInput,
        UpsertReportScheduleInput,
    };
    use reporting_service::exports::workflows::{
        DispatchWeeklyExportInput, GetExportArtifactInput, ListExportArtifactsInput,
        QueryExportJobInput, ReportExportJobEvidence, ReportExportOrchestrator,
        ReportExportWorkflowError, TriggerIncidentExportInput, TriggerOnDemandExportInput,
    };
    use std::sync::{Arc, Mutex};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    use tower::ServiceExt;

    fn bearer_token(actor_id: &str, role: &str, expires_unix: i64) -> String {
        format!("Bearer {actor_id}:{role}:{expires_unix}")
    }

    fn unique_correlation_id(prefix: &str) -> String {
        format!(
            "{prefix}-{}",
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        )
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
    struct StubReportScheduleOrchestrator {
        upsert_error: Option<(&'static str, &'static str)>,
        pause_error: Option<(&'static str, &'static str)>,
        resume_error: Option<(&'static str, &'static str)>,
        query_error: Option<(&'static str, &'static str)>,
    }

    impl ReportScheduleOrchestrator for StubReportScheduleOrchestrator {
        fn warmup_status(&self) -> &'static str {
            "report-schedule-stub-ready"
        }

        fn upsert_schedule(
            &self,
            input: UpsertReportScheduleInput,
        ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError> {
            if let Some((code, message)) = self.upsert_error {
                return Err(ReportSchedulingServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(ReportScheduleMutationEvidence {
                schedule_id: input.schedule_id.trim().to_lowercase(),
                cadence: input.cadence.trim().to_lowercase(),
                status: "active".to_string(),
                next_run_at_utc: "2026-04-08T00:00:00Z".to_string(),
                actor_id: input.actor_id,
                actor_role: input.actor_role,
                reason_code: input
                    .reason_code
                    .unwrap_or_else(|| ReportingScheduleReasonCode::Ready.code().to_string()),
                correlation_id: input.correlation_id,
                runbook_url: input.runbook_url.unwrap_or_else(|| {
                    "https://docs.example.com/operations/recurring-report-scheduling".to_string()
                }),
                timestamp_utc: input.timestamp_utc,
            })
        }

        fn pause_schedule(
            &self,
            input: PauseReportScheduleInput,
        ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError> {
            if let Some((code, message)) = self.pause_error {
                return Err(ReportSchedulingServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(ReportScheduleMutationEvidence {
                schedule_id: input.schedule_id.trim().to_lowercase(),
                cadence: "daily".to_string(),
                status: "paused".to_string(),
                next_run_at_utc: "2026-04-08T00:00:00Z".to_string(),
                actor_id: input.actor_id,
                actor_role: input.actor_role,
                reason_code: input.reason_code.unwrap_or_else(|| {
                    ReportingScheduleReasonCode::SchedulePaused
                        .code()
                        .to_string()
                }),
                correlation_id: input.correlation_id,
                runbook_url: "https://docs.example.com/operations/recurring-report-scheduling"
                    .to_string(),
                timestamp_utc: input.timestamp_utc,
            })
        }

        fn resume_schedule(
            &self,
            input: ResumeReportScheduleInput,
        ) -> Result<ReportScheduleMutationEvidence, ReportSchedulingServiceError> {
            if let Some((code, message)) = self.resume_error {
                return Err(ReportSchedulingServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(ReportScheduleMutationEvidence {
                schedule_id: input.schedule_id.trim().to_lowercase(),
                cadence: "daily".to_string(),
                status: "active".to_string(),
                next_run_at_utc: "2026-04-09T00:00:00Z".to_string(),
                actor_id: input.actor_id,
                actor_role: input.actor_role,
                reason_code: input.reason_code.unwrap_or_else(|| {
                    ReportingScheduleReasonCode::ScheduleResumed
                        .code()
                        .to_string()
                }),
                correlation_id: input.correlation_id,
                runbook_url: "https://docs.example.com/operations/recurring-report-scheduling"
                    .to_string(),
                timestamp_utc: input.timestamp_utc,
            })
        }

        fn query_run_history(
            &self,
            input: QueryReportRunHistoryInput,
        ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError> {
            if let Some((code, message)) = self.query_error {
                return Err(ReportSchedulingServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(vec![ReportRunRecord {
                run_id: "report-run::daily::2026-04-07t00-00-00z-2026-04-08t00-00-00z".to_string(),
                schedule_id: input.schedule_id.trim().to_lowercase(),
                cadence: ReportingCadence::Daily,
                window_key: "2026-04-07t00-00-00z-2026-04-08t00-00-00z".to_string(),
                window_started_at_utc: "2026-04-07T00:00:00Z".to_string(),
                window_ended_at_utc: "2026-04-08T00:00:00Z".to_string(),
                status: ReportingRunState::Succeeded,
                reason_code: ReportingScheduleReasonCode::RunSucceeded.code().to_string(),
                correlation_id: input.correlation_id,
                source_context: "reporting-service.scheduler".to_string(),
                actor_id: Some(input.actor_id),
                run_started_at_utc: "2026-04-08T00:00:00Z".to_string(),
                run_finished_at_utc: Some("2026-04-08T00:00:02Z".to_string()),
                alert_emitted_at_utc: None,
                runbook_url: Some(
                    "https://docs.example.com/operations/recurring-report-scheduling".to_string(),
                ),
                impacted_system: Some("reporting-service scheduler".to_string()),
                created_at_utc: input.queried_at_utc.clone(),
                updated_at_utc: input.queried_at_utc,
            }])
        }

        fn process_due_schedules(
            &self,
            _as_of_utc: &str,
        ) -> Result<Vec<ReportRunRecord>, ReportSchedulingServiceError> {
            Ok(Vec::new())
        }
    }

    fn test_app_with_report_schedule_orchestrator(
        report_schedule_orchestrator: Arc<dyn ReportScheduleOrchestrator>,
    ) -> Router {
        test_app_with_state(
            ControlApiState::new(
                Arc::new(GovernanceAuthorizationGuard::new(
                    AuthorizationEvaluator::default(),
                )),
                Arc::new(HeaderTokenAuthenticator),
                Arc::new(CapturingAuditAppender::default()),
            )
            .with_report_schedule_orchestrator(report_schedule_orchestrator),
        )
    }

    #[derive(Debug, Default)]
    struct StubReportExportOrchestrator {
        on_demand_error: Option<(&'static str, &'static str)>,
        incident_error: Option<(&'static str, &'static str)>,
        query_error: Option<(&'static str, &'static str)>,
        list_error: Option<(&'static str, &'static str)>,
        read_error: Option<(&'static str, &'static str)>,
    }

    impl StubReportExportOrchestrator {
        fn sample_job(job_id: String) -> ExportJobRecord {
            ExportJobRecord {
                job_id,
                trigger_source: ReportingExportTriggerSource::OnDemand,
                status: ReportingExportJobState::Succeeded,
                reason_code: ReportingExportReasonCode::JobSucceeded.code().to_string(),
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-export-001".to_string(),
                schedule_id: None,
                schedule_window_key: None,
                report_run_id: None,
                incident_id: None,
                incident_severity: None,
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                started_at_utc: Some("2026-04-07T00:00:01Z".to_string()),
                finished_at_utc: Some("2026-04-07T00:00:02Z".to_string()),
                package_reference: Some(
                    "s3://reporting-exports/export-job-001/manifest.json".to_string(),
                ),
                package_checksum: Some("a".repeat(64)),
                failure_metadata: None,
                created_at_utc: "2026-04-07T00:00:00Z".to_string(),
                updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            }
        }

        fn sample_artifact(
            job_id: String,
            artifact_id: String,
            is_available: bool,
        ) -> ExportArtifactRecord {
            ExportArtifactRecord {
                artifact_id,
                job_id,
                artifact_type: ReportingExportArtifactType::PromotionDecisions,
                source: "governance_promotion_decisions".to_string(),
                as_of_utc: "2026-04-07T00:00:00Z".to_string(),
                reason_code: if is_available {
                    ReportingExportReasonCode::Ready.code().to_string()
                } else {
                    ReportingExportReasonCode::ArtifactUnavailable
                        .code()
                        .to_string()
                },
                correlation_id: "corr-export-001".to_string(),
                checksum: if is_available {
                    "b".repeat(64)
                } else {
                    "unavailable".to_string()
                },
                retrieval_reference: "s3://reporting-exports/export-job-001/promotion.json"
                    .to_string(),
                is_available,
                created_at_utc: "2026-04-07T00:00:00Z".to_string(),
                updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            }
        }
    }

    impl ReportExportOrchestrator for StubReportExportOrchestrator {
        fn warmup_status(&self) -> &'static str {
            "report-export-stub-ready"
        }

        fn trigger_on_demand_export(
            &self,
            _input: TriggerOnDemandExportInput,
        ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
            if let Some((code, message)) = self.on_demand_error {
                return Err(ReportExportWorkflowError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(ReportExportJobEvidence {
                job_id: "export-job-001".to_string(),
                trigger_source: "on_demand".to_string(),
                status: "succeeded".to_string(),
                reason_code: ReportingExportReasonCode::JobSucceeded.code().to_string(),
                artifact_count: 5,
                missing_artifact_types: Vec::new(),
                package_reference: Some(
                    "s3://reporting-exports/export-job-001/manifest.json".to_string(),
                ),
                package_checksum: Some("a".repeat(64)),
                correlation_id: "corr-export-001".to_string(),
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                started_at_utc: Some("2026-04-07T00:00:01Z".to_string()),
                finished_at_utc: Some("2026-04-07T00:00:02Z".to_string()),
                updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            })
        }

        fn trigger_incident_export(
            &self,
            _input: TriggerIncidentExportInput,
        ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
            if let Some((code, message)) = self.incident_error {
                return Err(ReportExportWorkflowError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(ReportExportJobEvidence {
                job_id: "export-job-incident-001".to_string(),
                trigger_source: "incident_triggered".to_string(),
                status: "failed".to_string(),
                reason_code: ReportingExportReasonCode::DependencyUnavailable
                    .code()
                    .to_string(),
                artifact_count: 5,
                missing_artifact_types: vec!["incident_postmortems".to_string()],
                package_reference: Some(
                    "s3://reporting-exports/export-job-incident-001/manifest.json".to_string(),
                ),
                package_checksum: Some("c".repeat(64)),
                correlation_id: "corr-export-incident-001".to_string(),
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                started_at_utc: Some("2026-04-07T00:00:01Z".to_string()),
                finished_at_utc: Some("2026-04-07T00:00:02Z".to_string()),
                updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            })
        }

        fn dispatch_weekly_export(
            &self,
            _input: DispatchWeeklyExportInput,
        ) -> Result<ReportExportJobEvidence, ReportExportWorkflowError> {
            Ok(ReportExportJobEvidence {
                job_id: "export-job-weekly-001".to_string(),
                trigger_source: "scheduled_weekly".to_string(),
                status: "succeeded".to_string(),
                reason_code: ReportingExportReasonCode::JobSucceeded.code().to_string(),
                artifact_count: 5,
                missing_artifact_types: Vec::new(),
                package_reference: Some(
                    "s3://reporting-exports/export-job-weekly-001/manifest.json".to_string(),
                ),
                package_checksum: Some("d".repeat(64)),
                correlation_id: "corr-export-weekly-001".to_string(),
                requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
                started_at_utc: Some("2026-04-07T00:00:01Z".to_string()),
                finished_at_utc: Some("2026-04-07T00:00:02Z".to_string()),
                updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            })
        }

        fn query_export_job(
            &self,
            input: QueryExportJobInput,
        ) -> Result<ExportJobRecord, ReportExportWorkflowError> {
            if let Some((code, message)) = self.query_error {
                return Err(ReportExportWorkflowError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(Self::sample_job(input.job_id))
        }

        fn list_export_artifacts(
            &self,
            input: ListExportArtifactsInput,
        ) -> Result<Vec<ExportArtifactRecord>, ReportExportWorkflowError> {
            if let Some((code, message)) = self.list_error {
                return Err(ReportExportWorkflowError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(vec![
                Self::sample_artifact(
                    input.job_id.clone(),
                    "export-artifact-001".to_string(),
                    true,
                ),
                Self::sample_artifact(input.job_id, "export-artifact-002".to_string(), true),
            ])
        }

        fn get_export_artifact(
            &self,
            input: GetExportArtifactInput,
        ) -> Result<ExportArtifactRecord, ReportExportWorkflowError> {
            if let Some((code, message)) = self.read_error {
                return Err(ReportExportWorkflowError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(Self::sample_artifact(input.job_id, input.artifact_id, true))
        }
    }

    fn test_app_with_report_export_orchestrator(
        report_export_orchestrator: Arc<dyn ReportExportOrchestrator>,
    ) -> Router {
        test_app_with_state(
            ControlApiState::new(
                Arc::new(GovernanceAuthorizationGuard::new(
                    AuthorizationEvaluator::default(),
                )),
                Arc::new(HeaderTokenAuthenticator),
                Arc::new(CapturingAuditAppender::default()),
            )
            .with_report_export_orchestrator(report_export_orchestrator),
        )
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
    struct StubRewardRiskOrchestrator {
        upsert_error: Option<(&'static str, &'static str)>,
        read_error: Option<(&'static str, &'static str)>,
        read_returns_default_threshold: bool,
    }

    impl RewardRiskOrchestrator for StubRewardRiskOrchestrator {
        fn upsert_reward_risk_policy(
            &self,
            input: UpsertRewardRiskPolicyInput,
        ) -> Result<RewardRiskPolicyEvidence, RewardRiskServiceError> {
            if let Some((code, message)) = self.upsert_error {
                return Err(RewardRiskServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: if code == RewardRiskReasonCode::InvalidPayload.code() {
                        vec![RewardRiskValidationIssue {
                            field: "min_reward_per_risk",
                            code: RewardRiskReasonCode::InvalidThreshold.code(),
                            message: "min_reward_per_risk must be greater than or equal to 0"
                                .to_string(),
                        }]
                    } else {
                        Vec::new()
                    },
                });
            }

            Ok(RewardRiskPolicyEvidence {
                policy_key: input.policy_key.trim().to_ascii_lowercase(),
                strategy_key: input.strategy_key.trim().to_ascii_lowercase(),
                min_reward_per_risk: input.min_reward_per_risk,
                actor_id: input.actor_id,
                reason_code: RewardRiskReasonCode::PolicyUpdated.code().to_string(),
                correlation_id: input.correlation_id,
                updated_at_utc: input.updated_at_utc,
                default_threshold_applied: false,
            })
        }

        fn read_reward_risk_policy(
            &self,
            input: ReadRewardRiskPolicyInput,
        ) -> Result<RewardRiskPolicyEvidence, RewardRiskServiceError> {
            if let Some((code, message)) = self.read_error {
                return Err(RewardRiskServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            if self.read_returns_default_threshold {
                return Ok(RewardRiskPolicyEvidence {
                    policy_key: input.policy_key.trim().to_ascii_lowercase(),
                    strategy_key: input.policy_key.trim().to_ascii_lowercase(),
                    min_reward_per_risk: REWARD_RISK_DEFAULT_THRESHOLD,
                    actor_id: input.actor_id,
                    reason_code: RewardRiskReasonCode::DefaultThresholdApplied
                        .code()
                        .to_string(),
                    correlation_id: input.correlation_id,
                    updated_at_utc: input.queried_at_utc,
                    default_threshold_applied: true,
                });
            }

            Ok(RewardRiskPolicyEvidence {
                policy_key: input.policy_key.trim().to_ascii_lowercase(),
                strategy_key: input.policy_key.trim().to_ascii_lowercase(),
                min_reward_per_risk: 1.35,
                actor_id: input.actor_id,
                reason_code: RewardRiskReasonCode::PolicyRead.code().to_string(),
                correlation_id: input.correlation_id,
                updated_at_utc: input.queried_at_utc,
                default_threshold_applied: false,
            })
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

    #[derive(Debug, Default)]
    struct StubRecoveryOrchestrator {
        evaluate_error: Option<(&'static str, &'static str)>,
        resume_error: Option<(&'static str, &'static str)>,
        query_error: Option<(&'static str, &'static str)>,
        rehearsal_execute_error: Option<(&'static str, &'static str)>,
        rehearsal_query_error: Option<(&'static str, &'static str)>,
    }

    impl RecoveryOrchestrator for StubRecoveryOrchestrator {
        fn evaluate_recovery_readiness(
            &self,
            input: EvaluateRecoveryReadinessInput,
        ) -> Result<RecoveryGateRunEvidence, RecoveryServiceError> {
            if let Some((code, message)) = self.evaluate_error {
                return Err(RecoveryServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            Ok(sample_recovery_gate_run(
                format!("run::{}", input.correlation_id),
                input.correlation_id,
                input.profile_key,
                RecoveryReadinessStatus::Approved,
            ))
        }

        fn execute_recovery_resume(
            &self,
            input: ExecuteRecoveryResumeInput,
        ) -> Result<RecoveryResumeExecutionEvidence, RecoveryServiceError> {
            if let Some((code, message)) = self.resume_error {
                return Err(RecoveryServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            let run = sample_recovery_gate_run(
                input.run_id.clone(),
                input.correlation_id,
                "default".to_string(),
                RecoveryReadinessStatus::Approved,
            );
            Ok(RecoveryResumeExecutionEvidence {
                verification: RecoveryResumeVerificationEnvelope {
                    run_id: run.run_id.clone(),
                    correlation_id: run.correlation_id.clone(),
                    readiness_status: RecoveryReadinessStatus::Approved,
                    reason_code: RecoveryReasonCode::ResumeApproved.code().to_string(),
                    verified_at_utc: input.resumed_at_utc,
                },
                run,
            })
        }

        fn execute_restore_rehearsal(
            &self,
            input: ExecuteRestoreRehearsalInput,
        ) -> Result<RestoreRehearsalRunEvidence, RecoveryServiceError> {
            if let Some((code, message)) = self.rehearsal_execute_error {
                return Err(RecoveryServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(sample_restore_rehearsal_run(
                format!("rehearsal::{}", input.correlation_id),
                input.correlation_id,
                input.artifact_id,
                RestoreRehearsalStatus::Passed,
            ))
        }

        fn query_restore_rehearsal_by_run_id(
            &self,
            input: QueryRestoreRehearsalByRunIdInput,
        ) -> Result<RestoreRehearsalRunEvidence, RecoveryServiceError> {
            if let Some((code, message)) = self.rehearsal_query_error {
                return Err(RecoveryServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            Ok(sample_restore_rehearsal_run(
                input.run_id,
                input.correlation_id,
                "artifact::query".to_string(),
                RestoreRehearsalStatus::Passed,
            ))
        }

        fn query_restore_rehearsals(
            &self,
            input: QueryRestoreRehearsalsInput,
        ) -> Result<Vec<RestoreRehearsalRunEvidence>, RecoveryServiceError> {
            if let Some((code, message)) = self.rehearsal_query_error {
                return Err(RecoveryServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }
            let selector_artifact = input
                .artifact_id
                .unwrap_or_else(|| "artifact::query".to_string());
            let selector_correlation = input
                .query_correlation_id
                .unwrap_or_else(|| input.correlation_id.clone());
            let limit = input.limit.unwrap_or(2).clamp(1, 3) as usize;
            Ok((0..limit)
                .map(|index| {
                    sample_restore_rehearsal_run(
                        format!("rehearsal::{}::{index}", selector_correlation),
                        selector_correlation.clone(),
                        selector_artifact.clone(),
                        if index == 0 {
                            RestoreRehearsalStatus::Passed
                        } else {
                            RestoreRehearsalStatus::Failed
                        },
                    )
                })
                .collect())
        }

        fn query_recovery_gate_run(
            &self,
            input: QueryRecoveryGateRunInput,
        ) -> Result<RecoveryGateRunEvidence, RecoveryServiceError> {
            if let Some((code, message)) = self.query_error {
                return Err(RecoveryServiceError {
                    code,
                    message: message.to_string(),
                    field_errors: Vec::new(),
                });
            }

            if let Some(run_id) = input.run_id {
                return Ok(sample_recovery_gate_run(
                    run_id,
                    input.correlation_id,
                    "default".to_string(),
                    RecoveryReadinessStatus::Approved,
                ));
            }
            if let Some(query_correlation_id) = input.query_correlation_id {
                return Ok(sample_recovery_gate_run(
                    format!("run::{query_correlation_id}"),
                    query_correlation_id,
                    "default".to_string(),
                    RecoveryReadinessStatus::Blocked,
                ));
            }

            Err(RecoveryServiceError {
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "query missing run_id and correlation".to_string(),
                field_errors: Vec::new(),
            })
        }
    }

    fn sample_recovery_gate_run(
        run_id: String,
        correlation_id: String,
        profile_key: String,
        readiness_status: RecoveryReadinessStatus,
    ) -> RecoveryGateRunEvidence {
        let approved = readiness_status == RecoveryReadinessStatus::Approved;
        RecoveryGateRunEvidence {
            run_id,
            correlation_id,
            readiness_status,
            reason_code: if approved {
                RecoveryReasonCode::ResumeApproved.code().to_string()
            } else {
                RecoveryReasonCode::ResumeBlocked.code().to_string()
            },
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            profile_key,
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            evaluated_at_utc: "2026-04-06T12:00:01Z".to_string(),
            resumed_at_utc: if approved {
                Some("2026-04-06T12:00:02Z".to_string())
            } else {
                None
            },
            freshness_age_seconds: Some(if approved { 10.0 } else { 45.0 }),
            freshness_observed_at_utc: Some("2026-04-06T12:00:00Z".to_string()),
            reconciliation_run_id: Some("recon-1".to_string()),
            reconciliation_mismatch_rate: Some(if approved { 0.0002 } else { 0.01 }),
            approved_checksum: Some("a".repeat(64)),
            computed_checksum: Some(if approved {
                "a".repeat(64)
            } else {
                "b".repeat(64)
            }),
            signoff: Some(RecoveryOperatorSignoff {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                signoff_intent: "approve controlled recovery".to_string(),
                signed_at_utc: "2026-04-06T12:00:00Z".to_string(),
                audit_reference: Some("arb-2026-0007".to_string()),
            }),
            gate_outcomes: vec![
                RecoveryGateOutcome {
                    gate: RecoveryGateName::Freshness,
                    passed: approved,
                    reason_code: if approved {
                        RecoveryReasonCode::FreshnessPass.code().to_string()
                    } else {
                        RecoveryReasonCode::FreshnessStale.code().to_string()
                    },
                    trigger: "freshness <= 30s".to_string(),
                    context: "freshness telemetry observed".to_string(),
                    action: "apply freshness gate".to_string(),
                    verification: "freshness validated".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::Reconciliation,
                    passed: approved,
                    reason_code: if approved {
                        RecoveryReasonCode::ReconciliationPass.code().to_string()
                    } else {
                        RecoveryReasonCode::ReconciliationMismatch
                            .code()
                            .to_string()
                    },
                    trigger: "reconciliation mismatch < 0.1%".to_string(),
                    context: "reconciliation evidence observed".to_string(),
                    action: "apply reconciliation gate".to_string(),
                    verification: "reconciliation validated".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::RiskChecksum,
                    passed: approved,
                    reason_code: if approved {
                        RecoveryReasonCode::ChecksumMatch.code().to_string()
                    } else {
                        RecoveryReasonCode::ChecksumMismatch.code().to_string()
                    },
                    trigger: "checksum exact match".to_string(),
                    context: "checksum evidence observed".to_string(),
                    action: "apply checksum gate".to_string(),
                    verification: "checksum validated".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::OperatorSignoff,
                    passed: true,
                    reason_code: RecoveryReasonCode::SignoffRecorded.code().to_string(),
                    trigger: "signoff recorded".to_string(),
                    context: "signoff evidence observed".to_string(),
                    action: "apply signoff gate".to_string(),
                    verification: "signoff validated".to_string(),
                },
            ],
            failing_gate_codes: if approved {
                Vec::new()
            } else {
                vec![
                    RecoveryReasonCode::FreshnessStale.code().to_string(),
                    RecoveryReasonCode::ReconciliationMismatch
                        .code()
                        .to_string(),
                    RecoveryReasonCode::ChecksumMismatch.code().to_string(),
                ]
            },
            audit_reference: Some("arb-2026-0007".to_string()),
        }
    }

    fn sample_restore_rehearsal_run(
        run_id: String,
        correlation_id: String,
        artifact_id: String,
        status: RestoreRehearsalStatus,
    ) -> RestoreRehearsalRunEvidence {
        let passed = status == RestoreRehearsalStatus::Passed;
        RestoreRehearsalRunEvidence {
            run_id,
            correlation_id,
            artifact_id,
            artifact_checksum: "a".repeat(64),
            observed_checksum: if passed {
                "a".repeat(64)
            } else {
                "b".repeat(64)
            },
            restore_target: "sandbox-restore-target".to_string(),
            reconciliation_run_id: "recon-1".to_string(),
            reconciliation_mismatch_rate: Some(if passed { 0.0002 } else { 0.01 }),
            reconciliation_passed: passed,
            status,
            reason_code: if passed {
                RecoveryReasonCode::RehearsalSuccess.code().to_string()
            } else {
                RecoveryReasonCode::RehearsalChecksumMismatch
                    .code()
                    .to_string()
            },
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            started_at_utc: "2026-04-06T12:00:01Z".to_string(),
            completed_at_utc: "2026-04-06T12:00:02Z".to_string(),
            integrity_checks: vec![
                BackupIntegrityCheckItem {
                    check_name: "checksum_match".to_string(),
                    passed,
                    reason_code: if passed {
                        RecoveryReasonCode::RehearsalSuccess.code().to_string()
                    } else {
                        RecoveryReasonCode::RehearsalChecksumMismatch
                            .code()
                            .to_string()
                    },
                    expected_value: Some("a".repeat(64)),
                    observed_value: Some(if passed {
                        "a".repeat(64)
                    } else {
                        "b".repeat(64)
                    }),
                    details: if passed {
                        "checksum verification passed".to_string()
                    } else {
                        "checksum mismatch detected".to_string()
                    },
                },
                BackupIntegrityCheckItem {
                    check_name: "reconciliation_sanity".to_string(),
                    passed,
                    reason_code: if passed {
                        RecoveryReasonCode::RehearsalSuccess.code().to_string()
                    } else {
                        RecoveryReasonCode::RehearsalReconciliationSanityFailure
                            .code()
                            .to_string()
                    },
                    expected_value: Some("mismatch_rate < 0.001".to_string()),
                    observed_value: Some(if passed {
                        "0.000200".to_string()
                    } else {
                        "0.010000".to_string()
                    }),
                    details: if passed {
                        "reconciliation sanity passed".to_string()
                    } else {
                        "reconciliation mismatch exceeded threshold".to_string()
                    },
                },
            ],
            deterministic_signature: DeterministicReplaySignatureEvidence {
                deterministic_signature: if passed {
                    "sig::deterministic::ok".to_string()
                } else {
                    "sig::deterministic::mismatch".to_string()
                },
                prior_deterministic_signature: Some("sig::deterministic::ok".to_string()),
                deterministic_match: Some(passed),
                mismatch_summary: if passed {
                    None
                } else {
                    Some("field mismatch in canonical restore output".to_string())
                },
            },
            incident_correlation_id: Some("incident-corr-1".to_string()),
            incident_severity: Some("severity_1".to_string()),
            audit_reference: Some("arb-2026-0007".to_string()),
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
            Arc::new(RecoveryService::default()),
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
            Arc::new(RecoveryService::default()),
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
            Arc::new(RecoveryService::default()),
        ))
    }

    fn test_app_with_reward_risk_orchestrator(
        reward_risk_orchestrator: Arc<dyn RewardRiskOrchestrator>,
    ) -> Router {
        test_app_with_state(
            ControlApiState::with_all_orchestrators(
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
                Arc::new(SafetyControlService::default()),
                Arc::new(RecoveryService::default()),
            )
            .with_reward_risk_orchestrator(reward_risk_orchestrator),
        )
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
            Arc::new(RecoveryService::default()),
        ))
    }

    fn test_app_with_recovery_orchestrator(
        recovery_orchestrator: Arc<dyn RecoveryOrchestrator>,
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
            Arc::new(SafetyControlService::default()),
            recovery_orchestrator,
        ))
    }

    #[tokio::test]
    async fn report_schedule_upsert_endpoint_returns_schedule_evidence() {
        let response = test_app_with_report_schedule_orchestrator(Arc::new(
            StubReportScheduleOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-schedules/report-schedule-daily")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-schedule-upsert-001")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "cadence": "daily",
                        "scheduled_at_utc": "2026-04-07T00:00:00Z",
                        "reason_code": "ready",
                        "runbook_url": "https://docs.example.com/operations/recurring-report-scheduling"
                    })
                    .to_string(),
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
        assert_eq!(payload["action"], "report_schedule_upsert");
        assert_eq!(payload["schedule"]["schedule_id"], "report-schedule-daily");
        assert_eq!(payload["schedule"]["cadence"], "daily");
        assert_eq!(payload["schedule"]["status"], "active");
    }

    #[tokio::test]
    async fn report_schedule_run_history_endpoint_returns_run_rows() {
        let response = test_app_with_report_schedule_orchestrator(Arc::new(
            StubReportScheduleOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-schedules/report-schedule-daily/runs?limit=1")
                .method("GET")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-schedule-runs-001")
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
        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["action"], "report_schedule_run_history_query");
        assert_eq!(payload["runs"].as_array().map(Vec::len), Some(1));
        assert_eq!(payload["runs"][0]["status"], "succeeded");
        assert_eq!(
            payload["runs"][0]["reason_code"],
            ReportingScheduleReasonCode::RunSucceeded.code()
        );
        assert_eq!(payload["runs"][0]["actor_id"], "ops-1");
        assert_eq!(
            payload["runs"][0]["source_context"],
            "reporting-service.scheduler"
        );
        assert_eq!(
            payload["runs"][0]["impacted_system"],
            "reporting-service scheduler"
        );
    }

    #[tokio::test]
    async fn report_schedule_run_history_endpoint_allows_read_only_analytics_roles() {
        let response = test_app_with_report_schedule_orchestrator(Arc::new(
            StubReportScheduleOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-schedules/report-schedule-daily/runs?limit=1")
                .method("GET")
                .header(
                    "authorization",
                    bearer_token("analyst-1", "read_only_analytics", 4_102_444_800),
                )
                .header(
                    "x-correlation-id",
                    "corr-report-schedule-runs-read-only-001",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn report_schedule_pause_endpoint_maps_unauthorized_errors() {
        let response =
            test_app_with_report_schedule_orchestrator(Arc::new(StubReportScheduleOrchestrator {
                pause_error: Some((
                    ReportingScheduleReasonCode::Unauthorized.code(),
                    "forbidden",
                )),
                ..StubReportScheduleOrchestrator::default()
            }))
            .oneshot(
                Request::builder()
                    .uri("/control/report-schedules/report-schedule-daily/pause")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-report-schedule-pause-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "reason_code": "schedule_paused",
                            "observed_at_utc": "2026-04-07T00:05:00Z"
                        })
                        .to_string(),
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
        assert_eq!(
            payload["error_code"],
            ReportingScheduleReasonCode::Unauthorized.code()
        );
        assert_eq!(
            payload["security_signal"]["name"],
            "unauthorized_report_schedule_mutation_attempt_v1"
        );
    }

    #[tokio::test]
    async fn report_schedule_pause_endpoint_rejects_malformed_observed_timestamp() {
        let response = test_app_with_report_schedule_orchestrator(Arc::new(
            StubReportScheduleOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-schedules/report-schedule-daily/pause")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-schedule-pause-002")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "reason_code": "schedule_paused",
                        "observed_at_utc": "2026-04-07T00:05:00+01:00"
                    })
                    .to_string(),
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
            ReportingScheduleReasonCode::InvalidPayload.code()
        );
        assert_eq!(payload["field_errors"][0]["field"], "observed_at_utc");
    }

    #[tokio::test]
    async fn report_export_on_demand_endpoint_returns_enveloped_job_evidence() {
        let response = test_app_with_report_export_orchestrator(Arc::new(
            StubReportExportOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-exports/on-demand")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-export-on-demand-001")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "as_of_utc": "2026-04-07T00:00:00Z",
                        "reason_code": "ready"
                    })
                    .to_string(),
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

        assert_eq!(payload["meta"]["action"], "report_export_on_demand_trigger");
        assert_eq!(payload["data"]["job"]["job_id"], "export-job-001");
        assert_eq!(payload["data"]["job"]["trigger_source"], "on_demand");
        assert_eq!(payload["data"]["artifact_count"], 5);
        assert!(payload.get("error").is_none());
    }

    #[tokio::test]
    async fn report_export_incident_endpoint_rejects_missing_severity() {
        let response = test_app_with_report_export_orchestrator(Arc::new(
            StubReportExportOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-exports/incidents/incident-001")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-export-incident-001")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "as_of_utc": "2026-04-07T00:00:00Z"
                    })
                    .to_string(),
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

        assert_eq!(payload["meta"]["action"], "report_export_incident_trigger");
        assert_eq!(
            payload["error"]["error_code"],
            ReportingExportReasonCode::MissingIncidentContext.code()
        );
        assert_eq!(
            payload["error"]["field_errors"][0]["field"],
            "incident_severity"
        );
    }

    #[tokio::test]
    async fn report_export_job_query_endpoint_allows_read_only_analytics_roles() {
        let response = test_app_with_report_export_orchestrator(Arc::new(
            StubReportExportOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-exports/export-job-001")
                .method("GET")
                .header(
                    "authorization",
                    bearer_token("analyst-1", "read_only_analytics", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-export-query-001")
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

        assert_eq!(payload["meta"]["role"], "read_only_analytics");
        assert_eq!(payload["data"]["job"]["job_id"], "export-job-001");
        assert_eq!(payload["data"]["job"]["status"], "succeeded");
        assert!(payload.get("error").is_none());
    }

    #[tokio::test]
    async fn report_export_artifact_list_endpoint_maps_unauthorized_errors() {
        let response =
            test_app_with_report_export_orchestrator(Arc::new(StubReportExportOrchestrator {
                list_error: Some((ReportingExportReasonCode::Unauthorized.code(), "forbidden")),
                ..StubReportExportOrchestrator::default()
            }))
            .oneshot(
                Request::builder()
                    .uri("/control/report-exports/export-job-001/artifacts?limit=1")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-report-export-artifact-list-001")
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

        assert_eq!(
            payload["error"]["error_code"],
            ReportingExportReasonCode::Unauthorized.code()
        );
        assert_eq!(
            payload["error"]["security_signal"]["name"],
            "unauthorized_report_export_read_attempt_v1"
        );
    }

    #[tokio::test]
    async fn report_export_artifact_read_endpoint_returns_retrievable_metadata() {
        let response = test_app_with_report_export_orchestrator(Arc::new(
            StubReportExportOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/report-exports/export-job-001/artifacts/export-artifact-001")
                .method("GET")
                .header(
                    "authorization",
                    bearer_token("analyst-1", "read_only_analytics", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-report-export-artifact-read-001")
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

        assert_eq!(payload["meta"]["action"], "report_export_artifact_read");
        assert_eq!(
            payload["data"]["artifact"]["artifact_id"],
            "export-artifact-001"
        );
        assert_eq!(
            payload["data"]["artifact"]["artifact_type"],
            "promotion_decisions"
        );
        assert_eq!(payload["data"]["artifact"]["is_available"], true);
        assert!(payload.get("error").is_none());
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
    async fn recovery_readiness_endpoint_returns_gate_evidence() {
        let response = test_app_with_recovery_orchestrator(Arc::new(
            StubRecoveryOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/recovery/readiness/evaluate")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-recovery-evaluate-001")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "profile_key": "default",
                        "reconciliation_run_id": "recon-1",
                        "approved_checksum": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "signoff_intent": "approve controlled recovery",
                        "audit_reference": "arb-2026-0007"
                    })
                    .to_string(),
                ))
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

        assert_eq!(payload["readiness_status"], "approved");
        assert_eq!(payload["reason_code"], "recovery_resume_approved");
        assert_eq!(payload["gate_outcomes"].as_array().map(Vec::len), Some(4));
    }

    #[tokio::test]
    async fn recovery_resume_endpoint_maps_stale_evidence_errors() {
        let response = test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator {
            resume_error: Some((RecoveryReasonCode::StaleEvidence.code(), "resume blocked")),
            ..StubRecoveryOrchestrator::default()
        }))
        .oneshot(
            Request::builder()
                .uri("/control/recovery/resume")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-recovery-resume-001")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "run_id": "run::corr-recovery-resume-001",
                        "resumed_at_utc": "2026-04-06T12:00:05Z"
                    })
                    .to_string(),
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
        assert_eq!(
            payload["error_code"],
            RecoveryReasonCode::StaleEvidence.code()
        );
        assert_eq!(payload["action"], "recovery_resume_execute");
    }

    #[tokio::test]
    async fn recovery_query_endpoint_supports_correlation_lookup() {
        let response =
            test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator::default()))
                .oneshot(
                    Request::builder()
                        .uri("/control/recovery/runs?correlation_id=corr-recovery-query-001")
                        .method("GET")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", "corr-recovery-query-request-001")
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
        assert_eq!(payload["readiness_status"], "blocked");
        assert_eq!(payload["reason_code"], "recovery_resume_blocked");
        assert!(
            payload["failing_gate_codes"]
                .as_array()
                .is_some_and(|values| !values.is_empty())
        );
    }

    #[tokio::test]
    async fn recovery_query_endpoint_measures_p95_latency_within_target_for_repeated_queries() {
        let app =
            test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator::default()));
        let mut latencies = Vec::with_capacity(40);

        for index in 0..40 {
            let correlation = format!("corr-recovery-query-p95-{index:03}");
            let uri = format!("/control/recovery/runs?correlation_id={correlation}");
            let started = std::time::Instant::now();

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri.as_str())
                        .method("GET")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", correlation)
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
            assert_eq!(payload["readiness_status"], "blocked");

            latencies.push(i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX));
        }

        latencies.sort_unstable();
        let p95_index = (latencies.len() * 95).div_ceil(100) - 1;
        let p95_latency_ms = latencies[p95_index];
        assert!(
            p95_latency_ms <= 5_000,
            "recovery query p95 latency should remain <= 5000ms, got {p95_latency_ms}",
        );
    }

    #[tokio::test]
    async fn recovery_rehearsal_execute_endpoint_returns_integrity_evidence() {
        let response = test_app_with_recovery_orchestrator(Arc::new(
            StubRecoveryOrchestrator::default(),
        ))
        .oneshot(
            Request::builder()
                .uri("/control/recovery/rehearsals")
                .method("POST")
                .header(
                    "authorization",
                    bearer_token("ops-1", "operational_control", 4_102_444_800),
                )
                .header("x-correlation-id", "corr-rehearsal-execute-001")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "artifact_id": "artifact-001",
                        "artifact_checksum": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "restore_target": "sandbox-restore-target",
                        "reconciliation_run_id": "recon-1",
                        "restore_output": {
                            "positions": 100,
                            "matched_records": 100
                        },
                        "incident_severity": "severity_1",
                        "audit_reference": "arb-2026-0008"
                    })
                    .to_string(),
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
        assert_eq!(payload["rehearsal_status"], "passed");
        assert_eq!(payload["reason_code"], "recovery_rehearsal_success");
        assert!(
            payload["integrity_checks"]
                .as_array()
                .is_some_and(|checks| checks.len() >= 2)
        );
        assert!(
            payload["deterministic_signature"]["deterministic_signature"]
                .as_str()
                .is_some()
        );
    }

    #[tokio::test]
    async fn recovery_rehearsal_execute_endpoint_maps_json_rejection_to_machine_error() {
        let response =
            test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator::default()))
                .oneshot(
                    Request::builder()
                        .uri("/control/recovery/rehearsals")
                        .method("POST")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", "corr-rehearsal-json-rejection-001")
                        .header("content-type", "application/json")
                        .body(Body::from("{\"artifact_id\":"))
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
            RecoveryReasonCode::InvalidPayload.code()
        );
        assert_eq!(payload["action"], "recovery_rehearsal_execute");
        assert_eq!(payload["endpoint"], "/control/recovery/rehearsals");
    }

    #[tokio::test]
    async fn recovery_rehearsal_query_by_run_endpoint_returns_rehearsal_details() {
        let response =
            test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator::default()))
                .oneshot(
                    Request::builder()
                        .uri("/control/recovery/rehearsals/rehearsal::corr-001::0")
                        .method("GET")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", "corr-rehearsal-query-run-001")
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
        assert_eq!(payload["action"], "recovery_rehearsal_query");
        assert_eq!(payload["rehearsal_status"], "passed");
        assert_eq!(payload["artifact_id"], "artifact::query");
    }

    #[tokio::test]
    async fn recovery_rehearsal_query_endpoint_supports_selector_and_limit() {
        let response =
            test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator::default()))
                .oneshot(
                    Request::builder()
                        .uri("/control/recovery/rehearsals?artifact_id=artifact-001&limit=2")
                        .method("GET")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", "corr-rehearsal-query-selector-001")
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
        assert_eq!(payload["action"], "recovery_rehearsal_query");
        assert_eq!(payload["status"], "accepted");
        assert_eq!(payload["rehearsals"].as_array().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn recovery_rehearsal_query_endpoint_measures_p95_latency_within_target_for_repeated_queries()
     {
        let app =
            test_app_with_recovery_orchestrator(Arc::new(StubRecoveryOrchestrator::default()));
        let mut latencies = Vec::with_capacity(40);

        for index in 0..40 {
            let correlation = format!("corr-rehearsal-query-p95-{index:03}");
            let uri = format!("/control/recovery/rehearsals?correlation_id={correlation}&limit=1");
            let started = std::time::Instant::now();

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri.as_str())
                        .method("GET")
                        .header(
                            "authorization",
                            bearer_token("ops-1", "operational_control", 4_102_444_800),
                        )
                        .header("x-correlation-id", correlation)
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

            latencies.push(i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX));
        }

        latencies.sort_unstable();
        let p95_index = (latencies.len() * 95).div_ceil(100) - 1;
        let p95_latency_ms = latencies[p95_index];
        assert!(
            p95_latency_ms <= 5_000,
            "restore rehearsal query p95 latency should remain <= 5000ms, got {p95_latency_ms}",
        );
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
        let last_rotated_at_utc = (OffsetDateTime::now_utc() - Duration::days(89))
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .expect("rotation timestamp should format");
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
                        serde_json::json!({
                            "credential_scope": "control_api",
                            "credential_reference": "vault://control-api/prod",
                            "last_rotated_at_utc": last_rotated_at_utc,
                            "metadata": {
                                "crypto_posture_verified": true,
                                "runtime_injection_mode": "runtime_only",
                                "provider_ref": "vault://control-api/prod"
                            }
                        })
                        .to_string(),
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
    async fn reward_risk_policy_upsert_route_returns_machine_readable_evidence() {
        let app =
            test_app_with_reward_risk_orchestrator(Arc::new(StubRewardRiskOrchestrator::default()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/reward-risk/policies/strategy-maker-alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-reward-risk-upsert-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "strategy_key": "strategy::maker-alpha",
                            "min_reward_per_risk": 1.3
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
        assert_eq!(payload["policy_key"], "strategy-maker-alpha");
        assert_eq!(payload["reason_code"], "reward_risk_policy_updated");
        assert_eq!(payload["default_threshold_applied"], false);
    }

    #[tokio::test]
    async fn reward_risk_policy_read_route_returns_default_threshold_when_override_missing() {
        let app = test_app_with_reward_risk_orchestrator(Arc::new(StubRewardRiskOrchestrator {
            read_returns_default_threshold: true,
            ..Default::default()
        }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/reward-risk/policies/strategy-maker-alpha")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-reward-risk-read-001")
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
        assert_eq!(
            payload["reason_code"],
            "reward_risk_default_threshold_applied"
        );
        assert_eq!(payload["default_threshold_applied"], true);
        assert_eq!(
            payload["min_reward_per_risk"],
            serde_json::json!(REWARD_RISK_DEFAULT_THRESHOLD)
        );
    }

    #[tokio::test]
    async fn reward_risk_policy_upsert_route_surfaces_validation_field_errors() {
        let app = test_app_with_reward_risk_orchestrator(Arc::new(StubRewardRiskOrchestrator {
            upsert_error: Some(("reward_risk_invalid_payload", "invalid threshold payload")),
            ..Default::default()
        }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/reward-risk/policies/strategy-maker-alpha")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-reward-risk-invalid-001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "strategy_key": "strategy::maker-alpha",
                            "min_reward_per_risk": -0.2
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
        assert_eq!(payload["error_code"], "reward_risk_invalid_payload");
        assert_eq!(payload["action"], "reward_risk_policy_upsert");
        assert_eq!(payload["field_errors"][0]["field"], "min_reward_per_risk");
    }

    #[tokio::test]
    async fn reward_risk_policy_read_route_surfaces_service_unavailable_machine_error() {
        let app = test_app_with_reward_risk_orchestrator(Arc::new(StubRewardRiskOrchestrator {
            read_error: Some((
                "reward_risk_persistence_unavailable",
                "reward-risk repository unavailable",
            )),
            ..Default::default()
        }));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control/reward-risk/policies/strategy-maker-alpha")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-reward-risk-failure-001")
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
        assert_eq!(payload["error_code"], "reward_risk_persistence_unavailable");
        assert_eq!(payload["action"], "reward_risk_policy_read");
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
                    .header(
                        "x-correlation-id",
                        "corr-allocation-policy-invalid-approval-ref-001",
                    )
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
                    .header(
                        "x-correlation-id",
                        "corr-rebalance-invalid-approval-ref-001",
                    )
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
                    .header(
                        "x-correlation-id",
                        "corr-rebalance-execute-invalid-approval-ref-001",
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"approval_reference":"apr-unsafe-client-value"}"#,
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
        assert_eq!(
            payload["rows"][0]["correlation_id"],
            "corr-attribution-route-001"
        );
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
        assert!(
            payload["rows"]
                .as_array()
                .expect("rows should be array")
                .is_empty()
        );
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
        assert!(
            payload["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
        );
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
        assert!(
            payload["events"]
                .as_array()
                .expect("events should be array")
                .is_empty()
        );
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
        assert!(
            payload["field_errors"]
                .as_array()
                .expect("field_errors should be array")
                .iter()
                .any(|item| item["field"] == "start_ts")
        );
        assert!(
            payload["field_errors"]
                .as_array()
                .expect("field_errors should be array")
                .iter()
                .any(|item| item["field"] == "end_ts")
        );
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
    async fn incident_alert_query_route_returns_guidance_rich_alert_payload() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts?limit=10")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-alert-query-001")
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
        assert_eq!(payload["action"], "incident_alerts_query");
        assert_eq!(payload["data_state"], "ready");
        assert!(
            payload["alerts"]
                .as_array()
                .is_some_and(|alerts| !alerts.is_empty())
        );
        assert!(payload["alerts"][0]["recommended_next_action"].is_string());
        assert!(payload["alerts"][0]["evidence_link"].is_string());
        assert!(payload["alerts"][0]["issued_at"].is_string());
        assert!(payload["alerts"][0]["severity"].is_string());
        assert!(payload["alerts"][0]["impacted_subsystem"].is_string());
        assert!(
            payload["alerts"][0]["attempts"]
                .as_array()
                .is_some_and(|attempts| !attempts.is_empty())
        );
    }

    #[tokio::test]
    async fn incident_alert_dispatch_route_returns_delivered_response_within_sla() {
        let correlation_id = unique_correlation_id("corr-alert-dispatch");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "drawdown_pct_of_daily_limit": 82.5,
                            "evidence_link": "https://docs.example.com/operations/severity-alert-delivery#drawdown"
                        }"#,
                    ))
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

        assert_eq!(payload["action"], "incident_alert_dispatch");
        assert_eq!(payload["alert"]["severity"], "critical");
        assert_eq!(payload["alert"]["status"], "delivered");
        assert_eq!(payload["fallback_used"], false);
        assert!(
            payload["dispatch_latency_seconds"]
                .as_i64()
                .expect("dispatch latency should be integer")
                <= 30
        );
        assert_eq!(payload["alert"]["attempts"][0]["outcome"], "delivered");
    }

    #[tokio::test]
    async fn incident_alert_dispatch_route_attempts_fallback_when_primary_fails() {
        let correlation_id = unique_correlation_id("corr-alert-fallback");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "stream_disconnect_seconds": 420,
                            "simulate_primary_failure": true,
                            "evidence_link": "https://docs.example.com/operations/severity-alert-delivery#stream-disconnect"
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        let status = response.status();
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected fallback payload: {payload}"
        );

        assert_eq!(payload["fallback_used"], true);
        assert_eq!(payload["alert"]["status"], "delivered");
        assert_eq!(payload["alert"]["attempts"][0]["outcome"], "failed");
        assert_eq!(payload["alert"]["attempts"][1]["outcome"], "delivered");
    }

    #[tokio::test]
    async fn incident_alert_dispatch_route_surfaces_final_failure_when_fallback_fails() {
        let correlation_id = unique_correlation_id("corr-alert-fallback-fail");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "stream_disconnect_seconds": 420,
                            "simulate_primary_failure": true,
                            "simulate_fallback_failure": true,
                            "evidence_link": "https://docs.example.com/operations/severity-alert-delivery#stream-disconnect"
                        }"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        let status = response.status();
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body should be readable"),
        )
        .expect("payload should be valid json");
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "unexpected fallback-fail payload: {payload}"
        );
        assert_eq!(payload["error_code"], "alert_delivery_fallback_failed");
        assert_eq!(payload["action"], "incident_alert_dispatch");
    }

    #[tokio::test]
    async fn incident_alert_dispatch_route_rejects_boundary_non_trigger_payloads() {
        let correlation_id = unique_correlation_id("corr-alert-boundary");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "drawdown_pct_of_daily_limit": 80.0,
                            "stream_disconnect_seconds": 300,
                            "reconciliation_lag_seconds": 60
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
        assert_eq!(payload["error_code"], "alert_no_trigger");
        assert_eq!(payload["action"], "incident_alert_dispatch");
    }

    #[tokio::test]
    async fn incident_alert_dispatch_route_rejects_malformed_evidence_links() {
        let correlation_id = unique_correlation_id("corr-alert-link-invalid");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "stale_data_detected": true,
                            "evidence_link": "docs/local/runbook"
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
        assert_eq!(payload["error_code"], "alert_invalid_payload");
        assert!(
            payload["field_errors"]
                .as_array()
                .expect("field_errors should be array")
                .iter()
                .any(|item| item["field"] == "evidence_link")
        );
    }

    #[tokio::test]
    async fn incident_alert_query_route_returns_alert_unauthorized_for_denied_reads() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/alerts")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-alert-denied-001")
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
        assert_eq!(payload["error_code"], "alert_unauthorized");
        assert_eq!(payload["action"], "incident_alerts_query");
    }

    #[tokio::test]
    async fn regime_shift_dispatch_route_fails_closed_when_persistence_dependency_is_unavailable() {
        let correlation_id = unique_correlation_id("corr-regime-shift-dispatch");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/regime-shifts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "market_id": "market_yes_no_1",
                            "cluster_id": "cluster_alpha",
                            "previous_maker_rebate_bps": 8.0,
                            "current_maker_rebate_bps": 30.5,
                            "previous_spread_bps": 82.0,
                            "current_spread_bps": 138.5,
                            "previous_eligibility_state": "eligible",
                            "current_eligibility_state": "restricted",
                            "evidence_link": "https://docs.example.com/operations/incentive-regime-shift-alerts#fr40"
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
        assert_eq!(payload["action"], "regime_shift_alert_dispatch");
        assert_eq!(
            payload["error_code"],
            RegimeShiftReasonCode::DependencyUnavailable.code()
        );
    }

    #[tokio::test]
    async fn regime_shift_dispatch_route_rejects_boundary_non_trigger_payloads() {
        let correlation_id = unique_correlation_id("corr-regime-shift-boundary");
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/regime-shifts/dispatch")
                    .method("POST")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", correlation_id.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "market_id": "market_yes_no_1",
                            "cluster_id": "cluster_alpha",
                            "previous_maker_rebate_bps": 10.0,
                            "current_maker_rebate_bps": 30.0,
                            "previous_spread_bps": 100.0,
                            "current_spread_bps": 150.0,
                            "previous_eligibility_state": "eligible",
                            "current_eligibility_state": "eligible"
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
        assert_eq!(payload["error_code"], "alert_no_trigger");
        assert_eq!(payload["action"], "regime_shift_alert_dispatch");
    }

    #[tokio::test]
    async fn regime_shift_query_route_fails_closed_when_persistence_dependency_is_unavailable() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/regime-shifts?market_id=market_yes_no_1&limit=10")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-regime-shift-query-001")
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
        assert_eq!(payload["action"], "regime_shift_alerts_query");
        assert_eq!(
            payload["error_code"],
            RegimeShiftReasonCode::DependencyUnavailable.code()
        );
    }

    #[tokio::test]
    async fn participation_guardrail_query_route_fails_closed_when_persistence_dependency_is_unavailable()
     {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/participation-guardrails?market_id=market_yes_no_1&limit=10")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("ops-1", "operational_control", 4_102_444_800),
                    )
                    .header("x-correlation-id", "corr-participation-guardrail-query-001")
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
        assert_eq!(payload["action"], "participation_guardrail_events_query");
        assert_eq!(
            payload["error_code"],
            ParticipationGuardrailReasonCode::DependencyUnavailable.code()
        );
    }

    #[tokio::test]
    async fn participation_guardrail_query_route_returns_alert_unauthorized_for_denied_reads() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/control/incidents/participation-guardrails")
                    .method("GET")
                    .header(
                        "authorization",
                        bearer_token("reader-1", "read_only_analytics", 4_102_444_800),
                    )
                    .header(
                        "x-correlation-id",
                        "corr-participation-guardrail-denied-001",
                    )
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
        assert_eq!(payload["error_code"], "alert_unauthorized");
        assert_eq!(payload["action"], "participation_guardrail_events_query");
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
