mod gates;
mod limits;
mod safe_state;

use crate::gates::RuntimePolicyStateReader;
use common::time::timestamp_utc;
use domain::reconciliation::ReconciliationReasonCode;
use domain::recovery::{
    RecoveryGateRunEvidence, RecoveryReadinessStatus, RecoveryReasonCode,
    validate_recovery_gate_run_evidence,
};
use domain::risk::{
    EmergencyControlMode, MarketPolicyReasonCode, MarketSnapshot, PreTradeReasonCode,
    RewardRiskScoreInput, RiskLimitReasonCode, VenueEligibilityState,
    compute_reward_per_risk_score,
};
use persistence::postgres::freshness_gate::load_latest_freshness_gate_event;
use persistence::postgres::market_bucket_profiles::load_active_market_bucket_profile;
use persistence::postgres::market_policy::{
    load_active_market_policy_profile, load_market_cluster_override,
};
use persistence::postgres::market_stream::load_latest_market_stream_health;
use persistence::postgres::participation_guardrail_events::{
    ParticipationGuardrailPersistenceError, create_participation_guardrail_event,
};
use persistence::postgres::pretrade_gate::{
    PreTradeGatePersistenceError, insert_pretrade_gate_decision,
};
use persistence::postgres::recovery_gate_runs::load_latest_recovery_gate_run;
use persistence::postgres::reward_risk::load_reward_risk_policy;
use persistence::postgres::risk_limits::{
    load_active_risk_limit_profile_bundle, load_pending_risk_limit_profile_bundles,
};
use persistence::postgres::safety_controls::load_current_effective_safety_mode;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::future::Future;
use std::pin::Pin;

#[tokio::main]
async fn main() {
    let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
    let runtime_limit_state = limits::InMemoryRiskLimitState::default();
    let safe_state_signals = safe_state::InMemorySafeStateSignals::default();
    let bootstrap_config = BootstrapRuntimeConfig::from_env();
    let bootstrap_requested_at_utc = timestamp_utc();

    let pool = hydrate_runtime_state_from_sources(
        &runtime_policy_state,
        &runtime_limit_state,
        &bootstrap_config,
    )
    .await;
    let pretrade_persistence_port = pool
        .clone()
        .map(PostgresPreTradeDecisionPersistencePort::new);

    let bootstrap_intent = gates::OrderIntent {
        intent_id: "bootstrap-intent".to_string(),
        market_id: bootstrap_config.market_id.clone(),
        cluster_id: bootstrap_config.cluster_id.clone(),
        correlation_id: "risk-bootstrap-correlation".to_string(),
        requested_at_utc: bootstrap_requested_at_utc.clone(),
    };
    let _bootstrap_pretrade_decision =
        gates::evaluate_pretrade_gate_decision_with_limit_state_and_safe_state(
            &runtime_policy_state,
            &runtime_limit_state,
            &bootstrap_intent,
            &bootstrap_config.profile_key,
            &safe_state_signals,
        );
    let _bootstrap_gate_decision = evaluate_runtime_order_intent_gate_with_limit_state(
        &runtime_policy_state,
        &runtime_limit_state,
        &bootstrap_intent,
        &bootstrap_config.profile_key,
        &safe_state_signals,
        pretrade_persistence_port.as_ref(),
    )
    .await
    .expect("bootstrap runtime adjudication should evaluate without persistence side effects");
    let _bootstrap_limit_snapshot = limits::evaluate_risk_limit_state_snapshot(
        &runtime_limit_state,
        &bootstrap_config.profile_key,
        &bootstrap_requested_at_utc,
    );
    println!("risk-engine bootstrap ready at {}", timestamp_utc());
}

#[derive(Debug, Clone)]
struct PostgresPreTradeDecisionPersistencePort {
    pool: PgPool,
}

impl PostgresPreTradeDecisionPersistencePort {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl gates::PreTradeDecisionPersistencePort for PostgresPreTradeDecisionPersistencePort {
    fn persist_pretrade_decision<'a>(
        &'a self,
        decision: &'a domain::risk::PreTradeGateDecision,
    ) -> Pin<
        Box<dyn Future<Output = Result<(), gates::PreTradeDecisionPersistenceError>> + Send + 'a>,
    > {
        Box::pin(async move {
            insert_pretrade_gate_decision(&self.pool, decision)
                .await
                .map_err(map_pretrade_persistence_error)?;
            if let Some(evidence) = decision.participation_guardrail.as_ref() {
                create_participation_guardrail_event(&self.pool, evidence)
                    .await
                    .map_err(map_participation_guardrail_persistence_error)?;
            }
            Ok(())
        })
    }
}

fn map_pretrade_persistence_error(
    error: PreTradeGatePersistenceError,
) -> gates::PreTradeDecisionPersistenceError {
    let normalized_code = PreTradeReasonCode::parse(error.code)
        .map(|reason| reason.code())
        .unwrap_or(PreTradeReasonCode::PersistenceUnavailable.code());
    gates::PreTradeDecisionPersistenceError::new(normalized_code, error.to_string())
}

fn map_participation_guardrail_persistence_error(
    error: ParticipationGuardrailPersistenceError,
) -> gates::PreTradeDecisionPersistenceError {
    gates::PreTradeDecisionPersistenceError::new(
        PreTradeReasonCode::PersistenceUnavailable.code(),
        error.to_string(),
    )
}

async fn evaluate_runtime_order_intent_gate_with_limit_state<D>(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    runtime_limit_state: &limits::InMemoryRiskLimitState,
    intent: &gates::OrderIntent,
    profile_key: &str,
    safe_state_port: &safe_state::InMemorySafeStateSignals,
    pretrade_persistence_port: Option<&D>,
) -> Result<gates::OrderIntentGateDecision, gates::PreTradeDecisionPersistenceError>
where
    D: gates::PreTradeDecisionPersistencePort,
{
    if is_bootstrap_intent(&intent.intent_id) {
        return Ok(
            gates::evaluate_order_intent_gate_with_limit_state_and_safe_state(
                runtime_policy_state,
                runtime_limit_state,
                intent,
                profile_key,
                safe_state_port,
            ),
        );
    }

    let persistence_port = pretrade_persistence_port.ok_or_else(|| {
        gates::PreTradeDecisionPersistenceError::new(
            PreTradeReasonCode::PersistenceUnavailable.code(),
            "risk-engine pre-trade persistence adapter unavailable for live adjudication",
        )
    })?;

    gates::evaluate_order_intent_gate_with_limit_state_and_safe_state_and_persistence(
        runtime_policy_state,
        runtime_limit_state,
        intent,
        profile_key,
        safe_state_port,
        persistence_port,
    )
    .await
}

fn is_bootstrap_intent(intent_id: &str) -> bool {
    intent_id.trim().eq_ignore_ascii_case("bootstrap-intent")
}

#[derive(Debug, Clone)]
struct BootstrapRuntimeConfig {
    profile_key: String,
    cluster_id: String,
    market_id: String,
    stream_name: String,
    stream_heartbeat_timeout_seconds: f64,
}

impl BootstrapRuntimeConfig {
    fn from_env() -> Self {
        let stream_heartbeat_timeout_seconds =
            std::env::var("RISK_ENGINE_STREAM_HEARTBEAT_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.trim().parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(15.0);

        Self {
            profile_key: std::env::var("RISK_ENGINE_BOOTSTRAP_PROFILE_KEY")
                .unwrap_or_else(|_| "bootstrap".to_string())
                .trim()
                .to_ascii_lowercase(),
            cluster_id: std::env::var("RISK_ENGINE_BOOTSTRAP_CLUSTER_ID")
                .unwrap_or_else(|_| "bootstrap-cluster".to_string())
                .trim()
                .to_ascii_lowercase(),
            market_id: std::env::var("RISK_ENGINE_BOOTSTRAP_MARKET_ID")
                .unwrap_or_else(|_| "bootstrap-market".to_string())
                .trim()
                .to_ascii_lowercase(),
            stream_name: std::env::var("RISK_ENGINE_BOOTSTRAP_STREAM_NAME")
                .unwrap_or_else(|_| "polymarket_market_stream".to_string())
                .trim()
                .to_string(),
            stream_heartbeat_timeout_seconds,
        }
    }
}

async fn hydrate_runtime_state_from_sources(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    runtime_limit_state: &limits::InMemoryRiskLimitState,
    config: &BootstrapRuntimeConfig,
) -> Option<PgPool> {
    // Default to explicit fail-closed until each source-of-truth seam is hydrated.
    runtime_limit_state.set_state_unavailable(RiskLimitReasonCode::PolicyStateUnavailable.code());
    runtime_policy_state.clear_freshness_state();
    runtime_policy_state.clear_stream_health_state();
    runtime_policy_state.clear_drawdown_state();
    runtime_policy_state.clear_strategy_approval_state();
    runtime_policy_state.set_user_stream_auth_block(true);
    runtime_policy_state
        .set_reconciliation_halt(true, ReconciliationReasonCode::CriticalMismatch.code());
    if let Some(user_stream_auth_block_active) =
        env_bool("RISK_ENGINE_BOOTSTRAP_USER_STREAM_AUTH_BLOCK_ACTIVE")
    {
        runtime_policy_state.set_user_stream_auth_block(user_stream_auth_block_active);
    }
    if let Some(reconciliation_halt_active) =
        env_bool("RISK_ENGINE_BOOTSTRAP_RECONCILIATION_HALT_ACTIVE")
    {
        let reconciliation_reason_code = std::env::var(
            "RISK_ENGINE_BOOTSTRAP_RECONCILIATION_REASON_CODE",
        )
        .unwrap_or_else(|_| {
            ReconciliationReasonCode::CriticalMismatch
                .code()
                .to_string()
        });
        runtime_policy_state
            .set_reconciliation_halt(reconciliation_halt_active, &reconciliation_reason_code);
    }

    if let Some(snapshot) = load_market_snapshot_from_env(config) {
        runtime_policy_state.upsert_market_snapshot(snapshot);
    }
    if let Some(drawdown_state) = load_drawdown_state_from_env() {
        runtime_policy_state.set_drawdown_state(drawdown_state);
    }
    if let Some(strategy_state) = load_strategy_approval_state_from_env() {
        runtime_policy_state.set_strategy_approval_state(strategy_state);
    }

    let pool = connect_postgres_from_env().await?;
    hydrate_latest_freshness_state(runtime_policy_state, &pool).await;
    hydrate_latest_stream_health_state(runtime_policy_state, &pool, config).await;
    hydrate_latest_market_policy_state(runtime_policy_state, &pool, config).await;
    hydrate_latest_market_bucket_state(runtime_policy_state, &pool, config).await;
    hydrate_latest_limit_state(runtime_policy_state, runtime_limit_state, &pool, config).await;
    hydrate_latest_reward_risk_state(runtime_policy_state, &pool, config).await;
    hydrate_latest_safety_mode_state(runtime_policy_state, &pool).await;
    Some(pool)
}

async fn connect_postgres_from_env() -> Option<PgPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) => value,
        Err(_) => return None,
    };
    match PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
    {
        Ok(pool) => Some(pool),
        Err(error) => {
            println!(
                "risk-engine bootstrap failed to connect to Postgres, remaining fail-closed: {error}"
            );
            None
        }
    }
}

async fn hydrate_latest_freshness_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    pool: &PgPool,
) {
    match load_latest_freshness_gate_event(pool).await {
        Ok(Some(event)) => {
            runtime_policy_state.set_freshness_pause(event.pause_active, &event.reason_code);
        }
        Ok(None) => {}
        Err(error) => {
            println!(
                "risk-engine bootstrap could not hydrate freshness state, remaining fail-closed: {error}"
            );
        }
    }
}

async fn hydrate_latest_stream_health_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    pool: &PgPool,
    config: &BootstrapRuntimeConfig,
) {
    match load_latest_market_stream_health(pool, &config.stream_name).await {
        Ok(Some(health)) => {
            runtime_policy_state.set_stream_health_state(gates::RuntimeStreamHealthState {
                backlog_seconds: health.backlog_seconds,
                sustained_backlog_seconds: health.sustained_backlog_seconds,
                heartbeat_gap_seconds: health.heartbeat_gap_seconds,
                heartbeat_timeout_seconds: config.stream_heartbeat_timeout_seconds,
                observed_at_utc: health.observed_at_utc,
            })
        }
        Ok(None) => {}
        Err(error) => {
            println!(
                "risk-engine bootstrap could not hydrate stream health state, remaining fail-closed: {error}"
            );
        }
    }
}

async fn hydrate_latest_limit_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    runtime_limit_state: &limits::InMemoryRiskLimitState,
    pool: &PgPool,
    config: &BootstrapRuntimeConfig,
) {
    let resolved_profile_key = resolved_runtime_profile_key(runtime_policy_state, config);
    match load_active_risk_limit_profile_bundle(pool, &resolved_profile_key).await {
        Ok(Some(bundle)) => {
            runtime_limit_state.clear_state_unavailable();
            let profile_key = bundle.profile.profile_key.clone();
            runtime_limit_state.upsert_active_profile(bundle.profile);
            runtime_limit_state.upsert_inventory_rules(&profile_key, bundle.inventory_rules);
            match load_pending_risk_limit_profile_bundles(pool, Some(&resolved_profile_key)).await {
                Ok(pending_bundles) => {
                    for pending in pending_bundles {
                        runtime_limit_state.upsert_pending_profile(pending.profile);
                    }
                }
                Err(error) => {
                    println!(
                        "risk-engine bootstrap could not hydrate pending risk-limit profiles: {error}"
                    );
                }
            }
        }
        Ok(None) => {}
        Err(error) => {
            println!(
                "risk-engine bootstrap could not hydrate active risk-limit profile, remaining fail-closed: {error}"
            );
        }
    }
}

async fn hydrate_latest_market_policy_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    pool: &PgPool,
    config: &BootstrapRuntimeConfig,
) {
    match load_active_market_policy_profile(pool, &config.cluster_id).await {
        Ok(Some(profile)) => runtime_policy_state.upsert_market_policy_profile(profile),
        Ok(None) => {}
        Err(error) => {
            println!("risk-engine bootstrap could not hydrate market policy profile: {error}");
        }
    }
    match load_market_cluster_override(pool, &config.cluster_id).await {
        Ok(Some(override_state)) => runtime_policy_state.upsert_cluster_override(override_state),
        Ok(None) => {}
        Err(error) => {
            println!("risk-engine bootstrap could not hydrate market cluster override: {error}");
        }
    }
}

fn resolved_runtime_profile_key(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    config: &BootstrapRuntimeConfig,
) -> String {
    runtime_policy_state
        .market_bucket_profile(&config.market_id, &config.cluster_id)
        .map(|profile| profile.risk_policy_key)
        .unwrap_or_else(|| config.profile_key.clone())
}

async fn hydrate_latest_reward_risk_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    pool: &PgPool,
    config: &BootstrapRuntimeConfig,
) {
    let resolved_profile_key = resolved_runtime_profile_key(runtime_policy_state, config);
    match load_reward_risk_policy(pool, &resolved_profile_key).await {
        Ok(Some(policy)) => runtime_policy_state.upsert_reward_risk_policy(policy),
        Ok(None) => {}
        Err(error) => {
            println!("risk-engine bootstrap could not hydrate reward-risk policy: {error}");
        }
    }
}

async fn hydrate_latest_market_bucket_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    pool: &PgPool,
    config: &BootstrapRuntimeConfig,
) {
    match load_active_market_bucket_profile(pool, &config.market_id, &config.cluster_id).await {
        Ok(Some(profile)) => runtime_policy_state.upsert_market_bucket_profile(profile),
        Ok(None) => {}
        Err(error) => {
            println!("risk-engine bootstrap could not hydrate market bucket profile: {error}");
        }
    }
}

async fn hydrate_latest_safety_mode_state(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    pool: &PgPool,
) {
    match load_current_effective_safety_mode(pool).await {
        Ok(Some(mode)) => {
            let recovery_release_approved = match mode.resulting_mode {
                EmergencyControlMode::Paused => false,
                EmergencyControlMode::Normal | EmergencyControlMode::ReduceOnly => {
                    match load_latest_recovery_gate_run(pool).await {
                        Ok(Some(run)) => {
                            if is_approved_recovery_resume_run(&run) {
                                true
                            } else {
                                println!(
                                    "risk-engine bootstrap found recovery run `{}` but containment release is not approved",
                                    run.run_id
                                );
                                false
                            }
                        }
                        Ok(None) => {
                            println!(
                                "risk-engine bootstrap found no recovery gate run, preserving fail-closed containment"
                            );
                            false
                        }
                        Err(error) => {
                            println!(
                                "risk-engine bootstrap could not hydrate recovery gate run, preserving fail-closed containment: {error}"
                            );
                            false
                        }
                    }
                }
            };
            apply_safety_mode_containment(
                runtime_policy_state,
                mode.resulting_mode,
                recovery_release_approved,
            );
        }
        Ok(None) => {
            runtime_policy_state.set_user_stream_auth_block(true);
            println!(
                "risk-engine bootstrap found no effective safety mode, preserving fail-closed containment"
            );
        }
        Err(error) => {
            runtime_policy_state.set_user_stream_auth_block(true);
            println!(
                "risk-engine bootstrap could not hydrate safety mode, preserving fail-closed containment: {error}"
            );
        }
    }
}

fn apply_safety_mode_containment(
    runtime_policy_state: &gates::InMemoryRuntimePolicyState,
    mode: EmergencyControlMode,
    recovery_release_approved: bool,
) {
    if mode == EmergencyControlMode::Paused {
        runtime_policy_state.set_user_stream_auth_block(true);
        return;
    }
    runtime_policy_state.set_user_stream_auth_block(!recovery_release_approved);
}

fn is_approved_recovery_resume_run(run: &RecoveryGateRunEvidence) -> bool {
    if validate_recovery_gate_run_evidence(run).is_err() {
        return false;
    }
    if run.readiness_status != RecoveryReadinessStatus::Approved {
        return false;
    }
    if run.reason_code != RecoveryReasonCode::ResumeApproved.code() {
        return false;
    }
    run.resumed_at_utc
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

fn load_market_snapshot_from_env(config: &BootstrapRuntimeConfig) -> Option<MarketSnapshot> {
    let liquidity_depth_usd = env_f64("RISK_ENGINE_BOOTSTRAP_MARKET_LIQUIDITY_USD")?;
    let inactivity_gap_seconds = env_f64("RISK_ENGINE_BOOTSTRAP_MARKET_INACTIVITY_GAP_SECONDS");
    let spread_bps = env_f64("RISK_ENGINE_BOOTSTRAP_MARKET_SPREAD_BPS")?;
    let projected_exposure_pct_nav = env_f64("RISK_ENGINE_BOOTSTRAP_MARKET_EXPOSURE_PCT_NAV")?;
    let observed_at_utc = std::env::var("RISK_ENGINE_BOOTSTRAP_MARKET_OBSERVED_AT_UTC")
        .unwrap_or_else(|_| "2026-04-06T00:00:00Z".to_string());
    let expected_reward_bps = env_f64_any("RISK_ENGINE_BOOTSTRAP_MARKET_EXPECTED_REWARD_BPS");
    let maker_rebate_bps = env_f64_any("RISK_ENGINE_BOOTSTRAP_MARKET_MAKER_REBATE_BPS");
    let expected_cost_bps = env_f64_any("RISK_ENGINE_BOOTSTRAP_MARKET_EXPECTED_COST_BPS");
    let expected_volatility_bps =
        env_f64_any("RISK_ENGINE_BOOTSTRAP_MARKET_EXPECTED_VOLATILITY_BPS");

    let (
        reward_score,
        expected_reward_bps,
        maker_rebate_bps,
        expected_cost_bps,
        expected_volatility_bps,
    ) = match (
        expected_reward_bps,
        maker_rebate_bps,
        expected_cost_bps,
        expected_volatility_bps,
    ) {
        (
            Some(expected_reward_bps),
            Some(maker_rebate_bps),
            Some(expected_cost_bps),
            Some(expected_volatility_bps),
        ) => {
            let reward_score = compute_reward_per_risk_score(&RewardRiskScoreInput {
                expected_reward_bps,
                maker_rebate_bps,
                expected_cost_bps,
                expected_volatility_bps,
            })
            .ok()?;
            (
                reward_score,
                Some(expected_reward_bps),
                Some(maker_rebate_bps),
                Some(expected_cost_bps),
                Some(expected_volatility_bps),
            )
        }
        _ => {
            let reward_score = env_f64("RISK_ENGINE_BOOTSTRAP_MARKET_REWARD_SCORE")?;
            (
                reward_score,
                Some(reward_score),
                Some(0.0),
                Some(0.0),
                Some(1.0),
            )
        }
    };

    Some(MarketSnapshot {
        market_id: config.market_id.clone(),
        cluster_id: config.cluster_id.clone(),
        liquidity_depth_usd,
        inactivity_gap_seconds,
        spread_bps,
        reward_score,
        expected_reward_bps,
        maker_rebate_bps,
        expected_cost_bps,
        expected_volatility_bps,
        venue_eligibility_state: load_market_eligibility_state_from_env(),
        projected_exposure_pct_nav,
        observed_at_utc,
    })
}

fn load_market_eligibility_state_from_env() -> Option<VenueEligibilityState> {
    let raw = std::env::var("RISK_ENGINE_BOOTSTRAP_MARKET_ELIGIBILITY_STATE").ok()?;
    match VenueEligibilityState::parse(raw.trim()) {
        Ok(state) => Some(state),
        Err(error) => {
            println!(
                "risk-engine bootstrap ignored unsupported market eligibility state `{}`: {}",
                raw.trim(),
                error.message
            );
            None
        }
    }
}

fn load_drawdown_state_from_env() -> Option<gates::RuntimeDrawdownState> {
    let current_drawdown_pct = env_f64("RISK_ENGINE_BOOTSTRAP_DRAWDOWN_CURRENT_PCT")?;
    let configured_stop_threshold_pct = env_f64("RISK_ENGINE_BOOTSTRAP_DRAWDOWN_STOP_PCT")?;
    let observed_at_utc = std::env::var("RISK_ENGINE_BOOTSTRAP_DRAWDOWN_OBSERVED_AT_UTC")
        .unwrap_or_else(|_| "2026-04-06T00:00:00Z".to_string());

    Some(gates::RuntimeDrawdownState {
        current_drawdown_pct,
        configured_stop_threshold_pct,
        observed_at_utc,
    })
}

fn load_strategy_approval_state_from_env() -> Option<gates::RuntimeStrategyApprovalState> {
    let is_active = env_bool("RISK_ENGINE_BOOTSTRAP_STRATEGY_APPROVAL_ACTIVE")?;
    let reason_code = std::env::var("RISK_ENGINE_BOOTSTRAP_STRATEGY_APPROVAL_REASON_CODE")
        .unwrap_or_else(|_| {
            if is_active {
                MarketPolicyReasonCode::MarketEligible.code().to_string()
            } else {
                "strategy_approval_required".to_string()
            }
        });
    let observed_at_utc = std::env::var("RISK_ENGINE_BOOTSTRAP_STRATEGY_APPROVAL_OBSERVED_AT_UTC")
        .unwrap_or_else(|_| "2026-04-06T00:00:00Z".to_string());

    Some(gates::RuntimeStrategyApprovalState {
        is_active,
        reason_code,
        observed_at_utc,
    })
}

fn env_f64(name: &str) -> Option<f64> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
}

fn env_f64_any(name: &str) -> Option<f64> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn env_bool(name: &str) -> Option<bool> {
    let value = std::env::var(name).ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default, Clone)]
    struct RecordingPreTradeDecisionStore {
        persisted_intent_ids: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingPreTradeDecisionStore {
        fn persisted_intent_ids(&self) -> Vec<String> {
            self.persisted_intent_ids
                .lock()
                .expect("recording pre-trade decision store mutex should not be poisoned")
                .clone()
        }
    }

    impl gates::PreTradeDecisionPersistencePort for RecordingPreTradeDecisionStore {
        fn persist_pretrade_decision<'a>(
            &'a self,
            decision: &'a domain::risk::PreTradeGateDecision,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<(), gates::PreTradeDecisionPersistenceError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                self.persisted_intent_ids
                    .lock()
                    .expect("recording pre-trade decision store mutex should not be poisoned")
                    .push(decision.intent_id.clone());
                Ok(())
            })
        }
    }

    fn sample_intent(intent_id: &str) -> gates::OrderIntent {
        gates::OrderIntent {
            intent_id: intent_id.to_string(),
            market_id: "market-test-1".to_string(),
            cluster_id: "cluster-test-1".to_string(),
            correlation_id: "corr-test-1".to_string(),
            requested_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn runtime_adjudication_skips_persistence_for_bootstrap_intents() {
        let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
        let runtime_limit_state = limits::InMemoryRiskLimitState::default();
        let safe_state_signals = safe_state::InMemorySafeStateSignals::default();
        let decision_store = RecordingPreTradeDecisionStore::default();

        let decision = evaluate_runtime_order_intent_gate_with_limit_state(
            &runtime_policy_state,
            &runtime_limit_state,
            &sample_intent("bootstrap-intent"),
            "default",
            &safe_state_signals,
            Some(&decision_store),
        )
        .await
        .expect("bootstrap intents should evaluate without persistence");

        assert_eq!(decision.intent_id, "bootstrap-intent");
        assert!(decision_store.persisted_intent_ids().is_empty());
    }

    #[tokio::test]
    async fn runtime_adjudication_requires_persistence_for_non_bootstrap_intents() {
        let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
        let runtime_limit_state = limits::InMemoryRiskLimitState::default();
        let safe_state_signals = safe_state::InMemorySafeStateSignals::default();

        let error =
            evaluate_runtime_order_intent_gate_with_limit_state::<RecordingPreTradeDecisionStore>(
                &runtime_policy_state,
                &runtime_limit_state,
                &sample_intent("live-intent-001"),
                "default",
                &safe_state_signals,
                None,
            )
            .await
            .expect_err("live intents must fail closed when persistence port is unavailable");

        assert_eq!(
            error.code,
            PreTradeReasonCode::PersistenceUnavailable.code()
        );
    }

    #[tokio::test]
    async fn runtime_adjudication_persists_non_bootstrap_intent_decisions() {
        let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
        let runtime_limit_state = limits::InMemoryRiskLimitState::default();
        let safe_state_signals = safe_state::InMemorySafeStateSignals::default();
        let decision_store = RecordingPreTradeDecisionStore::default();

        let decision = evaluate_runtime_order_intent_gate_with_limit_state(
            &runtime_policy_state,
            &runtime_limit_state,
            &sample_intent("live-intent-002"),
            "default",
            &safe_state_signals,
            Some(&decision_store),
        )
        .await
        .expect("live intents should persist pre-trade decisions through runtime path");

        assert_eq!(decision.intent_id, "live-intent-002");
        assert_eq!(
            decision_store.persisted_intent_ids(),
            vec!["live-intent-002".to_string()]
        );
    }

    #[test]
    fn paused_mode_forces_fail_closed_user_stream_auth_block() {
        let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
        runtime_policy_state.set_user_stream_auth_block(false);
        apply_safety_mode_containment(&runtime_policy_state, EmergencyControlMode::Paused, false);
        assert!(
            gates::RuntimePolicyStateReader::user_stream_auth_block_active(&runtime_policy_state)
        );
    }

    #[test]
    fn normal_and_reduce_only_require_approved_recovery_release() {
        let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();

        runtime_policy_state.set_user_stream_auth_block(true);
        apply_safety_mode_containment(&runtime_policy_state, EmergencyControlMode::Normal, false);
        assert!(
            gates::RuntimePolicyStateReader::user_stream_auth_block_active(&runtime_policy_state)
        );
        apply_safety_mode_containment(
            &runtime_policy_state,
            EmergencyControlMode::ReduceOnly,
            false,
        );
        assert!(
            gates::RuntimePolicyStateReader::user_stream_auth_block_active(&runtime_policy_state)
        );

        runtime_policy_state.set_user_stream_auth_block(true);
        apply_safety_mode_containment(&runtime_policy_state, EmergencyControlMode::Normal, true);
        assert!(
            !gates::RuntimePolicyStateReader::user_stream_auth_block_active(&runtime_policy_state)
        );
        runtime_policy_state.set_user_stream_auth_block(true);
        apply_safety_mode_containment(
            &runtime_policy_state,
            EmergencyControlMode::ReduceOnly,
            true,
        );
        assert!(
            !gates::RuntimePolicyStateReader::user_stream_auth_block_active(&runtime_policy_state)
        );
    }

    #[test]
    fn approved_recovery_run_requires_valid_resume_verification() {
        let approved = sample_recovery_run(RecoveryReadinessStatus::Approved, true);
        assert!(is_approved_recovery_resume_run(&approved));

        let missing_verification = sample_recovery_run(RecoveryReadinessStatus::Approved, false);
        assert!(!is_approved_recovery_resume_run(&missing_verification));

        let blocked = sample_recovery_run(RecoveryReadinessStatus::Blocked, false);
        assert!(!is_approved_recovery_resume_run(&blocked));
    }

    fn sample_recovery_run(
        readiness_status: RecoveryReadinessStatus,
        include_resume_verification: bool,
    ) -> RecoveryGateRunEvidence {
        let approved = readiness_status == RecoveryReadinessStatus::Approved;
        RecoveryGateRunEvidence {
            run_id: "run-1".to_string(),
            correlation_id: "corr-1".to_string(),
            readiness_status,
            reason_code: if approved {
                RecoveryReasonCode::ResumeApproved.code().to_string()
            } else {
                RecoveryReasonCode::ResumeBlocked.code().to_string()
            },
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            profile_key: "default".to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            evaluated_at_utc: "2026-04-06T12:00:01Z".to_string(),
            resumed_at_utc: if include_resume_verification {
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
            signoff: Some(domain::recovery::RecoveryOperatorSignoff {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                signoff_intent: "approve controlled recovery".to_string(),
                signed_at_utc: "2026-04-06T12:00:00Z".to_string(),
                audit_reference: Some("arb-1".to_string()),
            }),
            gate_outcomes: vec![
                domain::recovery::RecoveryGateOutcome {
                    gate: domain::recovery::RecoveryGateName::Freshness,
                    passed: approved,
                    reason_code: if approved {
                        RecoveryReasonCode::FreshnessPass.code().to_string()
                    } else {
                        RecoveryReasonCode::FreshnessStale.code().to_string()
                    },
                    trigger: "freshness <= 30s".to_string(),
                    context: "freshness evaluation".to_string(),
                    action: "apply freshness gate".to_string(),
                    verification: "freshness evidence".to_string(),
                },
                domain::recovery::RecoveryGateOutcome {
                    gate: domain::recovery::RecoveryGateName::Reconciliation,
                    passed: approved,
                    reason_code: if approved {
                        RecoveryReasonCode::ReconciliationPass.code().to_string()
                    } else {
                        RecoveryReasonCode::ReconciliationMismatch
                            .code()
                            .to_string()
                    },
                    trigger: "reconciliation < 0.1%".to_string(),
                    context: "reconciliation evaluation".to_string(),
                    action: "apply reconciliation gate".to_string(),
                    verification: "reconciliation evidence".to_string(),
                },
                domain::recovery::RecoveryGateOutcome {
                    gate: domain::recovery::RecoveryGateName::RiskChecksum,
                    passed: approved,
                    reason_code: if approved {
                        RecoveryReasonCode::ChecksumMatch.code().to_string()
                    } else {
                        RecoveryReasonCode::ChecksumMismatch.code().to_string()
                    },
                    trigger: "checksum match".to_string(),
                    context: "checksum evaluation".to_string(),
                    action: "apply checksum gate".to_string(),
                    verification: "checksum evidence".to_string(),
                },
                domain::recovery::RecoveryGateOutcome {
                    gate: domain::recovery::RecoveryGateName::OperatorSignoff,
                    passed: true,
                    reason_code: RecoveryReasonCode::SignoffRecorded.code().to_string(),
                    trigger: "signoff recorded".to_string(),
                    context: "signoff evaluation".to_string(),
                    action: "apply signoff gate".to_string(),
                    verification: "signoff evidence".to_string(),
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
            audit_reference: Some("arb-1".to_string()),
        }
    }
}
