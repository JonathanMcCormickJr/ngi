//! gRPC service implementation for Raft consensus RPCs
//!
//! This module provides the server-side implementation of the RaftService gRPC service,
//! handling incoming RPC calls from peer nodes in the Raft cluster.

use crate::raft::DbRaft;
use openraft::{LogId, Vote};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::debug;

// Include generated proto code
tonic::include_proto!("db");

use raft_service_server::RaftService;

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
        _request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        // TODO: Implement full Raft append_entries RPC
        Err(Status::unimplemented("append_entries not yet implemented"))
    }

    async fn install_snapshot(
        &self,
        _request: Request<InstallSnapshotRequest>,
    ) -> Result<Response<InstallSnapshotResponse>, Status> {
        // TODO: Implement full Raft snapshot installation
        Err(Status::unimplemented("install_snapshot not yet implemented"))
    }
}
