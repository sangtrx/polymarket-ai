mod gates;
mod limits;
mod safe_state;

use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    println!("risk-engine bootstrap ready at {}", timestamp_utc());
}
