#![cfg_attr(not(test), allow(dead_code))]

use crate::limits::{RuntimeRiskLimitStateReader, evaluate_risk_limit_state_snapshot};
use crate::safe_state::{
    DrawdownProtectiveModeSignal, NoopSafeStateSignals, RuntimeSafeStateSignalPort,
};
use domain::reconciliation::ReconciliationReasonCode;
use domain::risk::{
    FreshnessGateReasonCode, MarketClusterOverride, MarketEligibilityOutcome, MarketPolicyProfile,
    MarketPolicyReasonCode, MarketSnapshot, MarketStreamHealthStatus, PreTradeDecisionOutcome,
    PreTradeGateDecision, PreTradeGateDimension, PreTradeGateResult, PreTradeReasonCode,
    UserStreamReasonCode, adjudicate_pretrade_gate_results, assess_market_stream_health,
    drawdown_stop_triggered, evaluate_market_eligibility,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderIntent {
    pub intent_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OrderIntentGateDecision {
    pub intent_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub allowed: bool,
    pub reason_code: String,
    pub correlation_id: String,
    pub decided_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FreshnessRuntimeState {
    pub pause_active: bool,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuntimeStreamHealthState {
    pub backlog_seconds: f64,
    pub sustained_backlog_seconds: f64,
    pub heartbeat_gap_seconds: f64,
    pub heartbeat_timeout_seconds: f64,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuntimeDrawdownState {
    pub current_drawdown_pct: f64,
    pub configured_stop_threshold_pct: f64,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeStrategyApprovalState {
    pub is_active: bool,
    pub reason_code: String,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTradeDecisionPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl PreTradeDecisionPersistenceError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub trait PreTradeDecisionPersistencePort: Send + Sync {
    fn persist_pretrade_decision<'a>(
        &'a self,
        decision: &'a PreTradeGateDecision,
    ) -> Pin<Box<dyn Future<Output = Result<(), PreTradeDecisionPersistenceError>> + Send + 'a>>;
}

pub trait RuntimePolicyStateReader: Send + Sync {
    fn cluster_override(&self, cluster_id: &str) -> Option<MarketClusterOverride>;
    fn user_stream_auth_block_active(&self) -> bool;
    fn freshness_state(&self) -> Option<FreshnessRuntimeState>;
    fn reconciliation_halt_reason_code(&self) -> Option<String>;
    fn stream_health_state(&self) -> Option<RuntimeStreamHealthState>;
    fn drawdown_state(&self) -> Option<RuntimeDrawdownState>;
    fn strategy_approval_state(&self) -> Option<RuntimeStrategyApprovalState>;
    fn market_snapshot(&self, market_id: &str) -> Option<MarketSnapshot>;
    fn market_policy_profile(&self, cluster_id: &str) -> Option<MarketPolicyProfile>;
}

const FALLBACK_PRETRADE_EVALUATED_AT_UTC: &str = "1970-01-01T00:00:00Z";

#[derive(Debug, Clone, Default)]
pub struct InMemoryRuntimePolicyState {
    overrides: Arc<RwLock<BTreeMap<String, MarketClusterOverride>>>,
    auth_block_active: Arc<AtomicBool>,
    freshness_state: Arc<RwLock<Option<FreshnessRuntimeState>>>,
    reconciliation_halt_reason_code: Arc<RwLock<Option<String>>>,
    stream_health_state: Arc<RwLock<Option<RuntimeStreamHealthState>>>,
    drawdown_state: Arc<RwLock<Option<RuntimeDrawdownState>>>,
    strategy_approval_state: Arc<RwLock<Option<RuntimeStrategyApprovalState>>>,
    market_snapshots: Arc<RwLock<BTreeMap<String, MarketSnapshot>>>,
    market_policy_profiles: Arc<RwLock<BTreeMap<String, MarketPolicyProfile>>>,
}

impl InMemoryRuntimePolicyState {
    pub fn upsert_cluster_override(&self, override_state: MarketClusterOverride) {
        self.overrides
            .write()
            .expect("cluster override runtime map should not be poisoned")
            .insert(
                override_state.cluster_id.trim().to_ascii_lowercase(),
                override_state,
            );
    }

    pub fn set_user_stream_auth_block(&self, is_active: bool) {
        self.auth_block_active.store(is_active, Ordering::SeqCst);
    }

    pub fn set_freshness_pause(&self, is_active: bool, reason_code: &str) {
        let mut freshness_state = self
            .freshness_state
            .write()
            .expect("freshness pause runtime state should not be poisoned");
        *freshness_state = Some(FreshnessRuntimeState {
            pause_active: is_active,
            reason_code: normalize_freshness_pause_reason(is_active, reason_code),
        });
    }

    pub fn clear_freshness_state(&self) {
        *self
            .freshness_state
            .write()
            .expect("freshness runtime state should not be poisoned") = None;
    }

    pub fn set_reconciliation_halt(&self, is_active: bool, reason_code: &str) {
        let mut reconciliation_halt_reason_code = self
            .reconciliation_halt_reason_code
            .write()
            .expect("reconciliation halt runtime state should not be poisoned");
        if is_active {
            *reconciliation_halt_reason_code =
                Some(normalize_reconciliation_halt_reason(reason_code));
        } else {
            *reconciliation_halt_reason_code = None;
        }
    }

    pub fn set_stream_health_state(&self, state: RuntimeStreamHealthState) {
        *self
            .stream_health_state
            .write()
            .expect("stream health runtime state should not be poisoned") = Some(state);
    }

    pub fn clear_stream_health_state(&self) {
        *self
            .stream_health_state
            .write()
            .expect("stream health runtime state should not be poisoned") = None;
    }

    pub fn set_drawdown_state(&self, state: RuntimeDrawdownState) {
        *self
            .drawdown_state
            .write()
            .expect("drawdown runtime state should not be poisoned") = Some(state);
    }

    pub fn clear_drawdown_state(&self) {
        *self
            .drawdown_state
            .write()
            .expect("drawdown runtime state should not be poisoned") = None;
    }

    pub fn set_strategy_approval_state(&self, state: RuntimeStrategyApprovalState) {
        *self
            .strategy_approval_state
            .write()
            .expect("strategy approval runtime state should not be poisoned") = Some(state);
    }

    pub fn clear_strategy_approval_state(&self) {
        *self
            .strategy_approval_state
            .write()
            .expect("strategy approval runtime state should not be poisoned") = None;
    }

    pub fn upsert_market_snapshot(&self, snapshot: MarketSnapshot) {
        self.market_snapshots
            .write()
            .expect("market snapshot runtime map should not be poisoned")
            .insert(snapshot.market_id.trim().to_ascii_lowercase(), snapshot);
    }

    pub fn upsert_market_policy_profile(&self, profile: MarketPolicyProfile) {
        self.market_policy_profiles
            .write()
            .expect("market policy runtime map should not be poisoned")
            .insert(profile.cluster_id.trim().to_ascii_lowercase(), profile);
    }
}

fn normalize_freshness_pause_reason(is_active: bool, reason_code: &str) -> String {
    let parsed = FreshnessGateReasonCode::parse(reason_code)
        .unwrap_or(FreshnessGateReasonCode::StateUnavailable);
    if !is_active {
        return FreshnessGateReasonCode::BoundarySafe.code().to_string();
    }
    let normalized = match parsed {
        FreshnessGateReasonCode::BoundarySafe | FreshnessGateReasonCode::RecoveryConfirmed => {
            FreshnessGateReasonCode::StateUnavailable
        }
        other => other,
    };
    normalized.code().to_string()
}

fn normalize_reconciliation_halt_reason(reason_code: &str) -> String {
    let parsed = ReconciliationReasonCode::parse(reason_code)
        .unwrap_or(ReconciliationReasonCode::CriticalMismatch);
    let normalized = match parsed {
        ReconciliationReasonCode::Matched
        | ReconciliationReasonCode::NonCriticalMismatch
        | ReconciliationReasonCode::Unauthorized
        | ReconciliationReasonCode::InvalidPayload => ReconciliationReasonCode::CriticalMismatch,
        other => other,
    };
    normalized.code().to_string()
}

impl RuntimePolicyStateReader for InMemoryRuntimePolicyState {
    fn cluster_override(&self, cluster_id: &str) -> Option<MarketClusterOverride> {
        self.overrides
            .read()
            .expect("cluster override runtime map should not be poisoned")
            .get(&cluster_id.trim().to_ascii_lowercase())
            .cloned()
    }

    fn user_stream_auth_block_active(&self) -> bool {
        self.auth_block_active.load(Ordering::SeqCst)
    }

    fn freshness_state(&self) -> Option<FreshnessRuntimeState> {
        self.freshness_state
            .read()
            .expect("freshness pause runtime state should not be poisoned")
            .clone()
    }

    fn reconciliation_halt_reason_code(&self) -> Option<String> {
        self.reconciliation_halt_reason_code
            .read()
            .expect("reconciliation halt runtime state should not be poisoned")
            .clone()
    }

    fn stream_health_state(&self) -> Option<RuntimeStreamHealthState> {
        self.stream_health_state
            .read()
            .expect("stream health runtime state should not be poisoned")
            .clone()
    }

    fn drawdown_state(&self) -> Option<RuntimeDrawdownState> {
        self.drawdown_state
            .read()
            .expect("drawdown runtime state should not be poisoned")
            .clone()
    }

    fn strategy_approval_state(&self) -> Option<RuntimeStrategyApprovalState> {
        self.strategy_approval_state
            .read()
            .expect("strategy approval runtime state should not be poisoned")
            .clone()
    }

    fn market_snapshot(&self, market_id: &str) -> Option<MarketSnapshot> {
        self.market_snapshots
            .read()
            .expect("market snapshot runtime map should not be poisoned")
            .get(&market_id.trim().to_ascii_lowercase())
            .cloned()
    }

    fn market_policy_profile(&self, cluster_id: &str) -> Option<MarketPolicyProfile> {
        self.market_policy_profiles
            .read()
            .expect("market policy runtime map should not be poisoned")
            .get(&cluster_id.trim().to_ascii_lowercase())
            .cloned()
    }
}

pub fn evaluate_order_intent_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    intent: &OrderIntent,
) -> OrderIntentGateDecision {
    let (allowed, reason_code) =
        if let Some(freshness_state) = runtime_policy_state.freshness_state() {
            if freshness_state.pause_active {
                (false, freshness_state.reason_code)
            } else if let Some(reconciliation_reason_code) =
                runtime_policy_state.reconciliation_halt_reason_code()
            {
                (false, reconciliation_reason_code)
            } else if runtime_policy_state.user_stream_auth_block_active() {
                (false, UserStreamReasonCode::AuthExpired.code().to_string())
            } else if intent.cluster_id.trim().is_empty() {
                (
                    false,
                    MarketPolicyReasonCode::InvalidClusterId.code().to_string(),
                )
            } else {
                match runtime_policy_state.cluster_override(&intent.cluster_id) {
                    Some(cluster_state) if cluster_state.is_enabled => (
                        true,
                        MarketPolicyReasonCode::MarketEligible.code().to_string(),
                    ),
                    Some(_) => (
                        false,
                        MarketPolicyReasonCode::ClusterDisabled.code().to_string(),
                    ),
                    None => (
                        false,
                        MarketPolicyReasonCode::PolicyStateUnavailable
                            .code()
                            .to_string(),
                    ),
                }
            }
        } else if let Some(reconciliation_reason_code) =
            runtime_policy_state.reconciliation_halt_reason_code()
        {
            (false, reconciliation_reason_code)
        } else if runtime_policy_state.user_stream_auth_block_active() {
            (false, UserStreamReasonCode::AuthExpired.code().to_string())
        } else if intent.cluster_id.trim().is_empty() {
            (
                false,
                MarketPolicyReasonCode::InvalidClusterId.code().to_string(),
            )
        } else {
            match runtime_policy_state.cluster_override(&intent.cluster_id) {
                Some(cluster_state) if cluster_state.is_enabled => (
                    true,
                    MarketPolicyReasonCode::MarketEligible.code().to_string(),
                ),
                Some(_) => (
                    false,
                    MarketPolicyReasonCode::ClusterDisabled.code().to_string(),
                ),
                None => (
                    false,
                    MarketPolicyReasonCode::PolicyStateUnavailable
                        .code()
                        .to_string(),
                ),
            }
        };

    let decision = OrderIntentGateDecision {
        intent_id: intent.intent_id.clone(),
        market_id: intent.market_id.clone(),
        cluster_id: intent.cluster_id.clone(),
        allowed,
        reason_code,
        correlation_id: intent.correlation_id.clone(),
        decided_at_utc: intent.requested_at_utc.clone(),
    };
    emit_order_intent_gate_telemetry(&decision);
    decision
}

pub fn evaluate_order_intent_gate_with_limit_state<S, L>(
    runtime_policy_state: &S,
    runtime_limit_state: &L,
    intent: &OrderIntent,
    profile_key: &str,
) -> OrderIntentGateDecision
where
    S: RuntimePolicyStateReader,
    L: RuntimeRiskLimitStateReader,
{
    let safe_state_port = NoopSafeStateSignals;
    evaluate_order_intent_gate_with_limit_state_and_safe_state(
        runtime_policy_state,
        runtime_limit_state,
        intent,
        profile_key,
        &safe_state_port,
    )
}

pub fn evaluate_order_intent_gate_with_limit_state_and_safe_state<S, L, P>(
    runtime_policy_state: &S,
    runtime_limit_state: &L,
    intent: &OrderIntent,
    profile_key: &str,
    safe_state_port: &P,
) -> OrderIntentGateDecision
where
    S: RuntimePolicyStateReader,
    L: RuntimeRiskLimitStateReader,
    P: RuntimeSafeStateSignalPort,
{
    let pretrade_decision = evaluate_pretrade_gate_decision_with_limit_state_and_safe_state(
        runtime_policy_state,
        runtime_limit_state,
        intent,
        profile_key,
        safe_state_port,
    );
    emit_pretrade_gate_telemetry(&pretrade_decision);
    let decision = to_legacy_gate_decision(&pretrade_decision);
    emit_order_intent_gate_telemetry(&decision);
    decision
}

pub async fn evaluate_order_intent_gate_with_limit_state_and_safe_state_and_persistence<
    S,
    L,
    P,
    D,
>(
    runtime_policy_state: &S,
    runtime_limit_state: &L,
    intent: &OrderIntent,
    profile_key: &str,
    safe_state_port: &P,
    decision_store: &D,
) -> Result<OrderIntentGateDecision, PreTradeDecisionPersistenceError>
where
    S: RuntimePolicyStateReader,
    L: RuntimeRiskLimitStateReader,
    P: RuntimeSafeStateSignalPort,
    D: PreTradeDecisionPersistencePort,
{
    let pretrade_decision = evaluate_pretrade_gate_decision_with_limit_state_and_safe_state(
        runtime_policy_state,
        runtime_limit_state,
        intent,
        profile_key,
        safe_state_port,
    );
    decision_store
        .persist_pretrade_decision(&pretrade_decision)
        .await?;
    emit_pretrade_gate_telemetry(&pretrade_decision);
    let decision = to_legacy_gate_decision(&pretrade_decision);
    emit_order_intent_gate_telemetry(&decision);
    Ok(decision)
}

pub fn evaluate_pretrade_gate_decision_with_limit_state<S, L>(
    runtime_policy_state: &S,
    runtime_limit_state: &L,
    intent: &OrderIntent,
    profile_key: &str,
) -> PreTradeGateDecision
where
    S: RuntimePolicyStateReader,
    L: RuntimeRiskLimitStateReader,
{
    let safe_state_port = NoopSafeStateSignals;
    evaluate_pretrade_gate_decision_with_limit_state_and_safe_state(
        runtime_policy_state,
        runtime_limit_state,
        intent,
        profile_key,
        &safe_state_port,
    )
}

pub fn evaluate_pretrade_gate_decision_with_limit_state_and_safe_state<S, L, P>(
    runtime_policy_state: &S,
    runtime_limit_state: &L,
    intent: &OrderIntent,
    profile_key: &str,
    safe_state_port: &P,
) -> PreTradeGateDecision
where
    S: RuntimePolicyStateReader,
    L: RuntimeRiskLimitStateReader,
    P: RuntimeSafeStateSignalPort,
{
    if validate_pretrade_intent_inputs(intent, profile_key) {
        return build_invalid_payload_decision(intent, profile_key);
    }

    let evaluated_at_utc = intent.requested_at_utc.clone();
    let mut gate_results = Vec::new();

    let freshness_gate = evaluate_pretrade_freshness_gate(runtime_policy_state, &evaluated_at_utc);
    let freshness_passed = freshness_gate.passed;
    gate_results.push(freshness_gate);
    if !freshness_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let stream_health_gate =
        evaluate_pretrade_stream_health_gate(runtime_policy_state, &evaluated_at_utc);
    let stream_health_passed = stream_health_gate.passed;
    gate_results.push(stream_health_gate);
    if !stream_health_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let exposure_gate =
        evaluate_pretrade_exposure_limit_gate(runtime_limit_state, profile_key, &evaluated_at_utc);
    let exposure_passed = exposure_gate.passed;
    gate_results.push(exposure_gate);
    if !exposure_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let reconciliation_gate =
        evaluate_pretrade_reconciliation_gate(runtime_policy_state, &evaluated_at_utc);
    let reconciliation_passed = reconciliation_gate.passed;
    gate_results.push(reconciliation_gate);
    if !reconciliation_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let user_stream_gate =
        evaluate_pretrade_user_stream_auth_gate(runtime_policy_state, &evaluated_at_utc);
    let user_stream_passed = user_stream_gate.passed;
    gate_results.push(user_stream_gate);
    if !user_stream_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let drawdown_gate = evaluate_pretrade_drawdown_gate(runtime_policy_state, &evaluated_at_utc);
    let drawdown_passed = drawdown_gate.passed;
    let protective_mode_active =
        drawdown_gate.reason_code == PreTradeReasonCode::DrawdownStopTriggered.code();
    gate_results.push(drawdown_gate);
    if !drawdown_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            protective_mode_active,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let strategy_approval_gate =
        evaluate_pretrade_strategy_approval_gate(runtime_policy_state, &evaluated_at_utc);
    let strategy_approval_passed = strategy_approval_gate.passed;
    gate_results.push(strategy_approval_gate);
    if !strategy_approval_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    let venue_gate = evaluate_pretrade_venue_eligibility_gate(runtime_policy_state, intent);
    let venue_passed = venue_gate.passed;
    gate_results.push(venue_gate);
    if !venue_passed {
        return finalize_pretrade_decision(
            intent,
            profile_key,
            gate_results,
            false,
            safe_state_port,
            &evaluated_at_utc,
        );
    }

    finalize_pretrade_decision(
        intent,
        profile_key,
        gate_results,
        false,
        safe_state_port,
        &evaluated_at_utc,
    )
}

fn validate_pretrade_intent_inputs(intent: &OrderIntent, profile_key: &str) -> bool {
    intent.intent_id.trim().is_empty()
        || intent.market_id.trim().is_empty()
        || intent.cluster_id.trim().is_empty()
        || intent.correlation_id.trim().is_empty()
        || profile_key.trim().is_empty()
        || !is_utc_timestamp(&intent.requested_at_utc)
}

fn is_utc_timestamp(value: &str) -> bool {
    let Ok(parsed) =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
    else {
        return false;
    };
    parsed.offset() == time::UtcOffset::UTC
}

fn build_invalid_payload_decision(intent: &OrderIntent, profile_key: &str) -> PreTradeGateDecision {
    let fallback_timestamp = normalized_pretrade_evaluated_at_utc(&intent.requested_at_utc);
    finalize_pretrade_decision(
        intent,
        profile_key,
        vec![build_pretrade_gate_result(
            PreTradeGateDimension::Freshness,
            false,
            PreTradeReasonCode::InvalidPayload,
            &fallback_timestamp,
        )],
        false,
        &NoopSafeStateSignals,
        &fallback_timestamp,
    )
}

fn finalize_pretrade_decision<P: RuntimeSafeStateSignalPort>(
    intent: &OrderIntent,
    profile_key: &str,
    gate_results: Vec<PreTradeGateResult>,
    protective_mode_active: bool,
    safe_state_port: &P,
    evaluated_at_utc: &str,
) -> PreTradeGateDecision {
    let intent_id = sanitize_identifier(&intent.intent_id, "unknown_intent");
    let market_id = sanitize_identifier(&intent.market_id, "unknown_market");
    let cluster_id = sanitize_identifier(&intent.cluster_id, "unknown_cluster");
    let normalized_profile_key = sanitize_identifier(profile_key, "unknown_profile");
    let correlation_id = sanitize_identifier(&intent.correlation_id, "unknown_correlation");
    let normalized_evaluated_at_utc = normalized_pretrade_evaluated_at_utc(evaluated_at_utc);

    let decision = match adjudicate_pretrade_gate_results(
        &intent_id,
        &market_id,
        &cluster_id,
        &normalized_profile_key,
        &correlation_id,
        &normalized_evaluated_at_utc,
        gate_results,
        protective_mode_active,
    ) {
        Ok(decision) => decision,
        Err(error) => {
            println!(
                "risk-engine pre-trade adjudication contract violation; falling back fail-closed: {} ({})",
                error.code, error.message
            );
            build_fail_closed_pretrade_decision(
                &intent_id,
                &market_id,
                &cluster_id,
                &normalized_profile_key,
                &correlation_id,
                &normalized_evaluated_at_utc,
            )
        }
    };

    if decision.protective_mode_active {
        safe_state_port.signal_drawdown_protective_mode(DrawdownProtectiveModeSignal {
            active: true,
            reason_code: decision.reason_code.clone(),
            correlation_id: decision.correlation_id.clone(),
            triggered_at_utc: decision.evaluated_at_utc.clone(),
        });
    }
    decision
}

fn sanitize_identifier(value: &str, fallback: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn normalized_pretrade_evaluated_at_utc(value: &str) -> String {
    if is_utc_timestamp(value) {
        value.to_string()
    } else {
        FALLBACK_PRETRADE_EVALUATED_AT_UTC.to_string()
    }
}

fn build_fail_closed_pretrade_decision(
    intent_id: &str,
    market_id: &str,
    cluster_id: &str,
    profile_key: &str,
    correlation_id: &str,
    evaluated_at_utc: &str,
) -> PreTradeGateDecision {
    PreTradeGateDecision {
        decision_id: format!(
            "pretrade::{}::{}",
            intent_id,
            compact_timestamp_token(evaluated_at_utc)
        ),
        intent_id: intent_id.to_string(),
        market_id: market_id.to_string(),
        cluster_id: cluster_id.to_string(),
        profile_key: profile_key.to_string(),
        outcome: PreTradeDecisionOutcome::Deny,
        reason_code: PreTradeReasonCode::InvalidPayload.code().to_string(),
        protective_mode_active: false,
        gate_results: vec![build_pretrade_gate_result(
            PreTradeGateDimension::Freshness,
            false,
            PreTradeReasonCode::InvalidPayload,
            evaluated_at_utc,
        )],
        correlation_id: correlation_id.to_string(),
        evaluated_at_utc: evaluated_at_utc.to_string(),
    }
}

fn compact_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn evaluate_pretrade_freshness_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    match runtime_policy_state.freshness_state() {
        None => build_pretrade_gate_result(
            PreTradeGateDimension::Freshness,
            false,
            PreTradeReasonCode::FreshnessStateUnavailable,
            evaluated_at_utc,
        ),
        Some(state) if state.pause_active => build_pretrade_gate_result(
            PreTradeGateDimension::Freshness,
            false,
            map_freshness_reason_to_pretrade(&state.reason_code),
            evaluated_at_utc,
        ),
        Some(_) => build_pretrade_gate_result(
            PreTradeGateDimension::Freshness,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        ),
    }
}

fn map_freshness_reason_to_pretrade(reason_code: &str) -> PreTradeReasonCode {
    match FreshnessGateReasonCode::parse(reason_code) {
        Ok(FreshnessGateReasonCode::StaleBreach) => PreTradeReasonCode::FreshnessStaleBreach,
        _ => PreTradeReasonCode::FreshnessStateUnavailable,
    }
}

fn evaluate_pretrade_stream_health_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    let Some(metrics) = runtime_policy_state.stream_health_state() else {
        return build_pretrade_gate_result(
            PreTradeGateDimension::StreamHealth,
            false,
            PreTradeReasonCode::StreamHealthStateUnavailable,
            evaluated_at_utc,
        );
    };
    if !is_utc_timestamp(&metrics.observed_at_utc) {
        return build_pretrade_gate_result(
            PreTradeGateDimension::StreamHealth,
            false,
            PreTradeReasonCode::StreamHealthStateUnavailable,
            evaluated_at_utc,
        );
    }

    match assess_market_stream_health(
        metrics.backlog_seconds,
        metrics.sustained_backlog_seconds,
        metrics.heartbeat_gap_seconds,
        metrics.heartbeat_timeout_seconds,
    ) {
        Ok(assessment) if assessment.status == MarketStreamHealthStatus::Healthy => {
            build_pretrade_gate_result(
                PreTradeGateDimension::StreamHealth,
                true,
                PreTradeReasonCode::Pass,
                evaluated_at_utc,
            )
        }
        Ok(_) => build_pretrade_gate_result(
            PreTradeGateDimension::StreamHealth,
            false,
            PreTradeReasonCode::StreamHealthDegraded,
            evaluated_at_utc,
        ),
        Err(_) => build_pretrade_gate_result(
            PreTradeGateDimension::StreamHealth,
            false,
            PreTradeReasonCode::StreamHealthStateUnavailable,
            evaluated_at_utc,
        ),
    }
}

fn evaluate_pretrade_exposure_limit_gate<L: RuntimeRiskLimitStateReader>(
    runtime_limit_state: &L,
    profile_key: &str,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    let snapshot =
        evaluate_risk_limit_state_snapshot(runtime_limit_state, profile_key, evaluated_at_utc);
    if snapshot.available {
        build_pretrade_gate_result(
            PreTradeGateDimension::ExposureLimitState,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        )
    } else {
        build_pretrade_gate_result(
            PreTradeGateDimension::ExposureLimitState,
            false,
            PreTradeReasonCode::RiskLimitStateUnavailable,
            evaluated_at_utc,
        )
    }
}

fn evaluate_pretrade_reconciliation_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    if runtime_policy_state
        .reconciliation_halt_reason_code()
        .is_some()
    {
        build_pretrade_gate_result(
            PreTradeGateDimension::ReconciliationHalt,
            false,
            PreTradeReasonCode::ReconciliationCriticalHalt,
            evaluated_at_utc,
        )
    } else {
        build_pretrade_gate_result(
            PreTradeGateDimension::ReconciliationHalt,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        )
    }
}

fn evaluate_pretrade_user_stream_auth_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    if runtime_policy_state.user_stream_auth_block_active() {
        build_pretrade_gate_result(
            PreTradeGateDimension::UserStreamAuth,
            false,
            PreTradeReasonCode::UserStreamAuthExpired,
            evaluated_at_utc,
        )
    } else {
        build_pretrade_gate_result(
            PreTradeGateDimension::UserStreamAuth,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        )
    }
}

fn evaluate_pretrade_drawdown_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    let Some(drawdown_state) = runtime_policy_state.drawdown_state() else {
        return build_pretrade_gate_result(
            PreTradeGateDimension::DrawdownStop,
            false,
            PreTradeReasonCode::DrawdownStateUnavailable,
            evaluated_at_utc,
        );
    };
    if !is_utc_timestamp(&drawdown_state.observed_at_utc) {
        return build_pretrade_gate_result(
            PreTradeGateDimension::DrawdownStop,
            false,
            PreTradeReasonCode::DrawdownStateUnavailable,
            evaluated_at_utc,
        );
    }

    match drawdown_stop_triggered(
        drawdown_state.current_drawdown_pct,
        drawdown_state.configured_stop_threshold_pct,
    ) {
        Ok(true) => build_pretrade_gate_result(
            PreTradeGateDimension::DrawdownStop,
            false,
            PreTradeReasonCode::DrawdownStopTriggered,
            evaluated_at_utc,
        ),
        Ok(false) => build_pretrade_gate_result(
            PreTradeGateDimension::DrawdownStop,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        ),
        Err(_) => build_pretrade_gate_result(
            PreTradeGateDimension::DrawdownStop,
            false,
            PreTradeReasonCode::DrawdownStateUnavailable,
            evaluated_at_utc,
        ),
    }
}

fn evaluate_pretrade_strategy_approval_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    let Some(strategy_approval_state) = runtime_policy_state.strategy_approval_state() else {
        return build_pretrade_gate_result(
            PreTradeGateDimension::StrategyApproval,
            false,
            PreTradeReasonCode::StrategyApprovalUnavailable,
            evaluated_at_utc,
        );
    };
    if !is_utc_timestamp(&strategy_approval_state.observed_at_utc) {
        return build_pretrade_gate_result(
            PreTradeGateDimension::StrategyApproval,
            false,
            PreTradeReasonCode::StrategyApprovalUnavailable,
            evaluated_at_utc,
        );
    }
    if strategy_approval_state.is_active {
        build_pretrade_gate_result(
            PreTradeGateDimension::StrategyApproval,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        )
    } else {
        build_pretrade_gate_result(
            PreTradeGateDimension::StrategyApproval,
            false,
            PreTradeReasonCode::StrategyApprovalRequired,
            evaluated_at_utc,
        )
    }
}

fn evaluate_pretrade_venue_eligibility_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    intent: &OrderIntent,
) -> PreTradeGateResult {
    let evaluated_at_utc = intent.requested_at_utc.as_str();
    let snapshot = runtime_policy_state.market_snapshot(&intent.market_id);
    let profile = runtime_policy_state.market_policy_profile(&intent.cluster_id);
    let cluster_override = runtime_policy_state.cluster_override(&intent.cluster_id);
    let eligibility = evaluate_market_eligibility(
        snapshot.as_ref(),
        profile.as_ref(),
        cluster_override.as_ref(),
        &intent.correlation_id,
        evaluated_at_utc,
    );

    if eligibility.outcome == MarketEligibilityOutcome::Tradable {
        build_pretrade_gate_result(
            PreTradeGateDimension::VenueEligibility,
            true,
            PreTradeReasonCode::Pass,
            evaluated_at_utc,
        )
    } else {
        build_pretrade_gate_result(
            PreTradeGateDimension::VenueEligibility,
            false,
            map_market_eligibility_reason_to_pretrade(&eligibility.reason_code),
            evaluated_at_utc,
        )
    }
}

fn map_market_eligibility_reason_to_pretrade(reason_code: &str) -> PreTradeReasonCode {
    match MarketPolicyReasonCode::parse(reason_code) {
        Ok(
            MarketPolicyReasonCode::PolicyStateUnavailable
            | MarketPolicyReasonCode::UnknownCluster
            | MarketPolicyReasonCode::InvalidPayload
            | MarketPolicyReasonCode::InvalidClusterId
            | MarketPolicyReasonCode::InvalidThreshold
            | MarketPolicyReasonCode::PersistenceUnavailable,
        ) => PreTradeReasonCode::VenueEligibilityUnavailable,
        Ok(MarketPolicyReasonCode::MarketEligible) => PreTradeReasonCode::Pass,
        Ok(_) => PreTradeReasonCode::VenueIneligible,
        Err(_) => PreTradeReasonCode::VenueEligibilityUnavailable,
    }
}

fn build_pretrade_gate_result(
    gate: PreTradeGateDimension,
    passed: bool,
    reason_code: PreTradeReasonCode,
    evaluated_at_utc: &str,
) -> PreTradeGateResult {
    PreTradeGateResult {
        gate,
        passed,
        reason_code: reason_code.code().to_string(),
        evaluated_at_utc: evaluated_at_utc.to_string(),
    }
}

fn to_legacy_gate_decision(pretrade_decision: &PreTradeGateDecision) -> OrderIntentGateDecision {
    OrderIntentGateDecision {
        intent_id: pretrade_decision.intent_id.clone(),
        market_id: pretrade_decision.market_id.clone(),
        cluster_id: pretrade_decision.cluster_id.clone(),
        allowed: matches!(pretrade_decision.outcome, PreTradeDecisionOutcome::Allow),
        reason_code: pretrade_decision.reason_code.clone(),
        correlation_id: pretrade_decision.correlation_id.clone(),
        decided_at_utc: pretrade_decision.evaluated_at_utc.clone(),
    }
}

fn emit_pretrade_gate_telemetry(decision: &PreTradeGateDecision) {
    let event = PreTradeGateTelemetryEvent {
        event_name: "risk_pretrade_gate_decision_v1",
        action: "evaluate_pretrade_gate_pipeline",
        decision_id: &decision.decision_id,
        intent_id: &decision.intent_id,
        market_id: &decision.market_id,
        cluster_id: &decision.cluster_id,
        profile_key: &decision.profile_key,
        outcome: decision.outcome.as_str(),
        reason_code: &decision.reason_code,
        protective_mode_active: decision.protective_mode_active,
        correlation_id: &decision.correlation_id,
        timestamp_utc: &decision.evaluated_at_utc,
        gate_results: &decision.gate_results,
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("pre-trade gate telemetry should serialize")
    );
}

fn emit_order_intent_gate_telemetry(decision: &OrderIntentGateDecision) {
    let event = OrderIntentGateTelemetryEvent {
        event_name: "risk_order_intent_gate_decision_v1",
        action: "evaluate_order_intent_cluster_toggle",
        intent_id: &decision.intent_id,
        market_id: &decision.market_id,
        cluster_id: &decision.cluster_id,
        outcome: if decision.allowed { "allow" } else { "deny" },
        reason_code: &decision.reason_code,
        correlation_id: &decision.correlation_id,
        timestamp_utc: &decision.decided_at_utc,
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("risk gate telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct PreTradeGateTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    decision_id: &'a str,
    intent_id: &'a str,
    market_id: &'a str,
    cluster_id: &'a str,
    profile_key: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    protective_mode_active: bool,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    gate_results: &'a [PreTradeGateResult],
}

#[derive(Debug, Serialize)]
struct OrderIntentGateTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    intent_id: &'a str,
    market_id: &'a str,
    cluster_id: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::InMemoryRiskLimitState;
    use crate::safe_state::InMemorySafeStateSignals;
    use domain::risk::{
        MarketPolicyProfile, MarketSnapshot, PreTradeDecisionOutcome, PreTradeGateDimension,
        PreTradeReasonCode,
        RiskLimitProfileStatus, RiskLimitProfileVersion, RiskLimitReasonCode, RiskLimitScope,
        RiskScopeLimit,
    };
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    fn sample_intent() -> OrderIntent {
        OrderIntent {
            intent_id: "intent-1".to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            correlation_id: "corr-intent-001".to_string(),
            requested_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_override(enabled: bool) -> MarketClusterOverride {
        MarketClusterOverride {
            cluster_id: "cluster_alpha".to_string(),
            is_enabled: enabled,
            reason_code: if enabled {
                MarketPolicyReasonCode::ClusterEnabled.code().to_string()
            } else {
                MarketPolicyReasonCode::ClusterDisabledByOperator
                    .code()
                    .to_string()
            },
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-toggle-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_limit_scope(
        scope: RiskLimitScope,
        scope_id: &str,
        max_notional_usd: f64,
        max_inventory_units: f64,
        max_concentration_pct_nav: f64,
    ) -> RiskScopeLimit {
        RiskScopeLimit {
            scope,
            scope_id: scope_id.to_string(),
            max_notional_usd,
            max_inventory_units,
            max_concentration_pct_nav,
        }
    }

    fn sample_active_limit_profile(updated_at_utc: &str) -> RiskLimitProfileVersion {
        RiskLimitProfileVersion {
            profile_key: "default".to_string(),
            version: 1,
            portfolio: sample_limit_scope(
                RiskLimitScope::Portfolio,
                "portfolio::default",
                1000.0,
                800.0,
                40.0,
            ),
            market: sample_limit_scope(
                RiskLimitScope::Market,
                "market::sports",
                600.0,
                400.0,
                30.0,
            ),
            strategy: sample_limit_scope(
                RiskLimitScope::Strategy,
                "strategy::maker",
                300.0,
                200.0,
                20.0,
            ),
            status: RiskLimitProfileStatus::Active,
            approval_reference: Some("apr_risk_limit_bootstrap".to_string()),
            actor_id: "ops-1".to_string(),
            reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: updated_at_utc.to_string(),
        }
    }

    fn sample_market_policy_profile() -> MarketPolicyProfile {
        MarketPolicyProfile {
            profile_id: "policy_cluster_alpha".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            min_liquidity_usd: 1000.0,
            max_spread_bps: 3.5,
            min_reward_score: 0.5,
            max_exposure_pct_nav: 20.0,
            is_active: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_market_snapshot() -> MarketSnapshot {
        MarketSnapshot {
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            liquidity_depth_usd: 1200.0,
            spread_bps: 2.0,
            reward_score: 0.8,
            projected_exposure_pct_nav: 12.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn seed_healthy_pretrade_policy_state(runtime_state: &InMemoryRuntimePolicyState) {
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_user_stream_auth_block(false);
        runtime_state.set_freshness_pause(false, FreshnessGateReasonCode::BoundarySafe.code());
        runtime_state
            .set_reconciliation_halt(false, ReconciliationReasonCode::CriticalMismatch.code());
        runtime_state.set_stream_health_state(RuntimeStreamHealthState {
            backlog_seconds: 4.0,
            sustained_backlog_seconds: 0.0,
            heartbeat_gap_seconds: 3.0,
            heartbeat_timeout_seconds: 15.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });
        runtime_state.set_drawdown_state(RuntimeDrawdownState {
            current_drawdown_pct: 4.0,
            configured_stop_threshold_pct: 8.5,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });
        runtime_state.set_strategy_approval_state(RuntimeStrategyApprovalState {
            is_active: true,
            reason_code: "approval_granted".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });
        runtime_state.upsert_market_policy_profile(sample_market_policy_profile());
        runtime_state.upsert_market_snapshot(sample_market_snapshot());
    }

    #[test]
    fn order_intent_gate_allows_when_cluster_enabled() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));

        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(decision.allowed);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }

    #[test]
    fn order_intent_gate_blocks_new_intents_after_runtime_toggle_without_restart() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));

        let first = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(first.allowed);

        runtime_state.upsert_cluster_override(sample_override(false));
        let second = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(!second.allowed);
        assert_eq!(
            second.reason_code,
            MarketPolicyReasonCode::ClusterDisabled.code()
        );
    }

    #[test]
    fn order_intent_gate_is_fail_closed_when_cluster_state_is_missing() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn order_intent_gate_denies_when_user_stream_auth_block_is_active() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_user_stream_auth_block(true);

        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            UserStreamReasonCode::AuthExpired.code()
        );
    }

    #[test]
    fn order_intent_gate_recovery_unblocks_after_auth_block_clears() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_user_stream_auth_block(true);
        let blocked = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(!blocked.allowed);

        runtime_state.set_user_stream_auth_block(false);
        let recovered = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(recovered.allowed);
        assert_eq!(
            recovered.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }

    #[test]
    fn order_intent_gate_denies_when_freshness_pause_is_active() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_freshness_pause(true, FreshnessGateReasonCode::StaleBreach.code());

        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            FreshnessGateReasonCode::StaleBreach.code()
        );
    }

    #[test]
    fn order_intent_gate_recovery_unblocks_after_freshness_pause_clears() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_freshness_pause(true, FreshnessGateReasonCode::StaleBreach.code());

        let blocked = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(!blocked.allowed);

        runtime_state.set_freshness_pause(false, FreshnessGateReasonCode::BoundarySafe.code());
        let recovered = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(recovered.allowed);
        assert_eq!(
            recovered.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }

    #[test]
    fn order_intent_gate_normalizes_non_pause_freshness_reasons_when_paused() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_freshness_pause(true, FreshnessGateReasonCode::BoundarySafe.code());

        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            FreshnessGateReasonCode::StateUnavailable.code()
        );
    }

    #[test]
    fn order_intent_gate_denies_when_reconciliation_halt_is_active() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state
            .set_reconciliation_halt(true, ReconciliationReasonCode::CriticalMismatch.code());

        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            ReconciliationReasonCode::CriticalMismatch.code()
        );
    }

    #[test]
    fn order_intent_gate_recovery_unblocks_after_reconciliation_halt_clears() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state
            .set_reconciliation_halt(true, ReconciliationReasonCode::CriticalMismatch.code());

        let blocked = evaluate_order_intent_gate(&runtime_state, &sample_intent());
        assert!(!blocked.allowed);

        runtime_state
            .set_reconciliation_halt(false, ReconciliationReasonCode::CriticalMismatch.code());
        let recovered = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(recovered.allowed);
        assert_eq!(
            recovered.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }

    #[test]
    fn order_intent_gate_normalizes_non_halt_reconciliation_reasons_when_active() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        runtime_state.upsert_cluster_override(sample_override(true));
        runtime_state.set_reconciliation_halt(true, ReconciliationReasonCode::Matched.code());

        let decision = evaluate_order_intent_gate(&runtime_state, &sample_intent());

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            ReconciliationReasonCode::CriticalMismatch.code()
        );
    }

    #[test]
    fn order_intent_gate_with_limit_state_fails_closed_when_limit_state_unavailable() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);

        let limit_state = InMemoryRiskLimitState::default();
        let decision = evaluate_order_intent_gate_with_limit_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
        );

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::RiskLimitStateUnavailable.code()
        );
    }

    #[test]
    fn order_intent_gate_with_limit_state_allows_when_all_pretrade_inputs_are_healthy() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));
        let pretrade_decision = evaluate_pretrade_gate_decision_with_limit_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
        );

        let decision = evaluate_order_intent_gate_with_limit_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
        );

        assert_eq!(pretrade_decision.outcome, PreTradeDecisionOutcome::Allow);
        assert_eq!(
            pretrade_decision.reason_code,
            PreTradeReasonCode::Pass.code()
        );
        assert!(decision.allowed);
        assert_eq!(decision.reason_code, PreTradeReasonCode::Pass.code());
    }

    #[test]
    fn order_intent_gate_with_limit_state_uses_deterministic_gate_precedence() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        runtime_state.set_freshness_pause(true, FreshnessGateReasonCode::StaleBreach.code());
        runtime_state.set_stream_health_state(RuntimeStreamHealthState {
            backlog_seconds: 12.0,
            sustained_backlog_seconds: 31.0,
            heartbeat_gap_seconds: 1.0,
            heartbeat_timeout_seconds: 15.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });

        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));

        let decision = evaluate_order_intent_gate_with_limit_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
        );

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::FreshnessStaleBreach.code()
        );
    }

    #[test]
    fn order_intent_gate_with_limit_state_denies_on_single_stream_health_failure() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        runtime_state.set_stream_health_state(RuntimeStreamHealthState {
            backlog_seconds: 10.1,
            sustained_backlog_seconds: 30.1,
            heartbeat_gap_seconds: 2.0,
            heartbeat_timeout_seconds: 15.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));

        let decision = evaluate_order_intent_gate_with_limit_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
        );

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::StreamHealthDegraded.code()
        );
    }

    #[test]
    fn order_intent_gate_with_limit_state_fails_closed_when_strategy_approval_state_missing() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        runtime_state.clear_strategy_approval_state();
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));

        let decision = evaluate_order_intent_gate_with_limit_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
        );

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::StrategyApprovalUnavailable.code()
        );
    }

    #[test]
    fn order_intent_gate_with_limit_state_fails_closed_when_market_snapshot_is_missing() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));
        let mut intent = sample_intent();
        intent.market_id = "missing_market".to_string();

        let decision =
            evaluate_order_intent_gate_with_limit_state(&runtime_state, &limit_state, &intent, "default");

        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::VenueEligibilityUnavailable.code()
        );
    }

    #[test]
    fn drawdown_boundary_equality_triggers_protective_mode_signal() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        runtime_state.set_drawdown_state(RuntimeDrawdownState {
            current_drawdown_pct: 8.5,
            configured_stop_threshold_pct: 8.5,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));
        let safe_state_signals = InMemorySafeStateSignals::default();

        let decision = evaluate_pretrade_gate_decision_with_limit_state_and_safe_state(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
            &safe_state_signals,
        );

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Deny);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::DrawdownStopTriggered.code()
        );
        assert!(decision.protective_mode_active);

        let latest_signal = safe_state_signals
            .latest_drawdown_signal()
            .expect("drawdown stop should publish protective mode signal");
        assert!(latest_signal.active);
        assert_eq!(
            latest_signal.reason_code,
            PreTradeReasonCode::DrawdownStopTriggered.code()
        );
    }

    #[test]
    fn invalid_payload_decision_preserves_valid_requested_timestamp() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        let limit_state = InMemoryRiskLimitState::default();
        let mut intent = sample_intent();
        intent.market_id = "   ".to_string();

        let decision = evaluate_pretrade_gate_decision_with_limit_state(
            &runtime_state,
            &limit_state,
            &intent,
            "default",
        );

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Deny);
        assert_eq!(decision.reason_code, PreTradeReasonCode::InvalidPayload.code());
        assert_eq!(decision.evaluated_at_utc, intent.requested_at_utc);
    }

    #[test]
    fn invalid_payload_decision_uses_epoch_fallback_for_invalid_timestamp() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        let limit_state = InMemoryRiskLimitState::default();
        let mut intent = sample_intent();
        intent.requested_at_utc = "not-a-timestamp".to_string();

        let decision = evaluate_pretrade_gate_decision_with_limit_state(
            &runtime_state,
            &limit_state,
            &intent,
            "default",
        );

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Deny);
        assert_eq!(decision.reason_code, PreTradeReasonCode::InvalidPayload.code());
        assert_eq!(
            decision.evaluated_at_utc,
            FALLBACK_PRETRADE_EVALUATED_AT_UTC
        );
    }

    #[test]
    fn finalize_pretrade_decision_falls_back_fail_closed_when_contract_rejected() {
        let intent = sample_intent();
        let safe_state_signals = InMemorySafeStateSignals::default();

        let decision = finalize_pretrade_decision(
            &intent,
            "default",
            vec![build_pretrade_gate_result(
                PreTradeGateDimension::Freshness,
                true,
                PreTradeReasonCode::Pass,
                &intent.requested_at_utc,
            )],
            true,
            &safe_state_signals,
            &intent.requested_at_utc,
        );

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Deny);
        assert_eq!(decision.reason_code, PreTradeReasonCode::InvalidPayload.code());
        assert!(!decision.protective_mode_active);
        assert_eq!(decision.gate_results.len(), 1);
        assert_eq!(
            decision.gate_results[0].reason_code,
            PreTradeReasonCode::InvalidPayload.code()
        );
        assert!(safe_state_signals.latest_drawdown_signal().is_none());
    }

    #[derive(Debug, Default, Clone)]
    struct RecordingPreTradeDecisionStore {
        persisted: Arc<Mutex<Vec<PreTradeGateDecision>>>,
    }

    impl RecordingPreTradeDecisionStore {
        fn persisted_len(&self) -> usize {
            self.persisted
                .lock()
                .expect("recording decision store mutex should not be poisoned")
                .len()
        }
    }

    impl PreTradeDecisionPersistencePort for RecordingPreTradeDecisionStore {
        fn persist_pretrade_decision<'a>(
            &'a self,
            decision: &'a PreTradeGateDecision,
        ) -> Pin<Box<dyn Future<Output = Result<(), PreTradeDecisionPersistenceError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.persisted
                    .lock()
                    .expect("recording decision store mutex should not be poisoned")
                    .push(decision.clone());
                Ok(())
            })
        }
    }

    #[derive(Debug, Default)]
    struct FailingPreTradeDecisionStore;

    impl PreTradeDecisionPersistencePort for FailingPreTradeDecisionStore {
        fn persist_pretrade_decision<'a>(
            &'a self,
            _decision: &'a PreTradeGateDecision,
        ) -> Pin<Box<dyn Future<Output = Result<(), PreTradeDecisionPersistenceError>> + Send + 'a>>
        {
            Box::pin(async {
                Err(PreTradeDecisionPersistenceError::new(
                    PreTradeReasonCode::PersistenceUnavailable.code(),
                    "simulated persistence outage",
                ))
            })
        }
    }

    #[tokio::test]
    async fn live_pretrade_path_persists_decisions_after_runtime_adjudication() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));
        let safe_state_signals = InMemorySafeStateSignals::default();
        let decision_store = RecordingPreTradeDecisionStore::default();

        let decision = evaluate_order_intent_gate_with_limit_state_and_safe_state_and_persistence(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
            &safe_state_signals,
            &decision_store,
        )
        .await
        .expect("live pre-trade adjudication should persist decision evidence");

        assert!(decision.allowed);
        assert_eq!(decision_store.persisted_len(), 1);
    }

    #[tokio::test]
    async fn live_pretrade_path_surfaces_persistence_failures() {
        let runtime_state = InMemoryRuntimePolicyState::default();
        seed_healthy_pretrade_policy_state(&runtime_state);
        let limit_state = InMemoryRiskLimitState::default();
        limit_state.upsert_active_profile(sample_active_limit_profile("2026-04-06T00:00:00Z"));
        let safe_state_signals = InMemorySafeStateSignals::default();

        let error = evaluate_order_intent_gate_with_limit_state_and_safe_state_and_persistence(
            &runtime_state,
            &limit_state,
            &sample_intent(),
            "default",
            &safe_state_signals,
            &FailingPreTradeDecisionStore,
        )
        .await
        .expect_err("persistence failures must be surfaced to caller");

        assert_eq!(
            error.code,
            PreTradeReasonCode::PersistenceUnavailable.code()
        );
    }
}
