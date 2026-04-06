use common::time::timestamp_utc;
use reporting_service::read_models::queries::ReportingReadModelOrchestrator;

#[tokio::main]
async fn main() {
    let started_at_utc = timestamp_utc();
    match ReportingReadModelOrchestrator::bootstrap_from_env() {
        Ok(Some(orchestrator)) => {
            println!(
                "reporting-service read-model seam ready at {} ({})",
                started_at_utc,
                orchestrator.warmup_status()
            );
        }
        Ok(None) => {
            println!(
                "reporting-service scaffold ready at {}; DATABASE_URL not set so read-model seam remains cold",
                started_at_utc
            );
        }
        Err(error) => {
            eprintln!(
                "reporting-service bootstrap degraded at {}: {}",
                started_at_utc, error
            );
        }
    }
}
