mod ingestion;
mod orders;
mod reconciliation;

use common::time::timestamp_utc;
use domain::risk::MarketSnapshot;

#[tokio::main]
async fn main() {
    let bootstrap_policy_state = ingestion::IngestionPolicyState::default();
    let bootstrap_snapshot = MarketSnapshot {
        market_id: "bootstrap_market".to_string(),
        cluster_id: "bootstrap_cluster".to_string(),
        liquidity_depth_usd: 0.0,
        spread_bps: 0.0,
        reward_score: 0.0,
        projected_exposure_pct_nav: 0.0,
        observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
    };
    let _bootstrap_decision = bootstrap_policy_state.evaluate_market_snapshot(
        &bootstrap_snapshot,
        "execution-bootstrap-correlation",
        "2026-04-06T00:00:00Z",
    );
    println!("execution-engine bootstrap ready at {}", timestamp_utc());
}
