#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use custodian::network::CustodianNetworkFactory;
use custodian::raft::{CustodianRaft, CustodianStore};
use custodian::server::{CustodianServiceImpl, create_server};
use openraft::Config;
use openraft::storage::Adaptor;
use shared::encryption::EncryptionService;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tonic::transport::Server;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
#[allow(clippy::too_many_lines)]
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
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "[::1]:8081".to_string());
    let storage_path =
        env::var("STORAGE_PATH").unwrap_or_else(|_| format!("/tmp/ngi-custodian-{node_id}"));

    // Parse peer nodes configuration
    // Format: "1:http://127.0.0.1:8081,2:http://127.0.0.1:8082,3:http://127.0.0.1:8083"
    let peers_str = env::var("RAFT_PEERS").unwrap_or_else(|_| {
        // Default single-node cluster
        format!("{node_id}:{listen_addr}")
    });

    let mut network_factory = CustodianNetworkFactory::new();
    let mut all_peers = Vec::new();

    for peer_config in peers_str.split(',') {
        let parts: Vec<&str> = peer_config.trim().split(':').collect();
        if parts.len() >= 2 {
            let peer_id: u64 = parts[0].parse().unwrap_or(0);
            let peer_addr = parts[1..].join(":");
            if peer_id > 0 {
                network_factory.add_node(peer_id, peer_addr);
                all_peers.push(peer_id);
                info!("Configured peer: node_id={}, addr={}", peer_id, parts[1]);
            }
        }
    }

    info!(
        "Starting Custodian service node {} on {}",
        node_id, listen_addr
    );
    info!("Storage path: {}", storage_path);
    info!("Cluster peers: {:?}", all_peers);

    // Create storage
    let store = CustodianStore::new(&storage_path)?;
    let storage = store.storage().clone();

    info!("Storage initialized at {}", storage_path);

    // Load or generate encryption keys
    let keys_path = Path::new(&storage_path).join("keys.bin");
    let keys = if keys_path.exists() {
        info!("Loading encryption keys from {:?}", keys_path);
        let bytes = fs::read(&keys_path)?;
        let keys: (Vec<u8>, Vec<u8>) = postcard::from_bytes(&bytes)?;
        keys
    } else {
        info!("Generating new encryption keys");
        let keys = EncryptionService::generate_keypair()
            .map_err(|e| anyhow::anyhow!("Failed to generate keys: {e}"))?;
        let bytes = postcard::to_allocvec(&keys)?;
        fs::write(&keys_path, bytes)?;
        info!("Saved encryption keys to {:?}", keys_path);
        keys
    };

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
    let raft: Arc<CustodianRaft> = Arc::new(
        CustodianRaft::new(node_id, config, network_factory, log_store, state_machine).await?,
    );

    info!("Raft node {} initialized", node_id);

    // Initialize cluster if this is node 1 or if peers are defined
    if node_id == all_peers.first().copied().unwrap_or(1) && !all_peers.is_empty() {
        info!(
            "Node {} is first peer, initializing cluster with {:?}",
            node_id, all_peers
        );

        let mut members = std::collections::BTreeSet::new();
        for peer_id in &all_peers {
            members.insert(*peer_id);
        }

        // Try to initialize the cluster
        match raft.initialize(members).await {
            Ok(()) => info!("Cluster initialized successfully"),
            Err(e) => {
                // It's OK if already initialized
                info!("Cluster initialization returned: {}", e);
            }
        }
    } else {
        info!(
            "Node {} is not the first peer, skipping initialization",
            node_id
        );
    }

    info!("Custodian service node {} ready", node_id);

    // Parse address
    let addr = listen_addr.parse::<std::net::SocketAddr>()?;

    // Create gRPC service; optionally connect to DB leader
    let db_client = if let Ok(db_endpoint) = std::env::var("DB_LEADER_ADDR") {
        match custodian::db_client::DbClient::connect(db_endpoint).await {
            Ok(c) => Some(std::sync::Arc::new(tokio::sync::Mutex::new(c))),
            Err(e) => {
                tracing::warn!("failed to connect to db: {}", e);
                None
            }
        }
    } else {
        None
    };

    let custodian_service = if let Some(db) = db_client {
        CustodianServiceImpl::with_db_client((*raft).clone(), storage.clone(), db, keys)
    } else {
        CustodianServiceImpl::new((*raft).clone(), storage.clone(), keys)
    };

    info!("Starting gRPC server on {}", addr);

    // Start gRPC server
    Server::builder()
        .add_service(create_server(custodian_service))
        .serve(addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
