use common::time::timestamp_utc;
use reporting_service::api::{
    ContractVersionPort, PostgresContractVersionPort, ReportingApiState, ReportingQueryPort,
    UnavailableContractVersionPort, reporting_router,
};
use reporting_service::exports::scheduling::{ReportScheduleOrchestrator, ReportSchedulingService};
use reporting_service::read_models::queries::ReportingReadModelOrchestrator;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_SCHEDULER_TICK_SECONDS: u64 = 30;

fn scheduler_tick_interval() -> Duration {
    let seconds = std::env::var("REPORT_SCHEDULER_TICK_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_SCHEDULER_TICK_SECONDS);
    Duration::from_secs(seconds)
}

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
    let scheduler_orchestrator: Arc<dyn ReportScheduleOrchestrator> =
        match ReportSchedulingService::bootstrap_from_env() {
            Ok(Some(service)) => Arc::new(service),
            Ok(None) => Arc::new(ReportSchedulingService::default()),
            Err(error) => {
                eprintln!(
                    "reporting-service schedule bootstrap degraded at {}: {}",
                    started_at_utc, error
                );
                Arc::new(ReportSchedulingService::default())
            }
        };
    let _app = reporting_router(ReportingApiState::new(query_port, contract_port));
    println!(
        "reporting-service contract API scaffold ready at {} ({})",
        started_at_utc,
        scheduler_orchestrator.warmup_status()
    );

    let mut scheduler_ticks = tokio::time::interval(scheduler_tick_interval());
    loop {
        scheduler_ticks.tick().await;
        let scheduler_tick_at = timestamp_utc();
        if let Err(error) = scheduler_orchestrator.process_due_schedules(&scheduler_tick_at) {
            eprintln!(
                "reporting-service schedule tick failed at {}: {}",
                scheduler_tick_at, error
            );
        }
    }
}
