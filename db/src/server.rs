//! gRPC server implementation for the Database service
//!
//! This module implements the gRPC endpoint handlers for the Database service,
//! routing requests appropriately through either Raft consensus (writes) or
//! direct storage access (reads).

use crate::raft::DbRaft;
use crate::storage::{LogEntry, Storage};
use tonic::{Request, Response, Status};

// Include generated protobuf code
pub mod db {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("db");
}

use db::database_server::Database;
use db::{
    BatchPutRequest, BatchPutResponse, ClusterStatusRequest, ClusterStatusResponse, DeleteRequest,
    DeleteResponse, ExistsRequest, ExistsResponse, GetRequest, GetResponse, HealthRequest,
    HealthResponse, KeyValue, ListRequest, ListResponse, PutRequest, PutResponse,
};

/// Database service implementation
///
/// Implements the gRPC Database service with the following behavior:
/// - Write operations (`Put`, `Delete`, `BatchPut`) are submitted to Raft for consensus
/// - Read operations (Get, List, Exists) are read directly from local storage
/// - Meta operations (`Health`, `ClusterStatus`) report Raft cluster state
pub struct DatabaseService {
    raft: DbRaft,
    storage: Storage,
}

impl DatabaseService {
    #[must_use]
    pub fn new(raft: DbRaft, storage: Storage) -> Self {
        Self { raft, storage }
    }
}

#[tonic::async_trait]
impl Database for DatabaseService {
    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        let req = request.into_inner();

        let entry = LogEntry::Put {
            collection: req.collection,
            key: req.key,
            value: req.value,
        };

        // Submit to Raft for consensus
        match self.raft.client_write(entry).await {
            Ok(_) => Ok(Response::new(PutResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Raft write failed: {e}"))),
        }
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();

        // Reads can go directly to local storage (linearizable reads via leader lease)
        match self.storage.get(&req.collection, &req.key) {
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
            Err(e) => Err(Status::internal(format!("Storage read failed: {e}"))),
        }
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();

        let entry = LogEntry::Delete {
            collection: req.collection,
            key: req.key,
        };

        match self.raft.client_write(entry).await {
            Ok(_) => Ok(Response::new(DeleteResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Raft write failed: {e}"))),
        }
    }

    async fn list(&self, request: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit > 0 {
            Some(req.limit as usize)
        } else {
            None
        };

        match self.storage.list(&req.collection, &req.prefix, limit) {
            Ok(pairs) => {
                let items = pairs
                    .into_iter()
                    .map(|(key, value)| KeyValue { key, value })
                    .collect();
                Ok(Response::new(ListResponse { items }))
            }
            Err(e) => Err(Status::internal(format!("Storage list failed: {e}"))),
        }
    }

    async fn exists(
        &self,
        request: Request<ExistsRequest>,
    ) -> Result<Response<ExistsResponse>, Status> {
        let req = request.into_inner();

        match self.storage.exists(&req.collection, &req.key) {
            Ok(exists) => Ok(Response::new(ExistsResponse { exists })),
            Err(e) => Err(Status::internal(format!("Storage check failed: {e}"))),
        }
    }

    async fn batch_put(
        &self,
        request: Request<BatchPutRequest>,
    ) -> Result<Response<BatchPutResponse>, Status> {
        let req = request.into_inner();

        let pairs: Vec<(Vec<u8>, Vec<u8>)> =
            req.items.into_iter().map(|kv| (kv.key, kv.value)).collect();

        let count = u32::try_from(pairs.len()).unwrap_or(u32::MAX);
        let entry = LogEntry::BatchPut {
            collection: req.collection,
            pairs,
        };

        match self.raft.client_write(entry).await {
            Ok(_) => Ok(Response::new(BatchPutResponse {
                success: true,
                written: count,
                error: String::new(),
            })),
            Err(e) => Err(Status::internal(format!("Raft write failed: {e}"))),
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
            healthy: matches!(
                metrics.state,
                openraft::ServerState::Leader | openraft::ServerState::Follower
            ),
            node_id: metrics.id.to_string(),
            role,
        }))
    }

    async fn cluster_status(
        &self,
        _request: Request<ClusterStatusRequest>,
    ) -> Result<Response<ClusterStatusResponse>, Status> {
        let metrics = self.raft.metrics().borrow().clone();

        let leader_id = metrics
            .current_leader
            .map(|id| id.to_string())
            .unwrap_or_default();
        let member_ids: Vec<String> = metrics
            .membership_config
            .membership()
            .nodes()
            .map(|(id, _node)| id.to_string())
            .collect();
        let term = metrics.current_term;
        let commit_index = metrics.last_applied.map_or(0, |id| id.index);

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
    use crate::network::DbNetworkFactory;
    use crate::raft::DbStore;
    use openraft::Config;
    use openraft::storage::Adaptor;
    use std::sync::Arc;

    async fn create_service() -> DatabaseService {
        let store = DbStore::new_temp().expect("temp store");
        let cfg = Arc::new(Config::default().validate().expect("raft config"));
        let network_factory = DbNetworkFactory::new();
        let (log_store, state_machine) = Adaptor::new(store.clone());

        let raft = crate::raft::DbRaft::new(1, cfg, network_factory, log_store, state_machine)
            .await
            .expect("raft node");

        let mut members = std::collections::BTreeSet::new();
        members.insert(1);
        let _ = raft.initialize(members).await;

        let storage = store.state_machine().read().await.storage.clone();
        DatabaseService::new(raft, storage)
    }

    #[tokio::test]
    async fn database_service_put_get_exists_delete_list_and_batch_put() {
        let svc = create_service().await;

        let put_resp = svc
            .put(Request::new(PutRequest {
                collection: "tickets".to_string(),
                key: b"k1".to_vec(),
                value: b"v1".to_vec(),
            }))
            .await
            .expect("put")
            .into_inner();
        assert!(put_resp.success);

        let exists_resp = svc
            .exists(Request::new(ExistsRequest {
                collection: "tickets".to_string(),
                key: b"k1".to_vec(),
            }))
            .await
            .expect("exists")
            .into_inner();
        assert!(exists_resp.exists);

        let get_resp = svc
            .get(Request::new(GetRequest {
                collection: "tickets".to_string(),
                key: b"k1".to_vec(),
            }))
            .await
            .expect("get")
            .into_inner();
        assert!(get_resp.found);
        assert_eq!(get_resp.value, b"v1");

        let batch_resp = svc
            .batch_put(Request::new(BatchPutRequest {
                collection: "tickets".to_string(),
                items: vec![
                    KeyValue {
                        key: b"k2".to_vec(),
                        value: b"v2".to_vec(),
                    },
                    KeyValue {
                        key: b"k3".to_vec(),
                        value: b"v3".to_vec(),
                    },
                ],
            }))
            .await
            .expect("batch_put")
            .into_inner();
        assert!(batch_resp.success);
        assert_eq!(batch_resp.written, 2);

        let list_resp = svc
            .list(Request::new(ListRequest {
                collection: "tickets".to_string(),
                prefix: b"k".to_vec(),
                limit: 10,
            }))
            .await
            .expect("list")
            .into_inner();
        assert!(list_resp.items.len() >= 3);

        let delete_resp = svc
            .delete(Request::new(DeleteRequest {
                collection: "tickets".to_string(),
                key: b"k1".to_vec(),
            }))
            .await
            .expect("delete")
            .into_inner();
        assert!(delete_resp.success);

        let get_deleted = svc
            .get(Request::new(GetRequest {
                collection: "tickets".to_string(),
                key: b"k1".to_vec(),
            }))
            .await
            .expect("get deleted")
            .into_inner();
        assert!(!get_deleted.found);
    }

    #[tokio::test]
    async fn database_service_reports_health_and_cluster_status() {
        let svc = create_service().await;

        let health = svc
            .health(Request::new(HealthRequest {}))
            .await
            .expect("health")
            .into_inner();
        assert_eq!(health.node_id, "1");
        assert!(!health.role.is_empty());

        let status = svc
            .cluster_status(Request::new(ClusterStatusRequest {}))
            .await
            .expect("cluster status")
            .into_inner();
        assert!(status.member_ids.contains(&"1".to_string()));
    }
}
