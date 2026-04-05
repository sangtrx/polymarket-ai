pub mod approvals;
pub mod audit;
pub mod credential_rotation;
pub mod freshness_gate;
pub mod market_policy;
pub mod market_stream;
pub mod orders;
pub mod rbac;
pub mod user_stream;

pub fn migration_namespace() -> &'static str {
    "governance_rbac"
}
