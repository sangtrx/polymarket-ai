mod gates;
mod limits;
mod safe_state;

use common::time::timestamp_utc;
use domain::risk::{FreshnessGateReasonCode, MarketClusterOverride, MarketPolicyReasonCode};

#[tokio::main]
async fn main() {
    let runtime_policy_state = gates::InMemoryRuntimePolicyState::default();
    runtime_policy_state.set_user_stream_auth_block(false);
    runtime_policy_state.set_freshness_pause(false, FreshnessGateReasonCode::BoundarySafe.code());
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
    let _bootstrap_gate_decision =
        gates::evaluate_order_intent_gate(&runtime_policy_state, &bootstrap_intent);
    println!("risk-engine bootstrap ready at {}", timestamp_utc());
}
