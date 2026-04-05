mod handlers;
mod middleware;
mod routes;

use axum::Router;
use common::time::timestamp_utc;
use domain::governance::AuthorizationEvaluator;
use governance_service::audit::{GovernanceAuditService, InMemoryAuditAppendPort};
use middleware::{ControlApiState, GovernanceAuthorizationGuard, HeaderTokenAuthenticator};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let state = ControlApiState::new(
        Arc::new(GovernanceAuthorizationGuard::new(
            AuthorizationEvaluator::default(),
        )),
        Arc::new(HeaderTokenAuthenticator),
        Arc::new(GovernanceAuditService::new(Arc::new(
            InMemoryAuditAppendPort::default(),
        ))),
    );
    let _app: Router = routes::app_router(state);
    println!("control-api bootstrap ready at {}", timestamp_utc());
}
