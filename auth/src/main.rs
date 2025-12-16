#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use server::auth::auth_service_server::AuthServiceServer;
use server::AuthServiceImpl;
use shared::encryption::EncryptionService;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Auth service...");

    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "[::1]:8082".to_string()).parse()?;
    let db_addr = env::var("DB_ADDR").unwrap_or_else(|_| "http://[::1]:8080".to_string());
    let storage_path = env::var("STORAGE_PATH").unwrap_or_else(|_| "/tmp/ngi-auth".to_string());
    
    // Ensure storage path exists
    fs::create_dir_all(&storage_path)?;

    // Load or generate encryption keys
    let keys_path = Path::new(&storage_path).join("keys.bin");
    let keys = if keys_path.exists() {
        info!("Loading encryption keys from {:?}", keys_path);
        let bytes = fs::read(&keys_path)?;
        let (keys, _): ((Vec<u8>, Vec<u8>), usize) = bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
        keys
    } else {
        info!("Generating new encryption keys");
        let keys = EncryptionService::generate_keypair()
            .map_err(|e| anyhow::anyhow!("Failed to generate keys: {e}"))?;
        let bytes = bincode::serde::encode_to_vec(&keys, bincode::config::standard())?;
        fs::write(&keys_path, bytes)?;
        info!("Saved encryption keys to {:?}", keys_path);
        keys
    };

    // Load or generate JWT secret
    let jwt_secret = if let Ok(secret) = env::var("JWT_SECRET") {
        secret.into_bytes()
    } else {
        let jwt_secret_path = Path::new(&storage_path).join("jwt.secret");
        if jwt_secret_path.exists() {
            fs::read(&jwt_secret_path)?
        } else {
            let secret: [u8; 32] = rand::random();
            fs::write(&jwt_secret_path, secret)?;
            secret.to_vec()
        }
    };

    // Connect to DB
    info!("Connecting to DB at {}", db_addr);
    let db_client = server::db::database_client::DatabaseClient::connect(db_addr).await?;
    let db_client = Arc::new(Mutex::new(db_client));

    // Create service
    let auth_service = AuthServiceImpl::new(db_client, jwt_secret, keys);

    info!("Auth service listening on {}", listen_addr);

    Server::builder()
        .add_service(AuthServiceServer::new(auth_service))
        .serve(listen_addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
