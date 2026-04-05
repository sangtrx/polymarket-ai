use domain::risk::{MarketClusterOverride, MarketPolicyReasonCode};
use serde::Serialize;
use std::collections::BTreeMap;
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

pub trait RuntimePolicyStateReader: Send + Sync {
    fn cluster_override(&self, cluster_id: &str) -> Option<MarketClusterOverride>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRuntimePolicyState {
    overrides: Arc<RwLock<BTreeMap<String, MarketClusterOverride>>>,
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
}

impl RuntimePolicyStateReader for InMemoryRuntimePolicyState {
    fn cluster_override(&self, cluster_id: &str) -> Option<MarketClusterOverride> {
        self.overrides
            .read()
            .expect("cluster override runtime map should not be poisoned")
            .get(&cluster_id.trim().to_ascii_lowercase())
            .cloned()
    }
}

pub fn evaluate_order_intent_gate<S: RuntimePolicyStateReader>(
    runtime_policy_state: &S,
    intent: &OrderIntent,
) -> OrderIntentGateDecision {
    let reason = if intent.cluster_id.trim().is_empty() {
        MarketPolicyReasonCode::InvalidClusterId
    } else {
        match runtime_policy_state.cluster_override(&intent.cluster_id) {
            Some(cluster_state) if cluster_state.is_enabled => {
                MarketPolicyReasonCode::MarketEligible
            }
            Some(_) => MarketPolicyReasonCode::ClusterDisabled,
            None => MarketPolicyReasonCode::PolicyStateUnavailable,
        }
    };

    let decision = OrderIntentGateDecision {
        intent_id: intent.intent_id.clone(),
        market_id: intent.market_id.clone(),
        cluster_id: intent.cluster_id.clone(),
        allowed: reason == MarketPolicyReasonCode::MarketEligible,
        reason_code: reason.code().to_string(),
        correlation_id: intent.correlation_id.clone(),
        decided_at_utc: intent.requested_at_utc.clone(),
    };
    emit_order_intent_gate_telemetry(&decision);
    decision
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
}
