pub mod allocation_policies;
pub mod alpha_hypotheses;
pub mod api_contract_versions;
pub mod approvals;
pub mod attribution_snapshots;
pub mod audit;
pub mod credential_rotation;
pub mod export_jobs;
pub mod freshness_gate;
pub mod incident_alerts;
pub mod incident_query_views;
pub mod market_bucket_profiles;
pub mod market_policy;
pub mod market_stream;
pub mod orders;
pub mod participation_guardrail_events;
pub mod pretrade_gate;
pub mod rbac;
pub mod reconciliation;
pub mod recovery_gate_runs;
pub mod regime_shift_alerts;
pub mod report_schedules;
pub mod reporting_read_models;
pub mod restore_rehearsals;
pub mod reward_risk;
pub mod risk_limits;
pub mod safety_controls;
pub mod shadow_evaluations;
pub mod user_stream;
pub mod validation_artifacts;
pub mod validation_gate_policies;
pub mod validation_runs;

pub fn migration_namespace() -> &'static str {
    "governance_rbac"
}
