mod promotion;
mod validation;

use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    println!("research-gateway scaffold ready at {}", timestamp_utc());
}
