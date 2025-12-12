//! gRPC service implementation for Raft consensus RPCs
//!
//! This module provides the server-side implementation of the RaftService gRPC service,
//! handling incoming RPC calls from peer nodes in the Raft cluster.

use crate::raft::{DbRaft, DbTypeConfig};
use openraft::{LogId, Vote};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::server::db::{
    raft_service_server::RaftService, AppendEntriesRequest, AppendEntriesResponse,
    InstallSnapshotRequest, InstallSnapshotResponse, ProtoLogId, ProtoVote,
    VoteRequest, VoteResponse,
};

/// Implementation of the Raft service
pub struct RaftServiceImpl {
    /// Reference to the Raft instance
    raft: Arc<DbRaft>,
}

impl RaftServiceImpl {
    pub fn new(raft: Arc<DbRaft>) -> Self {
        Self { raft }
    }
}

#[tonic::async_trait]
impl RaftService for RaftServiceImpl {
    async fn vote(
        &self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        let req = request.into_inner();

        debug!("received vote request: {:?}", req);

        // Convert proto to openraft types
        let proto_vote = req.vote.ok_or_else(|| Status::invalid_argument("missing vote"))?;

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
            .map_err(|e| Status::internal(format!("vote failed: {}", e)))?;

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

        debug!("received append_entries request: entries={}", req.entries.len());

        // Convert proto to openraft types
        let proto_vote = req.vote.ok_or_else(|| Status::invalid_argument("missing vote"))?;

        // Deserialize entries
        let entries: Vec<openraft::Entry<DbTypeConfig>> = req.entries
            .into_iter()
            .map(|entry| serde_json::from_slice(&entry.data))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Status::invalid_argument(format!("invalid entry data: {}", e)))?;

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
        let raft_response = self
            .raft
            .append_entries(append_req)
            .await
            .map_err(|e| Status::internal(format!("append_entries failed: {}", e)))?;

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

        debug!("received install_snapshot request: offset={}, done={}", req.offset, req.done);

        // Convert proto to openraft types
        let proto_vote = req.vote.ok_or_else(|| Status::invalid_argument("missing vote"))?;
        let proto_meta = req.meta.ok_or_else(|| Status::invalid_argument("missing snapshot meta"))?;

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
                            term: proto_meta.last_log_id.as_ref().map(|l| l.term).unwrap_or(0),
                            node_id: proto_vote.node_id,
                        },
                        index: proto_meta.last_membership as u64,
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
            .map_err(|e| Status::internal(format!("install_snapshot failed: {}", e)))?;

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
