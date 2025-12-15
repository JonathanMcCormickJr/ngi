#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use axum::{
    response::Json,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use tracing::{info, Level};

mod clients;
mod routes;

use clients::{CustodianClient, DbClient};

#[derive(Clone)]
pub struct AppState {
    custodian: CustodianClient,
    db: DbClient,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting LBRP service...");

    // Get configuration from environment
    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "[::]:443".to_string());
    let custodian_addr = std::env::var("CUSTODIAN_ADDR").unwrap_or_else(|_| "http://custodian:8081".to_string());
    let db_addr = std::env::var("DB_ADDR").unwrap_or_else(|_| "http://db:8080".to_string());

    // Create gRPC clients
    let custodian_client = CustodianClient::connect(custodian_addr).await?;
    let db_client = DbClient::connect(db_addr).await?;

    let state = AppState {
        custodian: custodian_client,
        db: db_client,
    };

    // Build the application with routes
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/ticket", post(routes::create_ticket))
        .route("/api/ticket/:id", get(routes::get_ticket))
        .route("/api/ticket/:id", axum::routing::put(routes::update_ticket))
        .route("/api/ticket/:id/lock", post(routes::acquire_lock))
        .route("/api/ticket/:id/lock", axum::routing::delete(routes::release_lock))
        .route("/api/cluster/status", get(routes::cluster_status))
        .route("/metrics", get(routes::metrics))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .layer(CompressionLayer::new())
        )
        .with_state(state);

    // Parse address
    let addr: SocketAddr = listen_addr.parse()?;

    info!("LBRP listening on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::Value::String("OK".to_string()))
}

#[cfg(test)]
mod tests;
