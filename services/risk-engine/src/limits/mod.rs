use domain::risk::{
    InventoryLimitRule, RiskLimitProfileStatus, RiskLimitProfileVersion, RiskLimitReasonCode,
    RiskLimitScope, validate_risk_limit_profile_version,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const LIMIT_STATE_STALE_THRESHOLD_SECONDS: i64 = 60;

pub trait RuntimeRiskLimitStateReader: Send + Sync {
    fn active_profile(&self, profile_key: &str) -> Option<RiskLimitProfileVersion>;
    fn pending_profiles(&self, profile_key: &str) -> Vec<RiskLimitProfileVersion>;
    fn normal_max_order_size_units(
        &self,
        profile_key: &str,
        market_id: &str,
        cluster_id: &str,
    ) -> Option<f64>;
    fn state_unavailable_reason_code(&self) -> Option<String>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRiskLimitState {
    active_profiles: Arc<RwLock<BTreeMap<String, RiskLimitProfileVersion>>>,
    pending_profiles: Arc<RwLock<BTreeMap<String, Vec<RiskLimitProfileVersion>>>>,
    inventory_rules: Arc<RwLock<BTreeMap<String, Vec<InventoryLimitRule>>>>,
    unavailable_reason_code: Arc<RwLock<Option<String>>>,
}

impl InMemoryRiskLimitState {
    pub fn upsert_active_profile(&self, profile: RiskLimitProfileVersion) {
        self.active_profiles
            .write()
            .expect("active risk limit profiles lock should not be poisoned")
            .insert(profile.profile_key.trim().to_ascii_lowercase(), profile);
    }

    pub fn upsert_pending_profile(&self, profile: RiskLimitProfileVersion) {
        self.pending_profiles
            .write()
            .expect("pending risk limit profiles lock should not be poisoned")
            .entry(profile.profile_key.trim().to_ascii_lowercase())
            .or_default()
            .push(profile);
    }

    pub fn upsert_inventory_rules(&self, profile_key: &str, rules: Vec<InventoryLimitRule>) {
        self.inventory_rules
            .write()
            .expect("inventory rules lock should not be poisoned")
            .insert(profile_key.trim().to_ascii_lowercase(), rules);
    }

    pub fn set_state_unavailable(&self, reason_code: &str) {
        let reason = match RiskLimitReasonCode::parse(reason_code) {
            Ok(parsed) => parsed.code().to_string(),
            Err(_) => RiskLimitReasonCode::PolicyStateUnavailable
                .code()
                .to_string(),
        };
        *self
            .unavailable_reason_code
            .write()
            .expect("risk limit unavailable state lock should not be poisoned") = Some(reason);
    }

    pub fn clear_state_unavailable(&self) {
        *self
            .unavailable_reason_code
            .write()
            .expect("risk limit unavailable state lock should not be poisoned") = None;
    }
}

impl RuntimeRiskLimitStateReader for InMemoryRiskLimitState {
    fn active_profile(&self, profile_key: &str) -> Option<RiskLimitProfileVersion> {
        self.active_profiles
            .read()
            .expect("active risk limit profiles lock should not be poisoned")
            .get(&profile_key.trim().to_ascii_lowercase())
            .cloned()
    }

    fn pending_profiles(&self, profile_key: &str) -> Vec<RiskLimitProfileVersion> {
        self.pending_profiles
            .read()
            .expect("pending risk limit profiles lock should not be poisoned")
            .get(&profile_key.trim().to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    fn normal_max_order_size_units(
        &self,
        profile_key: &str,
        market_id: &str,
        cluster_id: &str,
    ) -> Option<f64> {
        let normalized_profile_key = profile_key.trim().to_ascii_lowercase();
        let normalized_market_id = market_id.trim().to_ascii_lowercase();
        let normalized_cluster_id = cluster_id.trim().to_ascii_lowercase();
        let market_scope_candidates = [
            normalized_market_id.clone(),
            format!("market::{normalized_market_id}"),
        ];
        let strategy_scope_candidates = [
            normalized_cluster_id.clone(),
            format!("strategy::{normalized_cluster_id}"),
        ];

        let rules = self
            .inventory_rules
            .read()
            .expect("inventory rules lock should not be poisoned")
            .get(&normalized_profile_key)
            .cloned()
            .unwrap_or_default();

        let select_rule = |scope: RiskLimitScope, candidates: &[String]| {
            rules
                .iter()
                .find(|rule| {
                    rule.scope == scope
                        && candidates
                            .iter()
                            .any(|candidate| rule.scope_id.trim().eq_ignore_ascii_case(candidate))
                })
                .map(|rule| rule.max_order_size_units)
                .filter(|value| value.is_finite() && *value > 0.0)
        };

        select_rule(RiskLimitScope::Market, &market_scope_candidates)
            .or_else(|| select_rule(RiskLimitScope::Strategy, &strategy_scope_candidates))
    }

    fn state_unavailable_reason_code(&self) -> Option<String> {
        self.unavailable_reason_code
            .read()
            .expect("risk limit unavailable state lock should not be poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskLimitStateSnapshot {
    pub profile_key: String,
    pub available: bool,
    pub reason_code: String,
    pub pending_profile_count: usize,
    pub evaluated_at_utc: String,
}

pub fn evaluate_risk_limit_state_snapshot<S: RuntimeRiskLimitStateReader>(
    state: &S,
    profile_key: &str,
    now_utc: &str,
) -> RiskLimitStateSnapshot {
    let normalized_profile_key = profile_key.trim().to_ascii_lowercase();
    let pending_profile_count = state.pending_profiles(&normalized_profile_key).len();

    let decision = if normalized_profile_key.is_empty() || parse_utc(now_utc).is_none() {
        RiskLimitStateSnapshot {
            profile_key: normalized_profile_key,
            available: false,
            reason_code: RiskLimitReasonCode::InvalidPayload.code().to_string(),
            pending_profile_count,
            evaluated_at_utc: now_utc.to_string(),
        }
    } else if let Some(reason_code) = state.state_unavailable_reason_code() {
        let normalized_reason = RiskLimitReasonCode::parse(&reason_code)
            .map(|code| code.code().to_string())
            .unwrap_or_else(|_| {
                RiskLimitReasonCode::PolicyStateUnavailable
                    .code()
                    .to_string()
            });
        RiskLimitStateSnapshot {
            profile_key: normalized_profile_key.clone(),
            available: false,
            reason_code: normalized_reason,
            pending_profile_count,
            evaluated_at_utc: now_utc.to_string(),
        }
    } else {
        match state.active_profile(&normalized_profile_key) {
            None => RiskLimitStateSnapshot {
                profile_key: normalized_profile_key.clone(),
                available: false,
                reason_code: RiskLimitReasonCode::PolicyStateUnavailable
                    .code()
                    .to_string(),
                pending_profile_count,
                evaluated_at_utc: now_utc.to_string(),
            },
            Some(profile)
                if profile.status != RiskLimitProfileStatus::Active
                    || validate_risk_limit_profile_version(&profile).is_err()
                    || profile_stale(&profile, now_utc) =>
            {
                RiskLimitStateSnapshot {
                    profile_key: normalized_profile_key.clone(),
                    available: false,
                    reason_code: RiskLimitReasonCode::PolicyStateUnavailable
                        .code()
                        .to_string(),
                    pending_profile_count,
                    evaluated_at_utc: now_utc.to_string(),
                }
            }
            Some(profile) => RiskLimitStateSnapshot {
                profile_key: normalized_profile_key.clone(),
                available: true,
                reason_code: profile.reason_code,
                pending_profile_count,
                evaluated_at_utc: now_utc.to_string(),
            },
        }
    };

    emit_limit_state_telemetry(&decision);
    decision
}

fn profile_stale(profile: &RiskLimitProfileVersion, now_utc: &str) -> bool {
    let Some(now) = parse_utc(now_utc) else {
        return true;
    };
    let Some(updated_at) = parse_utc(&profile.updated_at_utc) else {
        return true;
    };
    (now - updated_at).whole_seconds() > LIMIT_STATE_STALE_THRESHOLD_SECONDS
}

fn parse_utc(value: &str) -> Option<OffsetDateTime> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).ok()?;
    if parsed.offset() != time::UtcOffset::UTC {
        return None;
    }
    Some(parsed)
}

fn emit_limit_state_telemetry(snapshot: &RiskLimitStateSnapshot) {
    let telemetry = RiskLimitStateTelemetryEvent {
        event_name: "risk_limit_state_snapshot_v1",
        action: "risk_limit_state_snapshot",
        profile_key: &snapshot.profile_key,
        outcome: if snapshot.available { "allow" } else { "deny" },
        reason_code: &snapshot.reason_code,
        pending_profile_count: snapshot.pending_profile_count,
        timestamp_utc: &snapshot.evaluated_at_utc,
    };
    println!(
        "{}",
        serde_json::to_string(&telemetry).expect("risk limit state telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct RiskLimitStateTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    profile_key: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    pending_profile_count: usize,
    timestamp_utc: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::risk::{InventoryLimitRule, RiskLimitScope, RiskScopeLimit};

    fn sample_scope(
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

    fn sample_active_profile(updated_at_utc: &str) -> RiskLimitProfileVersion {
        RiskLimitProfileVersion {
            profile_key: "default".to_string(),
            version: 2,
            portfolio: sample_scope(
                RiskLimitScope::Portfolio,
                "portfolio::default",
                1000.0,
                800.0,
                40.0,
            ),
            market: sample_scope(RiskLimitScope::Market, "market::sports", 600.0, 400.0, 30.0),
            strategy: sample_scope(
                RiskLimitScope::Strategy,
                "strategy::maker",
                300.0,
                200.0,
                20.0,
            ),
            status: RiskLimitProfileStatus::Active,
            approval_reference: Some("apr_risk_limit_increase_req_2".to_string()),
            actor_id: "ops-1".to_string(),
            reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: updated_at_utc.to_string(),
        }
    }

    fn sample_pending_profile(updated_at_utc: &str) -> RiskLimitProfileVersion {
        RiskLimitProfileVersion {
            status: RiskLimitProfileStatus::Pending,
            reason_code: RiskLimitReasonCode::ApprovalRequired.code().to_string(),
            approval_reference: None,
            ..sample_active_profile(updated_at_utc)
        }
    }

    fn sample_inventory_rule(
        scope: RiskLimitScope,
        scope_id: &str,
        max_order_size_units: f64,
    ) -> InventoryLimitRule {
        InventoryLimitRule {
            rule_id: format!("rule::{scope_id}"),
            profile_key: "default".to_string(),
            profile_version: 2,
            scope,
            scope_id: scope_id.to_string(),
            max_position_units: 200.0,
            max_order_size_units,
            max_concentration_pct_nav: 30.0,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn limit_state_snapshot_allows_when_active_profile_is_fresh() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_active_profile(sample_active_profile("2026-04-06T00:00:00Z"));

        let snapshot =
            evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:00:30Z");

        assert!(snapshot.available);
        assert_eq!(
            snapshot.reason_code,
            RiskLimitReasonCode::ProfileApplied.code()
        );
    }

    #[test]
    fn limit_state_snapshot_is_fail_closed_when_profile_missing() {
        let state = InMemoryRiskLimitState::default();
        let snapshot =
            evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:00:30Z");

        assert!(!snapshot.available);
        assert_eq!(
            snapshot.reason_code,
            RiskLimitReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn limit_state_snapshot_is_fail_closed_when_state_marked_unavailable() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_active_profile(sample_active_profile("2026-04-06T00:00:00Z"));
        state.set_state_unavailable(RiskLimitReasonCode::PersistenceUnavailable.code());

        let snapshot =
            evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:00:30Z");
        assert!(!snapshot.available);
        assert_eq!(
            snapshot.reason_code,
            RiskLimitReasonCode::PersistenceUnavailable.code()
        );
    }

    #[test]
    fn limit_state_snapshot_recovers_after_unavailable_flag_clears() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_active_profile(sample_active_profile("2026-04-06T00:00:00Z"));
        state.set_state_unavailable(RiskLimitReasonCode::PersistenceUnavailable.code());
        let blocked = evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:00:30Z");
        assert!(!blocked.available);

        state.clear_state_unavailable();
        let recovered =
            evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:00:30Z");
        assert!(recovered.available);
        assert_eq!(
            recovered.reason_code,
            RiskLimitReasonCode::ProfileApplied.code()
        );
    }

    #[test]
    fn limit_state_snapshot_is_fail_closed_when_active_profile_stale() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_active_profile(sample_active_profile("2026-04-06T00:00:00Z"));

        let snapshot =
            evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:02:05Z");
        assert!(!snapshot.available);
        assert_eq!(
            snapshot.reason_code,
            RiskLimitReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn limit_state_snapshot_reports_pending_profile_count_for_downstream_gates() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_active_profile(sample_active_profile("2026-04-06T00:00:00Z"));
        state.upsert_pending_profile(sample_pending_profile("2026-04-06T00:00:10Z"));
        state.upsert_pending_profile(sample_pending_profile("2026-04-06T00:00:20Z"));

        let snapshot =
            evaluate_risk_limit_state_snapshot(&state, "default", "2026-04-06T00:00:30Z");
        assert!(snapshot.available);
        assert_eq!(snapshot.pending_profile_count, 2);
    }

    #[test]
    fn normal_max_order_size_prefers_market_scoped_rule() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_inventory_rules(
            "default",
            vec![
                sample_inventory_rule(RiskLimitScope::Strategy, "strategy::cluster_alpha", 55.0),
                sample_inventory_rule(RiskLimitScope::Market, "market::market_yes_no_1", 40.0),
            ],
        );

        let max_order_size =
            state.normal_max_order_size_units("default", "market_yes_no_1", "cluster_alpha");
        assert_eq!(max_order_size, Some(40.0));
    }

    #[test]
    fn normal_max_order_size_falls_back_to_strategy_rule_when_market_rule_missing() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_inventory_rules(
            "default",
            vec![sample_inventory_rule(
                RiskLimitScope::Strategy,
                "strategy::cluster_alpha",
                32.0,
            )],
        );

        let max_order_size =
            state.normal_max_order_size_units("default", "market_yes_no_1", "cluster_alpha");
        assert_eq!(max_order_size, Some(32.0));
    }

    #[test]
    fn normal_max_order_size_is_unavailable_without_matching_market_or_strategy_rule() {
        let state = InMemoryRiskLimitState::default();
        state.upsert_inventory_rules(
            "default",
            vec![
                sample_inventory_rule(RiskLimitScope::Market, "market::unrelated_market", 40.0),
                sample_inventory_rule(
                    RiskLimitScope::Strategy,
                    "strategy::unrelated_cluster",
                    32.0,
                ),
            ],
        );

        let max_order_size =
            state.normal_max_order_size_units("default", "market_yes_no_1", "cluster_alpha");
        assert_eq!(max_order_size, None);
    }
}
