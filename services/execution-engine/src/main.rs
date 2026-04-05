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
    let store = ingestion::PostgresMarketStreamStore::new(pool.clone());
    let mut runtime = ingestion::MarketStreamIngestionRuntime::new(store, config);
    let user_stream_enabled = std::env::var("EXECUTION_USER_STREAM_ENABLED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    if user_stream_enabled {
        let user_config = ingestion::user_stream::UserStreamRuntimeConfig::from_env()
            .expect("execution user stream runtime configuration is invalid");
        let user_store = ingestion::user_stream::PostgresUserStreamStore::new(pool.clone());
        let mut user_runtime =
            ingestion::user_stream::UserStreamIngestionRuntime::new(user_store, user_config);
        println!(
            "execution-engine market/user-stream bootstrap ready at {}",
            timestamp_utc()
        );

        let market_runtime = ingestion::run_polymarket_ws_ingestion(&mut runtime);
        let user_stream_runtime =
            ingestion::user_stream::run_polymarket_user_ws_ingestion(&mut user_runtime);
        tokio::pin!(market_runtime);
        tokio::pin!(user_stream_runtime);

        tokio::select! {
            market_result = &mut market_runtime => {
                match market_result {
                    Ok(()) => {
                        panic!("execution-engine market-stream runtime exited unexpectedly fail-closed");
                    }
                    Err(error) => {
                        panic!("execution-engine market-stream runtime halted fail-closed: {error}");
                    }
                }
            }
            user_result = &mut user_stream_runtime => {
                match user_result {
                    Ok(()) => {
                        panic!("execution-engine user-stream runtime exited unexpectedly fail-closed");
                    }
                    Err(error) => {
                        panic!("execution-engine user-stream runtime halted fail-closed: {error}");
                    }
                }
            }
        }
    } else {
        println!(
            "execution-engine market-stream bootstrap ready at {}",
            timestamp_utc()
        );
        if let Err(error) = ingestion::run_polymarket_ws_ingestion(&mut runtime).await {
            panic!("execution-engine market-stream runtime halted fail-closed: {error}");
        }
    }
}
