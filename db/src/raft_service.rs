//! gRPC service implementation for Raft consensus RPCs
//!
//! This module provides the server-side implementation of the `RaftService` gRPC service,
//! handling incoming RPC calls from peer nodes in the Raft cluster.

use crate::raft::{DbRaft, DbTypeConfig};
use openraft::{LogId, Vote};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::server::db::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    ProtoLogId, ProtoVote, VoteRequest, VoteResponse, raft_service_server::RaftService,
};

/// Implementation of the Raft service
pub struct RaftServiceImpl {
    /// Reference to the Raft instance
    raft: Arc<DbRaft>,
}

impl RaftServiceImpl {
    #[must_use]
    pub fn new(raft: Arc<DbRaft>) -> Self {
        Self { raft }
    }
}

#[tonic::async_trait]
impl RaftService for RaftServiceImpl {
    async fn vote(&self, request: Request<VoteRequest>) -> Result<Response<VoteResponse>, Status> {
        let req = request.into_inner();

        debug!("received vote request: {:?}", req);

        // Convert proto to openraft types
        let proto_vote = req
            .vote
            .ok_or_else(|| Status::invalid_argument("missing vote"))?;

        let vote_req = openraft::raft::VoteRequest {
            vote: Vote {
                leader_id: openraft::LeaderId {
                    term: proto_vote.term,
                    node_id: proto_vote.node_id,
                },
                committed: proto_vote.committed,
            },
            last_log_id: req.last_log_id.map(|log_id| LogId {
                leader_id: openraft::LeaderId {
                    term: log_id.term,
                    node_id: proto_vote.node_id,
                },
                index: log_id.index,
            }),
        };

        // Forward to Raft
        let raft_response = self
            .raft
            .vote(vote_req)
            .await
            .map_err(|e| Status::internal(format!("vote failed: {e}")))?;

        // Convert back to proto
        let proto_response = VoteResponse {
            vote: Some(ProtoVote {
                term: raft_response.vote.leader_id.term,
                node_id: raft_response.vote.leader_id.node_id,
                committed: raft_response.vote.committed,
            }),
            vote_granted: raft_response.vote_granted,
            last_log_id: raft_response.last_log_id.map(|log_id| ProtoLogId {
                term: log_id.leader_id.term,
                index: log_id.index,
            }),
        };

        Ok(Response::new(proto_response))
    }

    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();

        debug!(
            "received append_entries request: entries={}",
            req.entries.len()
        );

        // Convert proto to openraft types
        let proto_vote = req
            .vote
            .ok_or_else(|| Status::invalid_argument("missing vote"))?;

        // Deserialize entries
        let entries: Vec<openraft::Entry<DbTypeConfig>> = req
            .entries
            .into_iter()
            .map(|entry| serde_json::from_slice(&entry.data))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Status::invalid_argument(format!("invalid entry data: {e}")))?;

        let append_req = openraft::raft::AppendEntriesRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: proto_vote.term,
                    node_id: proto_vote.node_id,
                },
                committed: proto_vote.committed,
            },
            prev_log_id: req.prev_log_id.map(|log_id| openraft::LogId {
                leader_id: openraft::LeaderId {
                    term: log_id.term,
                    node_id: proto_vote.node_id,
                },
                index: log_id.index,
            }),
            entries,
            leader_commit: req.leader_commit.map(|log_id| openraft::LogId {
                leader_id: openraft::LeaderId {
                    term: log_id.term,
                    node_id: proto_vote.node_id,
                },
                index: log_id.index,
            }),
        };

        // Forward to Raft
        let _raft_response = self
            .raft
            .append_entries(append_req)
            .await
            .map_err(|e| Status::internal(format!("append_entries failed: {e}")))?;

        // Convert back to proto
        let proto_response = AppendEntriesResponse {
            vote: Some(ProtoVote {
                term: proto_vote.term,
                node_id: proto_vote.node_id,
                committed: proto_vote.committed,
            }),
            response_type: 0, // Success - TODO: handle different response types
            partial_success_index: None,
        };

        Ok(Response::new(proto_response))
    }

    async fn install_snapshot(
        &self,
        request: Request<InstallSnapshotRequest>,
    ) -> Result<Response<InstallSnapshotResponse>, Status> {
        let req = request.into_inner();

        debug!(
            "received install_snapshot request: offset={}, done={}",
            req.offset, req.done
        );

        // Convert proto to openraft types
        let proto_vote = req
            .vote
            .ok_or_else(|| Status::invalid_argument("missing vote"))?;
        let proto_meta = req
            .meta
            .ok_or_else(|| Status::invalid_argument("missing snapshot meta"))?;

        let install_req = openraft::raft::InstallSnapshotRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: proto_vote.term,
                    node_id: proto_vote.node_id,
                },
                committed: proto_vote.committed,
            },
            meta: openraft::SnapshotMeta {
                last_log_id: proto_meta.last_log_id.map(|log_id| openraft::LogId {
                    leader_id: openraft::LeaderId {
                        term: log_id.term,
                        node_id: proto_vote.node_id,
                    },
                    index: log_id.index,
                }),
                last_membership: openraft::StoredMembership::new(
                    Some(openraft::LogId {
                        leader_id: openraft::LeaderId {
                            term: proto_meta.last_log_id.as_ref().map_or(0, |l| l.term),
                            node_id: proto_vote.node_id,
                        },
                        index: u64::from(proto_meta.last_membership),
                    }),
                    openraft::Membership::new(vec![], ()),
                ),
                snapshot_id: proto_meta.snapshot_id,
            },
            offset: req.offset,
            data: req.data,
            done: req.done,
        };

        // Forward to Raft
        let raft_response = self
            .raft
            .install_snapshot(install_req)
            .await
            .map_err(|e| Status::internal(format!("install_snapshot failed: {e}")))?;

        // Convert back to proto
        let proto_response = InstallSnapshotResponse {
            vote: Some(ProtoVote {
                term: raft_response.vote.leader_id.term,
                node_id: raft_response.vote.leader_id.node_id,
                committed: raft_response.vote.committed,
            }),
        };

        Ok(Response::new(proto_response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::DbNetworkFactory;
    use crate::raft::DbStore;
    use crate::storage::LogEntry;
    use openraft::storage::Adaptor;
    use tonic::Request;

    async fn test_service() -> RaftServiceImpl {
        let store = DbStore::new_temp().expect("temp store");
        let cfg = Arc::new(
            openraft::Config::default()
                .validate()
                .expect("raft config"),
        );
        let network_factory = DbNetworkFactory::new();
        let (log_store, state_machine) = Adaptor::new(store);
        let raft = Arc::new(
            DbRaft::new(1, cfg, network_factory, log_store, state_machine)
                .await
                .expect("raft node"),
        );

        let mut members = std::collections::BTreeSet::new();
        members.insert(1);
        let _ = raft.initialize(members).await;

        RaftServiceImpl::new(raft)
    }

    #[tokio::test]
    async fn vote_rejects_missing_vote() {
        let service = test_service().await;
        let err = service
            .vote(Request::new(VoteRequest {
                vote: None,
                last_log_id: None,
            }))
            .await
            .expect_err("missing vote must fail");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn vote_accepts_well_formed_request() {
        let service = test_service().await;
        let resp = service
            .vote(Request::new(VoteRequest {
                vote: Some(ProtoVote {
                    term: 1,
                    node_id: 1,
                    committed: false,
                }),
                last_log_id: None,
            }))
            .await
            .expect("vote request");

        let body = resp.into_inner();
        assert!(body.vote.is_some());
    }

    #[tokio::test]
    async fn append_entries_rejects_missing_vote() {
        let service = test_service().await;
        let err = service
            .append_entries(Request::new(AppendEntriesRequest {
                vote: None,
                prev_log_id: None,
                entries: vec![],
                leader_commit: None,
            }))
            .await
            .expect_err("missing vote must fail");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn append_entries_rejects_invalid_entry_data() {
        let service = test_service().await;
        let err = service
            .append_entries(Request::new(AppendEntriesRequest {
                vote: Some(ProtoVote {
                    term: 1,
                    node_id: 1,
                    committed: false,
                }),
                prev_log_id: None,
                entries: vec![crate::server::db::Entry {
                    data: b"not-json".to_vec(),
                }],
                leader_commit: None,
            }))
            .await
            .expect_err("invalid entry payload must fail");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn append_entries_handles_well_formed_entry() {
        let service = test_service().await;
        let entry = openraft::Entry::<DbTypeConfig> {
            log_id: openraft::LogId {
                leader_id: openraft::LeaderId { term: 1, node_id: 1 },
                index: 1,
            },
            payload: openraft::EntryPayload::Normal(LogEntry::Put {
                collection: "c".to_string(),
                key: b"k".to_vec(),
                value: b"v".to_vec(),
            }),
        };

        let entry_bytes = serde_json::to_vec(&entry).expect("serialize entry");

        let response = service
            .append_entries(Request::new(AppendEntriesRequest {
                vote: Some(ProtoVote {
                    term: 1,
                    node_id: 1,
                    committed: false,
                }),
                prev_log_id: None,
                entries: vec![crate::server::db::Entry { data: entry_bytes }],
                leader_commit: None,
            }))
            .await
            .expect("append entries should succeed");

        let body = response.into_inner();
        assert_eq!(body.response_type, 0);
        assert!(body.vote.is_some());
    }

    #[tokio::test]
    async fn install_snapshot_rejects_missing_vote_or_meta() {
        let service = test_service().await;

        let err_vote = service
            .install_snapshot(Request::new(InstallSnapshotRequest {
                vote: None,
                meta: Some(crate::server::db::SnapshotMeta {
                    last_log_id: None,
                    last_applied: 0,
                    last_membership: 0,
                    snapshot_id: "snap".to_string(),
                }),
                offset: 0,
                data: vec![],
                done: true,
            }))
            .await
            .expect_err("missing vote must fail");
        assert_eq!(err_vote.code(), tonic::Code::InvalidArgument);

        let err_meta = service
            .install_snapshot(Request::new(InstallSnapshotRequest {
                vote: Some(ProtoVote {
                    term: 1,
                    node_id: 1,
                    committed: false,
                }),
                meta: None,
                offset: 0,
                data: vec![],
                done: true,
            }))
            .await
            .expect_err("missing meta must fail");
        assert_eq!(err_meta.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn install_snapshot_accepts_well_formed_request() {
        let service = test_service().await;

        let response = service
            .install_snapshot(Request::new(InstallSnapshotRequest {
                vote: Some(ProtoVote {
                    term: 1,
                    node_id: 1,
                    committed: false,
                }),
                meta: Some(crate::server::db::SnapshotMeta {
                    last_log_id: None,
                    last_applied: 0,
                    last_membership: 1,
                    snapshot_id: "snap-1".to_string(),
                }),
                offset: 0,
                data: b"snapshot-chunk".to_vec(),
                done: true,
            }))
            .await
            .expect("install snapshot should succeed");

        let body = response.into_inner();
        assert!(body.vote.is_some());
    }
}
