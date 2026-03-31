#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

mod clients;
mod middleware;
mod routes;

use clients::{AdminClient, AuthClient, CustodianClient};
use middleware::AuthState;
use routes::AppState;

pub(crate) fn parse_listen_addr(
    raw: Option<String>,
) -> Result<SocketAddr, std::net::AddrParseError> {
    raw.unwrap_or_else(|| "0.0.0.0:8080".to_string()).parse()
}

pub(crate) fn env_or_default(raw: Option<String>, default_value: &str) -> String {
    raw.unwrap_or_else(|| default_value.to_string())
}

pub(crate) fn jwt_secret_from_env(raw: Option<String>) -> Vec<u8> {
    raw.map_or_else(|| b"secret".to_vec(), String::into_bytes)
}

/// Resolves the three backend service addresses from optional raw values, falling back to
/// defaults suitable for container service-name routing.
///
/// Returns `(auth_addr, admin_addr, custodian_addr)`.
pub(crate) fn resolve_backend_addrs(
    auth_raw: Option<String>,
    admin_raw: Option<String>,
    custodian_raw: Option<String>,
) -> (String, String, String) {
    (
        env_or_default(auth_raw, "http://auth:8082"),
        env_or_default(admin_raw, "http://admin:8083"),
        env_or_default(custodian_raw, "http://custodian-leader:8081"),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = parse_listen_addr(std::env::var("LISTEN_ADDR").ok())?;

    // Service addresses (in k8s/docker-compose these would be service names)
    let (auth_addr, admin_addr, custodian_addr) = resolve_backend_addrs(
        std::env::var("AUTH_ADDR").ok(),
        std::env::var("ADMIN_ADDR").ok(),
        std::env::var("CUSTODIAN_ADDR").ok(),
    );

    info!("LBRP Service starting on {}", addr);

    // Connect to backend services
    let auth_client = AuthClient::connect(auth_addr.clone()).await?;
    let admin_client = AdminClient::connect(admin_addr.clone()).await?;
    let custodian_client = CustodianClient::connect(custodian_addr.clone()).await?;

    // JWT Secret (must match Auth service)
    // In production, load from secure vault/env
    let jwt_secret = jwt_secret_from_env(std::env::var("JWT_SECRET").ok());

    let app_state = AppState {
        auth_client,
        admin_client,
        custodian_client,
        auth_state: Arc::new(AuthState { jwt_secret }),
    };

    let app = routes::app(app_state).fallback_service(
        tower_http::services::ServeDir::new("../web/dist").fallback(
            tower_http::services::ServeFile::new("../web/dist/index.html"),
        ),
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests;
