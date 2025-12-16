#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

mod server;

use server::AdminServiceImpl;
use server::admin::admin_service_server::AdminServiceServer;
use tonic::transport::Server;
use tracing::info;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use shared::encryption::EncryptionService;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8083".to_string())
        .parse()?;
    let db_addr = std::env::var("DB_ADDR")
        .unwrap_or_else(|_| "http://db-leader:8080".to_string());
    let storage_path = std::env::var("STORAGE_PATH").unwrap_or_else(|_| "/tmp/ngi-admin".to_string());

    info!("Admin Service starting on {}", addr);

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
        let keys = EncryptionService::generate_keypair()?;
        let bytes = bincode::serde::encode_to_vec(&keys, bincode::config::standard())?;
        fs::write(&keys_path, bytes)?;
        info!("Saved encryption keys to {:?}", keys_path);
        keys
    };

    // Connect to DB
    info!("Connecting to DB at {}", db_addr);
    let db_client = server::db::database_client::DatabaseClient::connect(db_addr).await?;
    let db_client = Arc::new(Mutex::new(db_client));

    let admin_service = AdminServiceImpl::new(db_client, keys);

    Server::builder()
        .add_service(AdminServiceServer::new(admin_service))
        .serve(addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
