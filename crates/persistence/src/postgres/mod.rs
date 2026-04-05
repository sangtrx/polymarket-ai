pub mod approvals;
pub mod audit;
pub mod rbac;

pub fn migration_namespace() -> &'static str {
    "governance_rbac"
}
