#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use db::network::DbNetworkFactory;
use db::raft::{DbRaft, DbStore};
use db::server::db::database_server::DatabaseServer;
use db::server::DatabaseService;
use openraft::{storage::Adaptor, Config};
use std::env;
use std::sync::Arc;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Get configuration from environment
    let node_id: u64 = env::var("NODE_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse()?;
    let addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "[::1]:50051".to_string());
    let storage_path = env::var("STORAGE_PATH")
        .unwrap_or_else(|_| format!("/tmp/ngi-db-{}", node_id));

    info!("Starting DB service node {} on {}", node_id, addr);
    info!("Storage path: {}", storage_path);

    // Create storage
    let store = DbStore::new(&storage_path)?;
    
    info!("Storage initialized at {}", storage_path);
    
    // Create Raft configuration
    let config = Config {
        heartbeat_interval: 500,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        ..Default::default()
    };
    let config = Arc::new(config.validate()?);
    
    // Create network factory
    let network = DbNetworkFactory::new();
    
    // Split store into log storage and state machine using Adaptor
    let (log_store, state_machine) = Adaptor::new(store.clone());
    
    // Create Raft instance
    let raft = DbRaft::new(node_id, config, network, log_store, state_machine).await?;
    
    info!("Raft node {} initialized", node_id);
    
    // Initialize single-node cluster if this is node 1
    if node_id == 1 {
        info!("Initializing single-node cluster");
        let mut nodes = std::collections::BTreeSet::new();
        nodes.insert(node_id);
        raft.initialize(nodes).await?;
        info!("Single-node cluster initialized");
    }
    
    info!("DB service node {} ready", node_id);

    // Parse address
    let addr = addr.parse::<std::net::SocketAddr>()?;

    // Get storage for read operations
    let storage = store.state_machine().read().await.storage.clone();
    
    // Create gRPC service
    let service = DatabaseService::new(raft, storage);
    
    info!("Starting gRPC server on {}", addr);
    
    // Start gRPC server
    Server::builder()
        .add_service(DatabaseServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
