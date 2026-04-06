mod gates;
mod limits;
mod safe_state;

use common::time::timestamp_utc;
use domain::reconciliation::ReconciliationReasonCode;
use domain::risk::{
    FreshnessGateReasonCode, MarketClusterOverride, MarketPolicyReasonCode, RiskLimitProfileStatus,
    RiskLimitProfileVersion, RiskLimitReasonCode, RiskLimitScope, RiskScopeLimit,
};

#[tokio::main]
async fn main() {
    let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
    let runtime_limit_state = limits::InMemoryRiskLimitState::default();

    runtime_policy_state.set_user_stream_auth_block(false);
    runtime_policy_state.set_freshness_pause(false, FreshnessGateReasonCode::BoundarySafe.code());
    runtime_policy_state
        .set_reconciliation_halt(false, ReconciliationReasonCode::CriticalMismatch.code());
    runtime_policy_state.upsert_cluster_override(MarketClusterOverride {
        cluster_id: "bootstrap-cluster".to_string(),
        is_enabled: true,
        reason_code: MarketPolicyReasonCode::ClusterEnabled.code().to_string(),
        actor_id: "risk-engine-bootstrap".to_string(),
        correlation_id: "risk-bootstrap-correlation".to_string(),
        updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
    });
    let bootstrap_intent = gates::OrderIntent {
        intent_id: "bootstrap-intent".to_string(),
        market_id: "bootstrap-market".to_string(),
        cluster_id: "bootstrap-cluster".to_string(),
        correlation_id: "risk-bootstrap-correlation".to_string(),
        requested_at_utc: "2026-04-06T00:00:00Z".to_string(),
    };
    runtime_limit_state.upsert_active_profile(RiskLimitProfileVersion {
        profile_key: "bootstrap".to_string(),
        version: 1,
        portfolio: RiskScopeLimit {
            scope: RiskLimitScope::Portfolio,
            scope_id: "portfolio::bootstrap".to_string(),
            max_notional_usd: 1000.0,
            max_inventory_units: 800.0,
            max_concentration_pct_nav: 40.0,
        },
        market: RiskScopeLimit {
            scope: RiskLimitScope::Market,
            scope_id: "market::bootstrap".to_string(),
            max_notional_usd: 600.0,
            max_inventory_units: 400.0,
            max_concentration_pct_nav: 30.0,
        },
        strategy: RiskScopeLimit {
            scope: RiskLimitScope::Strategy,
            scope_id: "strategy::bootstrap".to_string(),
            max_notional_usd: 300.0,
            max_inventory_units: 200.0,
            max_concentration_pct_nav: 20.0,
        },
        status: RiskLimitProfileStatus::Active,
        approval_reference: Some("apr_risk_limit_increase_bootstrap".to_string()),
        actor_id: "risk-engine-bootstrap".to_string(),
        reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
        correlation_id: "risk-bootstrap-correlation".to_string(),
        updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
    });
    runtime_limit_state.upsert_pending_profile(RiskLimitProfileVersion {
        profile_key: "bootstrap".to_string(),
        version: 2,
        portfolio: RiskScopeLimit {
            scope: RiskLimitScope::Portfolio,
            scope_id: "portfolio::bootstrap".to_string(),
            max_notional_usd: 1100.0,
            max_inventory_units: 820.0,
            max_concentration_pct_nav: 42.0,
        },
        market: RiskScopeLimit {
            scope: RiskLimitScope::Market,
            scope_id: "market::bootstrap".to_string(),
            max_notional_usd: 620.0,
            max_inventory_units: 420.0,
            max_concentration_pct_nav: 32.0,
        },
        strategy: RiskScopeLimit {
            scope: RiskLimitScope::Strategy,
            scope_id: "strategy::bootstrap".to_string(),
            max_notional_usd: 320.0,
            max_inventory_units: 220.0,
            max_concentration_pct_nav: 22.0,
        },
        status: RiskLimitProfileStatus::Pending,
        approval_reference: None,
        actor_id: "risk-engine-bootstrap".to_string(),
        reason_code: RiskLimitReasonCode::ApprovalRequired.code().to_string(),
        correlation_id: "risk-bootstrap-correlation".to_string(),
        updated_at_utc: "2026-04-06T00:00:05Z".to_string(),
    });
    runtime_limit_state.set_state_unavailable(RiskLimitReasonCode::PolicyStateUnavailable.code());
    runtime_limit_state.clear_state_unavailable();

    let _bootstrap_gate_decision = gates::evaluate_order_intent_gate_with_limit_state(
        &runtime_policy_state,
        &runtime_limit_state,
        &bootstrap_intent,
        "bootstrap",
    );
    let _bootstrap_limit_snapshot = limits::evaluate_risk_limit_state_snapshot(
        &runtime_limit_state,
        "bootstrap",
        "2026-04-06T00:00:00Z",
    );
    println!("risk-engine bootstrap ready at {}", timestamp_utc());
}
