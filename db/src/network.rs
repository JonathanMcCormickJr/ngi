//! Network layer for inter-node communication via gRPC
//!
//! This module implements the `RaftNetwork` trait from openraft to provide
//! inter-node RPC communication for distributed Raft consensus.

use crate::raft::DbTypeConfig;
use openraft::network::RaftNetworkFactory;
use openraft::RaftNetwork;
use std::collections::HashMap;
use std::sync::Arc;

/// Network client for communicating with a specific Raft peer
pub struct DbNetwork {
    target: u64,
    address: String,
    client: Option<crate::server::db::raft_service_client::RaftServiceClient<tonic::transport::Channel>>,
}

/// Factory for creating network clients
pub struct DbNetworkFactory {
    peers: Arc<HashMap<u64, String>>,
}

impl DbNetworkFactory {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(HashMap::new()),
        }
    }

    pub fn with_peers(peers: HashMap<u64, String>) -> Self {
        Self {
            peers: Arc::new(peers),
        }
    }

    pub fn add_node(&mut self, node_id: u64, address: String) {
        let mut peers = (*self.peers).clone();
        peers.insert(node_id, address);
        self.peers = Arc::new(peers);
    }

    fn get_peer_address(&self, node_id: u64) -> Option<String> {
        self.peers.get(&node_id).cloned()
    }
}

/// Implementation of openraft RaftNetworkFactory
impl RaftNetworkFactory<DbTypeConfig> for DbNetworkFactory {
    type Network = DbNetwork;

    fn new_client(
        &mut self,
        target: u64,
        node: &openraft::BasicNode,
    ) -> impl std::future::Future<Output = Self::Network> + Send {
        let address = self
            .get_peer_address(target)
            .unwrap_or_else(|| format!("http://{}:8080", node.addr));
        let target = target;

        async move {
            // For now, create without connecting - connect on first use
            DbNetwork {
                target,
                address,
                client: None,
            }
        }
    }
}

impl DbNetwork {
    async fn get_client(&mut self) -> Result<&mut crate::server::db::raft_service_client::RaftServiceClient<tonic::transport::Channel>, openraft::error::NetworkError> {
        if self.client.is_none() {
            match crate::server::db::raft_service_client::RaftServiceClient::connect(self.address.clone()).await {
                Ok(client) => {
                    self.client = Some(client);
                }
                Err(e) => {
                    return Err(openraft::error::NetworkError::new(&e));
                }
            }
        }
        Ok(self.client.as_mut().unwrap())
    }
}

impl RaftNetwork<DbTypeConfig> for DbNetwork {
    fn vote(
        &mut self,
        rpc: openraft::raft::VoteRequest<u64>,
        option: openraft::network::RPCOption,
    ) -> impl std::future::Future<Output = Result<
        openraft::raft::VoteResponse<u64>,
        openraft::error::RPCError<u64, openraft::BasicNode, openraft::error::RaftError<u64, openraft::error::Infallible>>,
    >> + Send {
        let rpc = rpc.clone(); // Clone for the async block
        async move {
            let client = self.get_client().await.map_err(openraft::error::RPCError::Network)?;
            
            // Convert to proto
            let proto_req = crate::server::db::VoteRequest {
                vote: Some(crate::server::db::ProtoVote {
                    term: rpc.vote.leader_id.term,
                    node_id: rpc.vote.leader_id.node_id,
                    committed: rpc.vote.committed,
                }),
                last_log_id: rpc.last_log_id.map(|log_id| crate::server::db::ProtoLogId {
                    term: log_id.leader_id.term,
                    index: log_id.index,
                }),
            };
            
            // Call gRPC
            let response = client.vote(proto_req).await
                .map_err(|e| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&e)))?;
            
            let resp = response.into_inner();
            
            // Convert back to OpenRaft types
            let proto_vote = resp.vote.ok_or_else(|| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&std::io::Error::new(std::io::ErrorKind::InvalidData, "missing vote"))))?;
            
            let vote_response = openraft::raft::VoteResponse {
                vote: openraft::Vote {
                    leader_id: openraft::LeaderId {
                        term: proto_vote.term,
                        node_id: proto_vote.node_id,
                    },
                    committed: proto_vote.committed,
                },
                vote_granted: resp.vote_granted,
                last_log_id: resp.last_log_id.map(|log_id| openraft::LogId {
                    leader_id: openraft::LeaderId {
                        term: log_id.term,
                        node_id: proto_vote.node_id, // Use the node_id from vote
                    },
                    index: log_id.index,
                }),
            };
            
            Ok(vote_response)
        }
    }

    fn append_entries(
        &mut self,
        rpc: openraft::raft::AppendEntriesRequest<DbTypeConfig>,
        option: openraft::network::RPCOption,
    ) -> impl std::future::Future<Output = Result<
        openraft::raft::AppendEntriesResponse<u64>,
        openraft::error::RPCError<u64, openraft::BasicNode, openraft::error::RaftError<u64, openraft::error::Infallible>>,
    >> + Send {
        let rpc = rpc.clone(); // Clone for the async block
        async move {
            let client = self.get_client().await.map_err(openraft::error::RPCError::Network)?;
            
            // Convert entries to protobuf
            let proto_entries: Vec<crate::server::db::Entry> = rpc.entries
                .into_iter()
                .map(|entry| {
                    Ok::<_, serde_json::Error>(crate::server::db::Entry {
                        data: serde_json::to_vec(&entry)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&e)))?;
            
            // Convert to proto
            let proto_req = crate::server::db::AppendEntriesRequest {
                vote: Some(crate::server::db::ProtoVote {
                    term: rpc.vote.leader_id.term,
                    node_id: rpc.vote.leader_id.node_id,
                    committed: rpc.vote.committed,
                }),
                prev_log_id: rpc.prev_log_id.map(|log_id| crate::server::db::ProtoLogId {
                    term: log_id.leader_id.term,
                    index: log_id.index,
                }),
                entries: proto_entries,
                leader_commit: rpc.leader_commit.map(|log_id| crate::server::db::ProtoLogId {
                    term: log_id.leader_id.term,
                    index: log_id.index,
                }),
            };
            
            // Call gRPC
            let response = client.append_entries(proto_req).await
                .map_err(|e| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&e)))?;
            
            let resp = response.into_inner();
            
            // Convert back to OpenRaft types
            let proto_vote = resp.vote.ok_or_else(|| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&std::io::Error::new(std::io::ErrorKind::InvalidData, "missing vote"))))?;
            
            let append_response = openraft::raft::AppendEntriesResponse::Success;
            
            Ok(append_response)
        }
    }

    fn full_snapshot(
        &mut self,
        _vote: openraft::Vote<u64>,
        _snapshot: openraft::Snapshot<DbTypeConfig>,
        _cancel: impl std::future::Future<Output = openraft::error::ReplicationClosed> + Send + 'static,
        _opt: openraft::network::RPCOption,
    ) -> impl std::future::Future<Output = Result<
        openraft::raft::SnapshotResponse<u64>,
        openraft::error::StreamingError<DbTypeConfig, openraft::error::Fatal<u64>>,
    >> + Send {
        async move {
            // TODO: Implement snapshot streaming
            Err(openraft::error::StreamingError::Closed(openraft::error::ReplicationClosed::new(&std::io::Error::new(std::io::ErrorKind::Other, "not implemented"))))
        }
    }

    fn install_snapshot(
        &mut self,
        rpc: openraft::raft::InstallSnapshotRequest<DbTypeConfig>,
        option: openraft::network::RPCOption,
    ) -> impl std::future::Future<Output = Result<
        openraft::raft::InstallSnapshotResponse<u64>,
        openraft::error::RPCError<u64, openraft::BasicNode, openraft::error::RaftError<u64, openraft::error::InstallSnapshotError>>,
    >> + Send {
        let rpc = rpc.clone(); // Clone for the async block
        async move {
            let client = self.get_client().await.map_err(openraft::error::RPCError::Network)?;
            
            // Convert to proto
            let proto_req = crate::server::db::InstallSnapshotRequest {
                vote: Some(crate::server::db::ProtoVote {
                    term: rpc.vote.leader_id.term,
                    node_id: rpc.vote.leader_id.node_id,
                    committed: rpc.vote.committed,
                }),
                meta: Some(crate::server::db::SnapshotMeta {
                    last_log_id: rpc.meta.last_log_id.map(|log_id| crate::server::db::ProtoLogId {
                        term: log_id.leader_id.term,
                        index: log_id.index,
                    }),
                    last_applied: rpc.meta.last_log_id.map(|log_id| log_id.index).unwrap_or(0),
                    last_membership: rpc.meta.last_membership.log_id().map(|log_id| log_id.index).unwrap_or(0) as u32,
                    snapshot_id: rpc.meta.snapshot_id,
                }),
                offset: rpc.offset,
                data: rpc.data,
                done: rpc.done,
            };
            
            // Call gRPC
            let response = client.install_snapshot(proto_req).await
                .map_err(|e| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&e)))?;
            
            let resp = response.into_inner();
            
            // Convert back to OpenRaft types
            let proto_vote = resp.vote.ok_or_else(|| openraft::error::RPCError::Network(openraft::error::NetworkError::new(&std::io::Error::new(std::io::ErrorKind::InvalidData, "missing vote"))))?;
            
            let install_response = openraft::raft::InstallSnapshotResponse {
                vote: openraft::Vote {
                    leader_id: openraft::LeaderId {
                        term: proto_vote.term,
                        node_id: proto_vote.node_id,
                    },
                    committed: proto_vote.committed,
                },
            };
            
            Ok(install_response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_factory_creation() {
        let mut peers = HashMap::new();
        peers.insert(1, "http://127.0.0.1:50051".to_string());
        let factory = DbNetworkFactory::with_peers(peers);

        assert!(factory.get_peer_address(1).is_some());
        assert!(factory.get_peer_address(99).is_none());
    }
}
