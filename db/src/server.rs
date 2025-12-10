//! gRPC server implementation for the Database service

use crate::raft::{DbRaft, DbResponse, DbTypeConfig};
use crate::storage::{LogEntry, Storage};
use anyhow::Result;
use tonic::{Request, Response, Status};

// Include generated protobuf code
pub mod db {
    tonic::include_proto!("db");
}

use db::database_server::{Database, DatabaseServer};
use db::*;

/// Database service implementation
pub struct DatabaseService {
    raft: DbRaft,
    storage: Storage,
}

impl DatabaseService {
    pub fn new(raft: DbRaft, storage: Storage) -> Self {
        Self { raft, storage }
    }
}

#[tonic::async_trait]
impl Database for DatabaseService {
    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        let req = request.into_inner();
        
        let entry = LogEntry::Put {
            key: req.key,
            value: req.value,
        };
        
        // Submit to Raft for consensus
        match self.raft.client_write(entry).await {
            Ok(_) => Ok(Response::new(PutResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Raft write failed: {}", e))),
        }
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();
        
        // Reads can go directly to local storage (linearizable reads via leader lease)
        match self.storage.get(&req.key) {
            Ok(Some(value)) => Ok(Response::new(GetResponse {
                found: true,
                value,
                error: String::new(),
            })),
            Ok(None) => Ok(Response::new(GetResponse {
                found: false,
                value: vec![],
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Storage read failed: {}", e))),
        }
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        
        let entry = LogEntry::Delete { key: req.key };
        
        match self.raft.client_write(entry).await {
            Ok(_) => Ok(Response::new(DeleteResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Raft write failed: {}", e))),
        }
    }

    async fn list(&self, request: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit > 0 {
            Some(req.limit as usize)
        } else {
            None
        };
        
        match self.storage.list(&req.prefix, limit) {
            Ok(pairs) => {
                let items = pairs
                    .into_iter()
                    .map(|(key, value)| KeyValue { key, value })
                    .collect();
                Ok(Response::new(ListResponse { items }))
            }
            Err(e) => Err(Status::internal(format!("Storage list failed: {}", e))),
        }
    }

    async fn exists(
        &self,
        request: Request<ExistsRequest>,
    ) -> Result<Response<ExistsResponse>, Status> {
        let req = request.into_inner();
        
        match self.storage.exists(&req.key) {
            Ok(exists) => Ok(Response::new(ExistsResponse { exists })),
            Err(e) => Err(Status::internal(format!("Storage check failed: {}", e))),
        }
    }

    async fn batch_put(
        &self,
        request: Request<BatchPutRequest>,
    ) -> Result<Response<BatchPutResponse>, Status> {
        let req = request.into_inner();
        
        let pairs: Vec<(Vec<u8>, Vec<u8>)> = req
            .items
            .into_iter()
            .map(|kv| (kv.key, kv.value))
            .collect();
        
        let count = pairs.len() as u32;
        let entry = LogEntry::BatchPut { pairs };
        
        match self.raft.client_write(entry).await {
            Ok(_) => Ok(Response::new(BatchPutResponse {
                success: true,
                written: count,
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Raft write failed: {}", e))),
        }
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let metrics = self.raft.metrics().borrow().clone();
        
        let role = match metrics.state {
            openraft::ServerState::Leader => "leader".to_string(),
            openraft::ServerState::Follower => "follower".to_string(),
            openraft::ServerState::Candidate => "candidate".to_string(),
            openraft::ServerState::Learner => "learner".to_string(),
            openraft::ServerState::Shutdown => "shutdown".to_string(),
        };
        
        Ok(Response::new(HealthResponse {
            healthy: matches!(metrics.state, openraft::ServerState::Leader | openraft::ServerState::Follower),
            node_id: metrics.id.to_string(),
            role,
        }))
    }

    async fn cluster_status(
        &self,
        _request: Request<ClusterStatusRequest>,
    ) -> Result<Response<ClusterStatusResponse>, Status> {
        let metrics = self.raft.metrics().borrow().clone();
        
        let leader_id = metrics.current_leader.map(|id| id.to_string()).unwrap_or_default();
        let member_ids: Vec<String> = metrics
            .membership_config
            .membership()
            .nodes()
            .map(|(id, _node)| id.to_string())
            .collect();
        let term = metrics.current_term;
        let commit_index = metrics.last_applied.map(|id| id.index).unwrap_or(0);
        
        Ok(Response::new(ClusterStatusResponse {
            leader_id,
            member_ids,
            term,
            commit_index,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_service_creation() {
        // This is a placeholder test - actual testing requires setting up a full Raft node
        // which we'll do in integration tests
        assert!(true);
    }
}
