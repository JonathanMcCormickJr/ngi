#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use db::network::DbNetworkFactory;
use db::raft::{DbRaft, DbStore};
use db::raft_service::RaftServiceImpl;
use db::server::db::{database_server::DatabaseServer, raft_service_server::RaftServiceServer};
use db::server::DatabaseService;
use openraft::storage::Adaptor;
use openraft::Config;
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
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "[::1]:50051".to_string());
    let storage_path = env::var("STORAGE_PATH")
        .unwrap_or_else(|_| format!("/tmp/ngi-db-{}", node_id));

    // Parse peer nodes configuration
    // Format: "1:http://127.0.0.1:50051,2:http://127.0.0.1:50052,3:http://127.0.0.1:50053"
    let peers_str = env::var("RAFT_PEERS").unwrap_or_else(|_| {
        // Default single-node cluster
        format!("{}:{}", node_id, listen_addr)
    });

    let mut network_factory = DbNetworkFactory::new();
    let mut all_peers = Vec::new();

    for peer_config in peers_str.split(',') {
        let parts: Vec<&str> = peer_config.trim().split(':').collect();
        if parts.len() >= 2 {
            let peer_id: u64 = parts[0].parse().unwrap_or(0);
            let peer_addr = parts[1..].join(":");
            if peer_id > 0 {
                network_factory.add_node(peer_id, peer_addr).await;
                all_peers.push(peer_id);
                info!("Configured peer: node_id={}, addr={}", peer_id, parts[1]);
            }
        }
    }

    info!("Starting DB service node {} on {}", node_id, listen_addr);
    info!("Storage path: {}", storage_path);
    info!("Cluster peers: {:?}", all_peers);

    // Create storage
    let store = DbStore::new(&storage_path).await?;

    info!("Storage initialized at {}", storage_path);

    // Create Raft configuration
    let config = Config {
        heartbeat_interval: 500,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        ..Default::default()
    };
    let config = Arc::new(config.validate()?);

    // Split store into log storage and state machine using Adaptor
    let (log_store, state_machine) = Adaptor::new(store.clone());

    // Create Raft instance
    let raft = Arc::new(DbRaft::new(node_id, config, network_factory, log_store, state_machine).await?);

    info!("Raft node {} initialized", node_id);

    // Initialize cluster if this is node 1 or if peers are defined
    if node_id == all_peers.first().copied().unwrap_or(1) && !all_peers.is_empty() {
        info!("Node {} is first peer, initializing cluster with {:?}", node_id, all_peers);

        let mut members = std::collections::BTreeSet::new();
        for peer_id in &all_peers {
            members.insert(*peer_id);
        }

        // Try to initialize the cluster
        match raft.initialize(members).await {
            Ok(_) => info!("Cluster initialized successfully"),
            Err(e) => {
                // It's OK if already initialized
                info!("Cluster initialization returned: {}", e);
            }
        }
    } else {
        info!("Node {} is not the first peer, skipping initialization", node_id);
    }

    info!("DB service node {} ready", node_id);

    // Parse address
    let addr = listen_addr.parse::<std::net::SocketAddr>()?;

    // Get storage for read operations
    let storage = store.state_machine().read().await.storage.clone();

    // Create gRPC services
    let db_service = DatabaseService::new((*raft).clone(), storage);
    let raft_service = RaftServiceImpl::new(raft);

    info!("Starting gRPC server on {}", addr);

    // Start gRPC server with both services
    Server::builder()
        .add_service(DatabaseServer::new(db_service))
        .add_service(RaftServiceServer::new(raft_service))
        .serve(addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
