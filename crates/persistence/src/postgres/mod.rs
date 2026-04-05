pub mod approvals;
pub mod audit;
pub mod credential_rotation;
pub mod market_policy;
pub mod rbac;

pub fn migration_namespace() -> &'static str {
    "governance_rbac"
}
