mod allocation;
mod attribution;

use common::time::timestamp_utc;
use domain::attribution::AttributionPeriod;

fn warm_allocation_recommendation_seam(now_utc: &str) {
    let observation = allocation::AllocationDriftObservation {
        policy_key: "portfolio-default".to_string(),
        exposure_drift_pct: 0.0,
        relative_alpha_drift_pct: 0.0,
        observed_at_utc: now_utc.to_string(),
        stale_after_seconds: 60.0,
        correlation_id: "portfolio-engine-startup".to_string(),
        require_execution: false,
        approval_reference: None,
    };
    let _ = allocation::build_rebalance_recommendation_read_model(None, &observation);
}

fn warm_attribution_read_model_seam(now_utc: &str) {
    let _ = attribution::build_attribution_read_model(
        AttributionPeriod::TwentyFourHours,
        now_utc,
        None,
        None,
        false,
        &[attribution::AttributionRuntimeInput {
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period_end_utc: now_utc.to_string(),
            realized_pnl_usd: 120.0,
            unrealized_pnl_usd: 20.0,
            fees_usd: 6.0,
            rebates_usd: 2.0,
            incentives_usd: 1.0,
            as_of_utc: now_utc.to_string(),
            source: "portfolio-engine.attribution.v1".to_string(),
            reason_code: domain::attribution::AttributionReasonCode::Ready,
            correlation_id: "portfolio-engine-startup".to_string(),
            snapshot_id: Some("snapshot::attribution::startup".to_string()),
            run_id: Some("run::reconciliation::startup".to_string()),
        }],
    );
}

#[tokio::main]
async fn main() {
    let now_utc = timestamp_utc();
    warm_allocation_recommendation_seam(&now_utc);
    warm_attribution_read_model_seam(&now_utc);
    println!("portfolio-engine scaffold ready at {}", now_utc);
}
