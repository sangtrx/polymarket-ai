mod allocation;
mod attribution;

use common::time::timestamp_utc;

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

#[tokio::main]
async fn main() {
    let now_utc = timestamp_utc();
    warm_allocation_recommendation_seam(&now_utc);
    println!("portfolio-engine scaffold ready at {}", now_utc);
}
