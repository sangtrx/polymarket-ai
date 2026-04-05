use domain::risk::{
    MarketClusterOverride, MarketEligibilityDecision, MarketPolicyProfile, MarketSnapshot,
    evaluate_market_eligibility,
};
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct IngestionPolicyState {
    pub profile: Option<MarketPolicyProfile>,
    pub cluster_override: Option<MarketClusterOverride>,
}

impl IngestionPolicyState {
    pub fn evaluate_market_snapshot(
        &self,
        snapshot: &MarketSnapshot,
        correlation_id: &str,
        evaluated_at_utc: &str,
    ) -> MarketEligibilityDecision {
        let decision = evaluate_market_eligibility(
            Some(snapshot),
            self.profile.as_ref(),
            self.cluster_override.as_ref(),
            correlation_id,
            evaluated_at_utc,
        );
        emit_ingestion_policy_telemetry(&decision);
        decision
    }
}

fn emit_ingestion_policy_telemetry(decision: &MarketEligibilityDecision) {
    let telemetry = IngestionPolicyTelemetryEvent {
        event_name: "ingestion_market_policy_evaluation_v1",
        action: "evaluate_market_tradability",
        market_id: &decision.market_id,
        cluster_id: &decision.cluster_id,
        outcome: decision.outcome.as_str(),
        reason_code: &decision.reason_code,
        correlation_id: &decision.correlation_id,
        timestamp_utc: &decision.evaluated_at_utc,
        tradable: decision.tradable,
    };
    println!(
        "{}",
        serde_json::to_string(&telemetry).expect("ingestion policy telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct IngestionPolicyTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    market_id: &'a str,
    cluster_id: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    tradable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::risk::MarketPolicyReasonCode;

    fn sample_snapshot() -> MarketSnapshot {
        MarketSnapshot {
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            liquidity_depth_usd: 1_500.0,
            spread_bps: 1.5,
            reward_score: 0.8,
            projected_exposure_pct_nav: 20.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_profile() -> MarketPolicyProfile {
        MarketPolicyProfile {
            profile_id: "policy_cluster_alpha".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            min_liquidity_usd: 500.0,
            max_spread_bps: 2.0,
            min_reward_score: 0.5,
            max_exposure_pct_nav: 25.0,
            is_active: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
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
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn evaluator_marks_market_tradable_when_thresholds_pass() {
        let state = IngestionPolicyState {
            profile: Some(sample_profile()),
            cluster_override: Some(sample_override(true)),
        };
        let decision = state.evaluate_market_snapshot(
            &sample_snapshot(),
            "corr-ingestion-001",
            "2026-04-06T00:00:00Z",
        );

        assert!(decision.tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }

    #[test]
    fn evaluator_is_fail_closed_when_policy_state_missing() {
        let state = IngestionPolicyState {
            profile: None,
            cluster_override: Some(sample_override(true)),
        };
        let decision = state.evaluate_market_snapshot(
            &sample_snapshot(),
            "corr-ingestion-002",
            "2026-04-06T00:00:00Z",
        );

        assert!(!decision.tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn evaluator_blocks_when_cluster_toggle_is_disabled() {
        let state = IngestionPolicyState {
            profile: Some(sample_profile()),
            cluster_override: Some(sample_override(false)),
        };
        let decision = state.evaluate_market_snapshot(
            &sample_snapshot(),
            "corr-ingestion-003",
            "2026-04-06T00:00:00Z",
        );

        assert!(!decision.tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::ClusterDisabled.code()
        );
    }
}
