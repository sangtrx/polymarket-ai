mod contracts;
mod exports;

use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    println!("reporting-service scaffold ready at {}", timestamp_utc());
}
