//! Network layer for inter-node communication via gRPC
//!
//! This module implements the `RaftNetwork` trait from openraft to provide
//! inter-node RPC communication for distributed Raft consensus.

use crate::raft::DbTypeConfig;
use async_trait::async_trait;
use openraft::network::RaftNetworkFactory;
use openraft::RaftNetwork;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Factory for creating network clients
pub struct DbNetworkFactory {
    peers: Arc<HashMap<u64, String>>,
}

impl DbNetworkFactory {
    pub fn new(peers: HashMap<u64, String>) -> Self {
        Self {
            peers: Arc::new(peers),
        }
    }

    fn get_peer_address(&self, node_id: u64) -> Option<String> {
        self.peers.get(&node_id).cloned()
    }
}

/// Implementation of openraft RaftNetworkFactory
#[async_trait]
impl RaftNetworkFactory<DbTypeConfig> for DbNetworkFactory {
    type Network = DbNetwork;

    async fn connect(
        &mut self,
        target: u64,
        _node: &openraft::BasicNode,
    ) -> std::io::Result<Self::Network> {
        let address = self
            .get_peer_address(target)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no address configured for node {}", target),
                )
            })?;

        debug!(
            "creating network client for node {} at {}",
            target, address
        );

        Ok(DbNetwork { target, address })
    }
}

pub struct DbNetwork {
    target: u64,
    address: String,
}

#[async_trait]
impl RaftNetwork<DbTypeConfig> for DbNetwork {
    async fn vote(
        &mut self,
        _rpc: openraft::raft::VoteRequest<u64>,
        _opt: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::VoteResponse<u64>,
        openraft::error::RPCError<u64, openraft::error::RaftError<u64>>,
    > {
        // TODO: Implement vote RPC
        Err(openraft::error::RaftError::Unreachable.into())
    }

    async fn append_entries(
        &mut self,
        _rpc: openraft::raft::AppendEntriesRequest<DbTypeConfig>,
        _opt: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::AppendEntriesResponse<u64>,
        openraft::error::RPCError<u64, openraft::error::RaftError<u64>>,
    > {
        // TODO: Implement append_entries RPC
        Err(openraft::error::RaftError::Unreachable.into())
    }

    async fn full_snapshot(
        &mut self,
        _vote: openraft::Vote<u64>,
        _snapshot: openraft::Snapshot<DbTypeConfig>,
        _cancel: impl std::future::Future<Output = openraft::error::ReplicationClosed> + Send + 'static,
        _opt: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::SnapshotResponse<u64>,
        openraft::error::StreamingError<DbTypeConfig, openraft::error::Fatal<u64>>,
    > {
        // TODO: Implement snapshot streaming
        Err(openraft::error::Fatal::Stopped.into())
    }

    async fn install_snapshot(
        &mut self,
        _rpc: openraft::raft::InstallSnapshotRequest<DbTypeConfig>,
        _opt: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::InstallSnapshotResponse<u64>,
        openraft::error::RPCError<u64, openraft::error::RaftError<u64>, openraft::error::InstallSnapshotError>,
    > {
        // TODO: Implement snapshot installation RPC
        Err(openraft::error::RaftError::Unreachable.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_factory_creation() {
        let mut peers = HashMap::new();
        peers.insert(1, "http://127.0.0.1:50051".to_string());
        let factory = DbNetworkFactory::new(peers);

        assert!(factory.get_peer_address(1).is_some());
        assert!(factory.get_peer_address(99).is_none());
    }
}
