mod approvals;
mod audit;

use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    println!("governance-service scaffold ready at {}", timestamp_utc());
}
