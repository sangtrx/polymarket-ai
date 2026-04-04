mod allocation;
mod attribution;

use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    println!("portfolio-engine scaffold ready at {}", timestamp_utc());
}
