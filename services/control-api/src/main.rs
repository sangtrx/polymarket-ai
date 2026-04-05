mod handlers;
mod middleware;
mod routes;

use axum::Router;
use common::time::timestamp_utc;
use domain::governance::AuthorizationEvaluator;
use governance_service::approvals::GovernanceApprovalService;
use governance_service::audit::{GovernanceAuditService, InMemoryAuditAppendPort};
use governance_service::credentials::CredentialRotationService;
use governance_service::market_policy::MarketPolicyService;
use middleware::{ControlApiState, GovernanceAuthorizationGuard, HeaderTokenAuthenticator};
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

    let state = ControlApiState::with_all_orchestrators(
        Arc::new(GovernanceAuthorizationGuard::new(
            AuthorizationEvaluator::default(),
        )),
        Arc::new(HeaderTokenAuthenticator),
        Arc::new(GovernanceAuditService::new(Arc::new(
            InMemoryAuditAppendPort::default(),
        ))),
        Arc::new(GovernanceApprovalService::postgres(pool.clone())),
        Arc::new(CredentialRotationService::postgres(pool.clone())),
        Arc::new(MarketPolicyService::postgres(pool)),
    );
    let _app: Router = routes::app_router(state);
    println!("control-api bootstrap ready at {}", timestamp_utc());
}
