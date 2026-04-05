mod ingestion;
mod orders;
mod reconciliation;

use common::time::timestamp_utc;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for durable market stream persistence");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to Postgres for market stream ingestion");

    let config = ingestion::MarketStreamRuntimeConfig::from_env()
        .expect("execution market stream runtime configuration is invalid");
    let store = ingestion::PostgresMarketStreamStore::new(pool);
    let mut runtime = ingestion::MarketStreamIngestionRuntime::new(store, config);

    println!(
        "execution-engine market-stream bootstrap ready at {}",
        timestamp_utc()
    );
    if let Err(error) = ingestion::run_polymarket_ws_ingestion(&mut runtime).await {
        panic!("execution-engine market-stream runtime halted fail-closed: {error}");
    }
}
