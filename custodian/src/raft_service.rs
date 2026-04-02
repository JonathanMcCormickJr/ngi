//! gRPC Raft service server for custodian
//!
//! This mirrors the DB service `RaftService` implementation, forwarding
//! incoming RPCs to the local `CustodianRaft` instance.

use crate::raft::{CustodianRaft, CustodianTypeConfig};
use openraft::{LogId, Vote};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::server::custodian::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse, raft_service_server::RaftService,
};

/// Raft service implementation
pub struct RaftServiceImpl {
    raft: Arc<CustodianRaft>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use openraft::Config;
    use openraft::storage::Adaptor;
    use std::sync::Arc;

    async fn create_raft_service() -> RaftServiceImpl {
        let store = crate::raft::CustodianStore::new_temp().unwrap();
        let cfg = Arc::new(Config::default());
        let network_factory = crate::network::CustodianNetworkFactory::new();
        let (log_store, state_machine) = Adaptor::new(store.clone());
        let raft = CustodianRaft::new(1u64, cfg, network_factory, log_store, state_machine)
            .await
            .expect("create raft");
        RaftServiceImpl::new(Arc::new(raft))
    }

    #[tokio::test]
    async fn test_raft_service_handlers() {
        let svc = create_raft_service().await;

        // Vote request
        let vote_req = crate::server::custodian::VoteRequest {
            term: 1,
            candidate_id: "1".to_string(),
            last_log_index: 0,
            last_log_term: 0,
        };

        let resp = svc
            .vote(tonic::Request::new(vote_req))
            .await
            .expect("vote rpc");
        assert_eq!(resp.get_ref().term, 1);

        // Append entries (empty)
        let append_req = crate::server::custodian::AppendEntriesRequest {
            term: 1,
            leader_id: "1".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let resp = svc
            .append_entries(tonic::Request::new(append_req))
            .await
            .expect("append rpc");
        assert!(resp.get_ref().success);

        // Install snapshot (small payload)
        let install_req = crate::server::custodian::InstallSnapshotRequest {
            term: 1,
            leader_id: "1".to_string(),
            last_included_index: 0,
            last_included_term: 0,
            data: vec![1, 2, 3],
            done: true,
        };

        let resp = svc
            .install_snapshot(tonic::Request::new(install_req))
            .await
            .expect("install snapshot rpc");
        assert_eq!(resp.get_ref().term, 1);
    }

    #[tokio::test]
    async fn test_append_entries_with_acquire_lock_command() {
        let svc = create_raft_service().await;

        let acquire_cmd = crate::server::custodian::LockCommand {
            command_type: Some(
                crate::server::custodian::lock_command::CommandType::AcquireLock(
                    crate::server::custodian::AcquireLockCommand {
                        ticket_id: 42,
                        user_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    },
                ),
            ),
        };

        let entry = crate::server::custodian::LogEntry {
            term: 1,
            index: 1,
            command: Some(acquire_cmd),
        };

        let append_req = crate::server::custodian::AppendEntriesRequest {
            term: 1,
            leader_id: "1".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![entry],
            leader_commit: 0,
        };

        // Exercise the code path; may succeed or return error depending on Raft state
        let _ = svc.append_entries(tonic::Request::new(append_req)).await;
    }

    #[tokio::test]
    async fn test_append_entries_with_release_lock_command() {
        let svc = create_raft_service().await;

        let release_cmd = crate::server::custodian::LockCommand {
            command_type: Some(
                crate::server::custodian::lock_command::CommandType::ReleaseLock(
                    crate::server::custodian::ReleaseLockCommand {
                        ticket_id: 42,
                        user_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    },
                ),
            ),
        };

        let entry = crate::server::custodian::LogEntry {
            term: 1,
            index: 1,
            command: Some(release_cmd),
        };

        let append_req = crate::server::custodian::AppendEntriesRequest {
            term: 1,
            leader_id: "1".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![entry],
            leader_commit: 0,
        };

        let _ = svc.append_entries(tonic::Request::new(append_req)).await;
    }

    #[tokio::test]
    async fn test_append_entries_with_blank_command() {
        let svc = create_raft_service().await;

        // Entry with no command (None) should be treated as Blank
        let entry = crate::server::custodian::LogEntry {
            term: 1,
            index: 1,
            command: None,
        };

        let append_req = crate::server::custodian::AppendEntriesRequest {
            term: 1,
            leader_id: "1".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![entry],
            leader_commit: 0,
        };

        let _ = svc.append_entries(tonic::Request::new(append_req)).await;
    }

    #[tokio::test]
    async fn test_vote_with_nonzero_last_log_index() {
        let svc = create_raft_service().await;

        // Vote request with a nonzero last_log_index to exercise the Some branch
        let vote_req = crate::server::custodian::VoteRequest {
            term: 2,
            candidate_id: "1".to_string(),
            last_log_index: 5,
            last_log_term: 1,
        };

        let resp = svc.vote(tonic::Request::new(vote_req)).await;
        // May succeed or fail depending on Raft state, but should not panic
        let _ = resp;
    }

    #[tokio::test]
    async fn test_append_entries_with_nonzero_prev_and_commit() {
        let svc = create_raft_service().await;

        // Test with nonzero prev_log_index and leader_commit to exercise those branches
        let append_req = crate::server::custodian::AppendEntriesRequest {
            term: 1,
            leader_id: "1".to_string(),
            prev_log_index: 3,
            prev_log_term: 1,
            entries: vec![],
            leader_commit: 5,
        };

        let resp = svc.append_entries(tonic::Request::new(append_req)).await;
        // May fail (Raft state mismatch) but exercises the code paths
        let _ = resp;
    }
}

impl RaftServiceImpl {
    #[must_use]
    pub fn new(raft: Arc<CustodianRaft>) -> Self {
        Self { raft }
    }
}

#[tonic::async_trait]
impl RaftService for RaftServiceImpl {
    async fn vote(&self, request: Request<VoteRequest>) -> Result<Response<VoteResponse>, Status> {
        let req = request.into_inner();

        debug!("received vote request: {:?}", req);

        // Map proto VoteRequest -> openraft VoteRequest
        let vote_req = openraft::raft::VoteRequest {
            vote: Vote {
                leader_id: openraft::LeaderId {
                    term: req.term,
                    node_id: req.candidate_id.parse().unwrap_or_default(),
                },
                committed: true,
            },
            last_log_id: if req.last_log_index != 0 {
                Some(LogId {
                    leader_id: openraft::LeaderId {
                        term: req.last_log_term,
                        node_id: req.candidate_id.parse().unwrap_or_default(),
                    },
                    index: req.last_log_index,
                })
            } else {
                None
            },
        };

        let raft_response = self
            .raft
            .vote(vote_req)
            .await
            .map_err(|e| Status::internal(format!("vote failed: {e}")))?;

        let proto_response = VoteResponse {
            term: raft_response.vote.leader_id.term,
            vote_granted: raft_response.vote_granted,
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

        let mut entries: Vec<openraft::Entry<CustodianTypeConfig>> = Vec::new();
        for e in req.entries {
            let leader_node = req.leader_id.parse().unwrap_or_default();
            let log_id = LogId {
                leader_id: openraft::LeaderId {
                    term: e.term,
                    node_id: leader_node,
                },
                index: e.index,
            };

            let payload = match e.command.and_then(|c| c.command_type) {
                Some(crate::server::custodian::lock_command::CommandType::AcquireLock(acquire)) => {
                    openraft::EntryPayload::Normal(crate::storage::LockCommand::AcquireLock {
                        ticket_id: acquire.ticket_id,
                        user_id: uuid::Uuid::parse_str(&acquire.user_uuid).unwrap_or_default(),
                    })
                }
                Some(crate::server::custodian::lock_command::CommandType::ReleaseLock(release)) => {
                    openraft::EntryPayload::Normal(crate::storage::LockCommand::ReleaseLock {
                        ticket_id: release.ticket_id,
                        user_id: uuid::Uuid::parse_str(&release.user_uuid).unwrap_or_default(),
                    })
                }
                None => openraft::EntryPayload::Blank,
            };

            entries.push(openraft::Entry { log_id, payload });
        }

        let append_req = openraft::raft::AppendEntriesRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: req.term,
                    node_id: req.leader_id.parse().unwrap_or_default(),
                },
                committed: true,
            },
            prev_log_id: if req.prev_log_index != 0 {
                Some(LogId {
                    leader_id: openraft::LeaderId {
                        term: req.prev_log_term,
                        node_id: req.leader_id.parse().unwrap_or_default(),
                    },
                    index: req.prev_log_index,
                })
            } else {
                None
            },
            entries,
            leader_commit: if req.leader_commit != 0 {
                Some(LogId {
                    leader_id: openraft::LeaderId {
                        term: 0,
                        node_id: req.leader_id.parse().unwrap_or_default(),
                    },
                    index: req.leader_commit,
                })
            } else {
                None
            },
        };

        let _raft_response = self
            .raft
            .append_entries(append_req)
            .await
            .map_err(|e| Status::internal(format!("append_entries failed: {e}")))?;

        Ok(Response::new(AppendEntriesResponse {
            term: req.term,
            success: true,
        }))
    }

    async fn install_snapshot(
        &self,
        request: Request<InstallSnapshotRequest>,
    ) -> Result<Response<InstallSnapshotResponse>, Status> {
        let req = request.into_inner();

        debug!(
            "received install_snapshot request: last_index={}, last_term={}, done={}",
            req.last_included_index, req.last_included_term, req.done
        );

        // Build SnapshotMeta
        let meta = openraft::SnapshotMeta {
            last_log_id: req.last_included_index.checked_sub(0).map(|index| LogId {
                leader_id: openraft::LeaderId {
                    term: req.last_included_term,
                    node_id: 0,
                },
                index,
            }),
            last_membership: openraft::StoredMembership::default(),
            snapshot_id: String::new(),
        };

        let install_req = openraft::raft::InstallSnapshotRequest {
            vote: openraft::Vote {
                leader_id: openraft::LeaderId {
                    term: req.term,
                    node_id: 0,
                },
                committed: true,
            },
            meta,
            offset: 0,
            data: req.data,
            done: req.done,
        };

        let raft_response = self
            .raft
            .install_snapshot(install_req)
            .await
            .map_err(|e| Status::internal(format!("install_snapshot failed: {e}")))?;

        Ok(Response::new(InstallSnapshotResponse {
            term: raft_response.vote.leader_id.term,
        }))
    }
}
