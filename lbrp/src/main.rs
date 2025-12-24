#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

mod clients;
mod middleware;
mod routes;

use clients::{AuthClient, AdminClient, CustodianClient};
use middleware::AuthState;
use routes::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;
    
    // Service addresses (in k8s/docker-compose these would be service names)
    let auth_addr = std::env::var("AUTH_ADDR").unwrap_or_else(|_| "http://auth:8082".to_string());
    let admin_addr = std::env::var("ADMIN_ADDR").unwrap_or_else(|_| "http://admin:8083".to_string());
    let custodian_addr = std::env::var("CUSTODIAN_ADDR").unwrap_or_else(|_| "http://custodian-leader:8081".to_string());

    info!("LBRP Service starting on {}", addr);

    // Connect to backend services
    let auth_client = AuthClient::connect(auth_addr.clone()).await?;
    let admin_client = AdminClient::connect(admin_addr.clone()).await?;
    let custodian_client = CustodianClient::connect(custodian_addr.clone()).await?;

    // JWT Secret (must match Auth service)
    // In production, load from secure vault/env
    let jwt_secret = std::env::var("JWT_SECRET")
        .map_or_else(|_| b"secret".to_vec(), String::into_bytes); 

    let app_state = AppState {
        auth_client,
        admin_client,
        custodian_client,
        auth_state: Arc::new(AuthState { jwt_secret }),
    };

    let app = routes::app(app_state)
        .fallback_service(tower_http::services::ServeDir::new("../web/dist")
            .fallback(tower_http::services::ServeFile::new("../web/dist/index.html")));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests;
