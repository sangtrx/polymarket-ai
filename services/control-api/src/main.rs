mod handlers;
mod middleware;
mod routes;

use axum::Router;
use common::time::timestamp_utc;
use domain::governance::AuthorizationEvaluator;
use governance_service::allocation_policy::AllocationPolicyService;
use governance_service::approvals::GovernanceApprovalService;
use governance_service::audit::{GovernanceAuditService, InMemoryAuditAppendPort};
use governance_service::credentials::CredentialRotationService;
use governance_service::market_policy::MarketPolicyService;
use governance_service::recovery::RecoveryService;
use governance_service::reward_risk::RewardRiskService;
use governance_service::risk_limits::RiskLimitService;
use governance_service::safety_controls::SafetyControlService;
use middleware::{ControlApiState, GovernanceAuthorizationGuard, HeaderTokenAuthenticator};
use reporting_service::exports::scheduling::ReportSchedulingService;
use reporting_service::exports::workflows::ReportExportWorkflowService;
use research_gateway::validation::gate_policies::{
    ValidationGatePolicyOrchestrator, ValidationGatePolicyService,
};
use research_gateway::validation::hypothesis_registry::HypothesisRegistryService;
use research_gateway::validation::shadow_mode::ShadowModeService;
use research_gateway::validation::workflow_runs::ValidationWorkflowRunService;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for durable approval persistence");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to Postgres for approval persistence");
    let research_validation_gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator> =
        Arc::new(ValidationGatePolicyService::postgres(pool.clone()));
    let research_validation_workflow_orchestrator =
        Arc::new(ValidationWorkflowRunService::postgres(
            pool.clone(),
            Arc::clone(&research_validation_gate_orchestrator),
        ));
    let research_shadow_mode_orchestrator = Arc::new(ShadowModeService::postgres(pool.clone()));

    let state = ControlApiState::with_all_orchestrators_and_reporting(
        Arc::new(GovernanceAuthorizationGuard::new(
            AuthorizationEvaluator::default(),
        )),
        Arc::new(HeaderTokenAuthenticator),
        Arc::new(GovernanceAuditService::new(Arc::new(
            InMemoryAuditAppendPort::default(),
        ))),
        Arc::new(GovernanceApprovalService::postgres(pool.clone())),
        Arc::new(CredentialRotationService::postgres(pool.clone())),
        Arc::new(AllocationPolicyService::postgres(pool.clone())),
        Arc::new(MarketPolicyService::postgres(pool.clone())),
        Arc::new(RiskLimitService::postgres(pool.clone())),
        Arc::new(SafetyControlService::postgres(pool.clone())),
        Arc::new(ReportSchedulingService::postgres(pool.clone())),
        Arc::new(RecoveryService::postgres(pool.clone())),
    )
    .with_reward_risk_orchestrator(Arc::new(RewardRiskService::postgres(pool.clone())))
    .with_report_export_orchestrator(Arc::new(ReportExportWorkflowService::postgres(
        pool.clone(),
    )))
    .with_research_hypothesis_orchestrator(Arc::new(HypothesisRegistryService::postgres(
        pool.clone(),
    )))
    .with_research_validation_gate_orchestrator(research_validation_gate_orchestrator)
    .with_research_validation_workflow_orchestrator(research_validation_workflow_orchestrator)
    .with_research_shadow_mode_orchestrator(research_shadow_mode_orchestrator)
    .with_attribution_pool(pool);
    let _app: Router = routes::app_router(state);
    println!("control-api bootstrap ready at {}", timestamp_utc());
}
