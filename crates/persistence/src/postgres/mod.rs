pub mod allocation_policies;
pub mod attribution_snapshots;
pub mod approvals;
pub mod audit;
pub mod credential_rotation;
pub mod freshness_gate;
pub mod incident_query_views;
pub mod market_policy;
pub mod market_stream;
pub mod orders;
pub mod pretrade_gate;
pub mod rbac;
pub mod reconciliation;
pub mod risk_limits;
pub mod safety_controls;
pub mod user_stream;

pub fn migration_namespace() -> &'static str {
    "governance_rbac"
}
