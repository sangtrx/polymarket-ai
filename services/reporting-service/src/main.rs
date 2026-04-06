use common::time::timestamp_utc;
use reporting_service::api::{
    ContractVersionPort, PostgresContractVersionPort, ReportingApiState, ReportingQueryPort,
    UnavailableContractVersionPort, reporting_router,
};
use reporting_service::read_models::queries::ReportingReadModelOrchestrator;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let started_at_utc = timestamp_utc();
    let query_port: Arc<dyn ReportingQueryPort> =
        match ReportingReadModelOrchestrator::bootstrap_from_env() {
            Ok(Some(orchestrator)) => Arc::new(orchestrator),
            Ok(None) => Arc::new(ReportingReadModelOrchestrator::without_pool()),
            Err(error) => {
                eprintln!(
                    "reporting-service read-model bootstrap degraded at {}: {}",
                    started_at_utc, error
                );
                Arc::new(ReportingReadModelOrchestrator::without_pool())
            }
        };
    let contract_port: Arc<dyn ContractVersionPort> = match std::env::var("DATABASE_URL") {
        Ok(database_url) => match PgPoolOptions::new()
            .max_connections(4)
            .connect_lazy(&database_url)
        {
            Ok(pool) => Arc::new(PostgresContractVersionPort::new(pool)),
            Err(error) => {
                eprintln!(
                    "reporting-service contract adapter unavailable at {}: {}",
                    started_at_utc, error
                );
                Arc::new(UnavailableContractVersionPort)
            }
        },
        Err(_) => Arc::new(UnavailableContractVersionPort),
    };

    let _app = reporting_router(ReportingApiState::new(query_port, contract_port));
    println!(
        "reporting-service contract API scaffold ready at {}",
        started_at_utc
    );
}
