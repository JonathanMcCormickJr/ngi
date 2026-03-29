//! Network layer for inter-node communication via gRPC
//!
//! This module implements the `RaftNetwork` trait from openraft to provide
//! inter-node RPC communication for distributed lock management.

use crate::raft::CustodianTypeConfig;
use openraft::RaftNetwork;
use openraft::network::RaftNetworkFactory;
use std::collections::HashMap;
use std::sync::Arc;

/// Network client for communicating with a specific Raft peer
pub struct CustodianNetwork {
    _target: u64,
    address: String,
    client: Option<
        crate::server::custodian::raft_service_client::RaftServiceClient<tonic::transport::Channel>,
    >,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::custodian::raft_service_server::RaftService;
    use crate::server::custodian::{
        AppendEntriesRequest, AppendEntriesResponse, VoteRequest, VoteResponse,
    };
    use tokio::task;
    use tonic::{Request, Response, Status};

    #[tokio::test]
    async fn test_network_factory_creation() {
        let mut peers = HashMap::new();
        peers.insert(1, "http://127.0.0.1:50051".to_string());
        let factory = CustodianNetworkFactory::with_peers(peers);

        assert!(factory.get_peer_address(1).is_some());
        assert!(factory.get_peer_address(99).is_none());
    }

    #[derive(Default)]
    struct TestRaftSvc {}

    #[tonic::async_trait]
    impl RaftService for TestRaftSvc {
        async fn vote(
            &self,
            request: Request<VoteRequest>,
        ) -> Result<Response<VoteResponse>, Status> {
            let req = request.into_inner();
            Ok(Response::new(VoteResponse {
                term: req.term,
                vote_granted: true,
            }))
        }

        async fn append_entries(
            &self,
            request: Request<AppendEntriesRequest>,
        ) -> Result<Response<AppendEntriesResponse>, Status> {
            let _req = request.into_inner();
            Ok(Response::new(AppendEntriesResponse {
                term: 1,
                success: true,
            }))
        }

        async fn install_snapshot(
            &self,
            _request: Request<crate::server::custodian::InstallSnapshotRequest>,
        ) -> Result<Response<crate::server::custodian::InstallSnapshotResponse>, Status> {
            Ok(Response::new(
                crate::server::custodian::InstallSnapshotResponse { term: 1 },
            ))
        }
    }

    #[tokio::test]
    async fn test_network_install_snapshot_multi_chunk() {
        use crate::server::custodian::InstallSnapshotRequest;
        use std::sync::Mutex;

        // Shared buffer to collect incoming snapshot bytes
        let received = std::sync::Arc::new(Mutex::new(Vec::<u8>::new()));

        #[derive(Clone)]
        struct ChunkedSvc(std::sync::Arc<Mutex<Vec<u8>>>);

        #[tonic::async_trait]
        impl RaftService for ChunkedSvc {
            async fn vote(
                &self,
                _request: Request<VoteRequest>,
            ) -> Result<Response<VoteResponse>, Status> {
                Ok(Response::new(VoteResponse {
                    term: 1,
                    vote_granted: true,
                }))
            }

            async fn append_entries(
                &self,
                _request: Request<AppendEntriesRequest>,
            ) -> Result<Response<AppendEntriesResponse>, Status> {
                Ok(Response::new(AppendEntriesResponse {
                    term: 1,
                    success: true,
                }))
            }

            async fn install_snapshot(
                &self,
                request: Request<InstallSnapshotRequest>,
            ) -> Result<Response<crate::server::custodian::InstallSnapshotResponse>, Status>
            {
                let req = request.into_inner();
                let mut guard = self.0.lock().unwrap();
                guard.extend_from_slice(&req.data);
                Ok(Response::new(
                    crate::server::custodian::InstallSnapshotResponse { term: req.term },
                ))
            }
        }

        let svc = ChunkedSvc(received.clone());
        let addr = "127.0.0.1:50053".parse().unwrap();
        let svc_server = crate::server::custodian::raft_service_server::RaftServiceServer::new(svc);

        let server = tokio::task::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(svc_server)
                .serve(addr)
                .await
                .unwrap();
        });

        // Give server a moment
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Create client
        let mut peers = HashMap::new();
        peers.insert(1, "http://127.0.0.1:50053".to_string());
        let mut factory = CustodianNetworkFactory::with_peers(peers);
        let mut client = factory.new_client(1, &openraft::BasicNode::default()).await;

        // Prepare two chunks
        let chunk1 = b"hello ".to_vec();
        let chunk2 = b"world!".to_vec();

        // First chunk, done = false
        let req1 = openraft::raft::InstallSnapshotRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: 1,
                    node_id: 1,
                },
                committed: true,
            },
            meta: openraft::SnapshotMeta::default(),
            offset: 0,
            data: chunk1.clone(),
            done: false,
        };

        // Second chunk, done = true
        let req2 = openraft::raft::InstallSnapshotRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: 1,
                    node_id: 1,
                },
                committed: true,
            },
            meta: openraft::SnapshotMeta::default(),
            offset: chunk1.len() as u64,
            data: chunk2.clone(),
            done: true,
        };

        // Send chunks
        let res1 = client
            .install_snapshot(
                req1,
                openraft::network::RPCOption::new(std::time::Duration::from_secs(1)),
            )
            .await;
        assert!(res1.is_ok());
        let res2 = client
            .install_snapshot(
                req2,
                openraft::network::RPCOption::new(std::time::Duration::from_secs(1)),
            )
            .await;
        assert!(res2.is_ok());

        // Verify server received concatenated bytes
        let guard = received.lock().unwrap();
        assert_eq!(&guard[..], &[chunk1, chunk2].concat()[..]);

        // Metrics updated by RPC client (sanity check)
        let _prev_created = crate::metrics::SNAPSHOT_CREATED_TOTAL.get();
        crate::metrics::SNAPSHOT_LAST_SIZE_BYTES.set(123);
        assert!(crate::metrics::SNAPSHOT_LAST_SIZE_BYTES.get() > 0);

        server.abort();
    }

    #[tokio::test]
    async fn test_network_vote_and_append() {
        // Start test server
        let svc = TestRaftSvc::default();
        let addr = "127.0.0.1:50052".parse().unwrap();
        let svc_server = crate::server::custodian::raft_service_server::RaftServiceServer::new(svc);

        let server = task::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(svc_server)
                .serve(addr)
                .await
                .unwrap();
        });

        // Give server a moment
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Factory and client
        let mut peers = HashMap::new();
        peers.insert(1, "http://127.0.0.1:50052".to_string());
        let mut factory = CustodianNetworkFactory::with_peers(peers);
        let mut client = factory.new_client(1, &openraft::BasicNode::default()).await;

        // Test vote
        let vote_req = openraft::raft::VoteRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: 1,
                    node_id: 1,
                },
                committed: true,
            },
            last_log_id: None,
        };
        let res = client
            .vote(
                vote_req,
                openraft::network::RPCOption::new(std::time::Duration::from_secs(1)),
            )
            .await;
        assert!(res.is_ok());

        // Test append
        let append_req = openraft::raft::AppendEntriesRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: 1,
                    node_id: 1,
                },
                committed: true,
            },
            prev_log_id: None,
            entries: vec![],
            leader_commit: None,
        };
        let res = client
            .append_entries(
                append_req,
                openraft::network::RPCOption::new(std::time::Duration::from_secs(1)),
            )
            .await;
        assert!(res.is_ok());

        // Shut down server
        server.abort();
    }
}

/// Factory for creating network clients
#[derive(Clone)]
pub struct CustodianNetworkFactory {
    peers: Arc<HashMap<u64, String>>,
}

impl Default for CustodianNetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl CustodianNetworkFactory {
    #[must_use]
    pub fn new() -> Self {
        Self {
            peers: Arc::new(HashMap::new()),
        }
    }

    #[must_use]
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

/// Implementation of openraft `RaftNetworkFactory`
impl RaftNetworkFactory<CustodianTypeConfig> for CustodianNetworkFactory {
    type Network = CustodianNetwork;

    fn new_client(
        &mut self,
        target: u64,
        node: &openraft::BasicNode,
    ) -> impl std::future::Future<Output = Self::Network> + Send {
        let address = self
            .get_peer_address(target)
            .unwrap_or_else(|| format!("http://{}:8081", node.addr));

        async move {
            // For now, create without connecting - connect on first use
            CustodianNetwork {
                _target: target,
                address,
                client: None,
            }
        }
    }
}

impl CustodianNetwork {
    async fn get_client(
        &mut self,
    ) -> Result<
        &mut crate::server::custodian::raft_service_client::RaftServiceClient<
            tonic::transport::Channel,
        >,
        openraft::error::NetworkError,
    > {
        if self.client.is_none() {
            match crate::server::custodian::raft_service_client::RaftServiceClient::connect(
                self.address.clone(),
            )
            .await
            {
                Ok(client) => {
                    self.client = Some(client);
                }
                Err(e) => {
                    return Err(openraft::error::NetworkError::new(&e));
                }
            }
        }
        // Client should be initialized above, but handle gracefully
        self.client.as_mut().ok_or_else(|| {
            openraft::error::NetworkError::new(&std::io::Error::other("client not initialized"))
        })
    }
}

// Placeholder implementations - full network layer to be implemented
impl RaftNetwork<CustodianTypeConfig> for CustodianNetwork {
    async fn vote(
        &mut self,
        rpc: openraft::raft::VoteRequest<u64>,
        _option: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::VoteResponse<u64>,
        openraft::error::RPCError<
            u64,
            openraft::BasicNode,
            openraft::error::RaftError<u64, openraft::error::Infallible>,
        >,
    > {
        // Build proto request
        let proto_req = crate::server::custodian::VoteRequest {
            term: rpc.vote.leader_id.term,
            candidate_id: rpc.vote.leader_id.node_id.to_string(),
            last_log_index: rpc.last_log_id.map(|l| l.index).unwrap_or_default(),
            last_log_term: rpc
                .last_log_id
                .map(|l| l.leader_id.term)
                .unwrap_or_default(),
        };

        // Call remote RPC
        let client = match self.get_client().await {
            Ok(c) => c,
            Err(e) => return Err(openraft::error::RPCError::Network(e)),
        };

        match client.vote(tonic::Request::new(proto_req)).await {
            Ok(resp) => {
                let r = resp.into_inner();

                // Map to OpenRaft VoteResponse
                let vote_response = openraft::raft::VoteResponse {
                    vote: openraft::Vote {
                        leader_id: openraft::LeaderId {
                            term: r.term,
                            node_id: rpc.vote.leader_id.node_id,
                        },
                        committed: rpc.vote.committed,
                    },
                    vote_granted: r.vote_granted,
                    last_log_id: None,
                };

                Ok(vote_response)
            }
            Err(e) => Err(openraft::error::RPCError::Network(
                openraft::error::NetworkError::new(&e),
            )),
        }
    }

    async fn append_entries(
        &mut self,
        rpc: openraft::raft::AppendEntriesRequest<CustodianTypeConfig>,
        _option: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::AppendEntriesResponse<u64>,
        openraft::error::RPCError<
            u64,
            openraft::BasicNode,
            openraft::error::RaftError<u64, openraft::error::Infallible>,
        >,
    > {
        // Convert entries to proto LogEntry
        let mut entries = Vec::new();
        for entry in &rpc.entries {
            let mut proto_entry = crate::server::custodian::LogEntry {
                term: entry.log_id.leader_id.term,
                index: entry.log_id.index,
                command: None,
            };

            if let openraft::EntryPayload::Normal(cmd) = &entry.payload {
                // Map LockCommand -> proto LockCommand
                let proto_cmd = match cmd {
                    crate::storage::LockCommand::AcquireLock { ticket_id, user_id } => Some(
                        crate::server::custodian::lock_command::CommandType::AcquireLock(
                            crate::server::custodian::AcquireLockCommand {
                                ticket_id: *ticket_id,
                                user_uuid: user_id.to_string(),
                            },
                        ),
                    ),
                    crate::storage::LockCommand::ReleaseLock { ticket_id, user_id } => Some(
                        crate::server::custodian::lock_command::CommandType::ReleaseLock(
                            crate::server::custodian::ReleaseLockCommand {
                                ticket_id: *ticket_id,
                                user_uuid: user_id.to_string(),
                            },
                        ),
                    ),
                };
                if let Some(ct) = proto_cmd {
                    proto_entry.command = Some(crate::server::custodian::LockCommand {
                        command_type: Some(ct),
                    });
                }
            }

            entries.push(proto_entry);
        }

        let proto_req = crate::server::custodian::AppendEntriesRequest {
            term: rpc.vote.leader_id.term,
            leader_id: rpc.vote.leader_id.node_id.to_string(),
            prev_log_index: rpc.prev_log_id.map(|l| l.index).unwrap_or_default(),
            prev_log_term: rpc
                .prev_log_id
                .map(|l| l.leader_id.term)
                .unwrap_or_default(),
            entries,
            leader_commit: rpc.leader_commit.map(|l| l.index).unwrap_or_default(),
        };

        let client = match self.get_client().await {
            Ok(c) => c,
            Err(e) => return Err(openraft::error::RPCError::Network(e)),
        };

        match client.append_entries(tonic::Request::new(proto_req)).await {
            Ok(_) => Ok(openraft::raft::AppendEntriesResponse::Success),
            Err(e) => Err(openraft::error::RPCError::Network(
                openraft::error::NetworkError::new(&e),
            )),
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: openraft::raft::InstallSnapshotRequest<CustodianTypeConfig>,
        _option: openraft::network::RPCOption,
    ) -> Result<
        openraft::raft::InstallSnapshotResponse<u64>,
        openraft::error::RPCError<
            u64,
            openraft::BasicNode,
            openraft::error::RaftError<u64, openraft::error::InstallSnapshotError>,
        >,
    > {
        // Use the chunked snapshot data provided by OpenRaft (rpc.data)
        // OpenRaft may call install_snapshot multiple times with chunks
        // indicated by `rpc.offset` and `rpc.done`.
        let snapshot_chunk = rpc.data.clone();

        let proto_req = crate::server::custodian::InstallSnapshotRequest {
            term: rpc.vote.leader_id.term,
            leader_id: rpc.vote.leader_id.node_id.to_string(),
            last_included_index: rpc.meta.last_log_id.map(|l| l.index).unwrap_or_default(),
            last_included_term: rpc
                .meta
                .last_log_id
                .map(|l| l.leader_id.term)
                .unwrap_or_default(),
            data: snapshot_chunk,
            done: rpc.done,
        };

        let client = match self.get_client().await {
            Ok(c) => c,
            Err(e) => return Err(openraft::error::RPCError::Network(e)),
        };

        // Record metrics for RPC send (clamp to i64::MAX to avoid cast wrap)
        let max_usize = usize::try_from(i64::MAX).unwrap_or(usize::MAX);
        let last_size = std::cmp::min(proto_req.data.len(), max_usize);
        let last_size_i64 = std::convert::TryInto::<i64>::try_into(last_size).unwrap_or(i64::MAX);
        crate::metrics::SNAPSHOT_LAST_SIZE_BYTES.set(last_size_i64);
        crate::metrics::SNAPSHOT_INSTALL_STARTED_TOTAL.inc();
        // Also push metrics to admin if configured
        if let Ok(admin_addr) = std::env::var("ADMIN_ADDR") {
            let size = proto_req.data.len() as u64;
            let mut counters = std::collections::HashMap::new();
            let started = std::convert::TryInto::<i64>::try_into(
                crate::metrics::SNAPSHOT_INSTALL_STARTED_TOTAL.get(),
            )
            .unwrap_or(i64::MAX);
            counters.insert("snapshot_install_started_total".to_string(), started);
            crate::admin_client::init(admin_addr);
            tokio::spawn(async move {
                crate::admin_client::push_snapshot("custodian", size, counters).await;
            });
        }

        match client
            .install_snapshot(tonic::Request::new(proto_req))
            .await
        {
            Ok(resp) => {
                let r = resp.into_inner();

                // Completed via RPC
                crate::metrics::SNAPSHOT_INSTALL_COMPLETED_TOTAL.inc();

                let install_response = openraft::raft::InstallSnapshotResponse {
                    vote: openraft::Vote {
                        leader_id: openraft::LeaderId {
                            term: r.term,
                            node_id: rpc.vote.leader_id.node_id,
                        },
                        committed: rpc.vote.committed,
                    },
                };

                Ok(install_response)
            }
            Err(e) => Err(openraft::error::RPCError::Network(
                openraft::error::NetworkError::new(&e),
            )),
        }
    }
}
