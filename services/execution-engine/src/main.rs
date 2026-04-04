mod ingestion;
mod orders;
mod reconciliation;

use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    println!("execution-engine bootstrap ready at {}", timestamp_utc());
}
