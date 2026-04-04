mod handlers;
mod middleware;
mod routes;

use axum::{Router, routing::get};
use common::time::timestamp_utc;

#[tokio::main]
async fn main() {
    let _app: Router = Router::new().route("/health", get(routes::health));
    println!("control-api bootstrap ready at {}", timestamp_utc());
}
