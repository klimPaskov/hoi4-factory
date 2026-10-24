//! HOI4 Factory server. S01 skeleton: a health endpoint only, no product features.

use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = std::env::var("FACTORY_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
    let app = Router::new().route("/healthz", get(healthz));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
