//! gRPC server implementation for the Custodian service
//!
//! This module implements the gRPC endpoint handlers for the Custodian service,
//! managing ticket lifecycle and distributed locking with Raft consensus.

use crate::raft::{CustodianRaft, LockResponse};
use prost::Message;
use crate::storage::LockCommand;
use tonic::{Request, Response, Status};
use uuid::Uuid;

// Include generated protobuf code
pub mod custodian {
    tonic::include_proto!("custodian");
}

use custodian::custodian_service_server::{CustodianService, CustodianServiceServer};
use custodian::{
    AcquireLockCommand, CreateTicketRequest, HealthRequest,
    HealthResponse, LockRelease, LockRequest, LockResponse as ProtoLockResponse,
    ReleaseLockCommand, Ticket, UpdateTicketRequest,
};

// Expose metrics endpoint via gRPC is handled elsewhere; ensure metrics module is initialized
#[allow(dead_code)]
fn init_metrics() {
    // Touch metrics to ensure they are registered
    let _ = crate::metrics::SNAPSHOT_CREATED_TOTAL.get();
}

/// Custodian service implementation
///
/// Implements the gRPC CustodianService with the following behavior:
/// - Ticket operations (Create, Update) are forwarded to the DB service
/// - Lock operations (Acquire, Release) use Raft consensus for coordination
/// - Health checks report Raft cluster state
pub struct CustodianServiceImpl {
    raft: CustodianRaft,
    storage: crate::storage::Storage,
    db_client: Option<std::sync::Arc<tokio::sync::Mutex<crate::db_client::DbClient>>>,
}

impl CustodianServiceImpl {
    pub fn with_db_client(raft: CustodianRaft, storage: crate::storage::Storage, db_client: std::sync::Arc<tokio::sync::Mutex<crate::db_client::DbClient>>) -> Self {
        Self { raft, storage, db_client: Some(db_client) }
    }

    pub fn new(raft: CustodianRaft, storage: crate::storage::Storage) -> Self {
        Self { raft, storage, db_client: None }
    }
}

impl CustodianServiceImpl {
    /// Convert protobuf Ticket to our domain type
    fn proto_to_ticket(_proto: &custodian::Ticket) -> Result<shared::Ticket, Status> {
        // TODO: Implement conversion from protobuf to shared::Ticket
        Err(Status::unimplemented("Ticket conversion not implemented"))
    }

    /// Convert our domain Ticket to protobuf
    fn ticket_to_proto(_ticket: &shared::Ticket) -> custodian::Ticket {
        // TODO: Implement conversion from shared::Ticket to protobuf
        custodian::Ticket {
            ticket_id: 0,
            customer_ticket_number: None,
            isp_ticket_number: None,
            other_ticket_number: None,
            title: String::new(),
            project: String::new(),
            account_uuid: String::new(),
            symptom: 0,
            status: 0,
            next_action: 0,
            resolution: None,
            locked_by_uuid: None,
            assigned_to_uuid: None,
            created_by_uuid: String::new(),
            created_at: None,
            updated_by_uuid: String::new(),
            updated_at: None,
            history: vec![],
            ebond: None,
            tracking_url: None,
            network_devices: vec![],
            schema_version: 0,
        }
    }
}

#[tonic::async_trait]
impl CustodianService for CustodianServiceImpl {
    async fn create_ticket(
        &self,
        request: Request<CreateTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();

        // Basic validation
        if req.title.is_empty() {
            return Err(Status::invalid_argument("title is required"));
        }

        // Generate ticket id (millisecond timestamp)
        let ticket_id = chrono::Utc::now().timestamp_millis() as u64;

        // Build proto ticket
        let ticket = custodian::Ticket {
            ticket_id,
            customer_ticket_number: None,
            isp_ticket_number: None,
            other_ticket_number: None,
            title: req.title.clone(),
            project: req.project.clone(),
            account_uuid: req.account_uuid.clone(),
            symptom: req.symptom as i32,
            status: 0,
            next_action: 0,
            resolution: None,
            locked_by_uuid: None,
            assigned_to_uuid: None,
            created_by_uuid: req.created_by_uuid.clone(),
            created_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            updated_by_uuid: String::new(),
            updated_at: None,
            history: vec![],
            ebond: req.customer_ticket_number.clone(),
            tracking_url: None,
            network_devices: vec![],
            schema_version: 1,
        };

        // Persist to DB if client available
        if let Some(client) = &self.db_client {
            let mut lock = client.lock().await;
            let key = ticket_id.to_be_bytes().to_vec();
            let mut bytes = Vec::new();
            ticket.encode(&mut bytes).map_err(|e| Status::internal(format!("encode error: {}", e)))?;
            lock.put("ticket", key, bytes).await.map_err(|e| Status::internal(format!("db put error: {}", e)))?;
        }

        Ok(Response::new(ticket))
    }

    async fn acquire_lock(
        &self,
        request: Request<LockRequest>,
    ) -> Result<Response<ProtoLockResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_uuid)
            .map_err(|_| Status::invalid_argument("Invalid user UUID"))?;

        let command = LockCommand::AcquireLock {
            ticket_id: req.ticket_id,
            user_id,
        };

        // Submit to Raft for consensus
        match self.raft.client_write(command).await {
            Ok(response) => {
                let proto_response = ProtoLockResponse {
                    success: response.data.success,
                    error: response.data.error.unwrap_or_default(),
                    current_holder: None, // TODO: Get current holder if lock failed
                };
                Ok(Response::new(proto_response))
            }
            Err(e) => Err(Status::internal(format!("Raft write failed: {e}"))),
        }
    }

    async fn release_lock(
        &self,
        request: Request<LockRelease>,
    ) -> Result<Response<ProtoLockResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_uuid)
            .map_err(|_| Status::invalid_argument("Invalid user UUID"))?;

        let command = LockCommand::ReleaseLock {
            ticket_id: req.ticket_id,
            user_id,
        };

        // Submit to Raft for consensus
        match self.raft.client_write(command).await {
            Ok(response) => {
                let proto_response = ProtoLockResponse {
                    success: response.data.success,
                    error: response.data.error.unwrap_or_default(),
                    current_holder: None,
                };
                Ok(Response::new(proto_response))
            }
            Err(e) => Err(Status::internal(format!("Raft write failed: {e}"))),
        }
    }

    async fn update_ticket(
        &self,
        request: Request<UpdateTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();

        // Validate updated_by_uuid
        let updater_str = req.updated_by_uuid.as_deref().ok_or_else(|| Status::invalid_argument("updated_by_uuid is required"))?;
        let updater = Uuid::parse_str(updater_str)
            .map_err(|_| Status::invalid_argument("Invalid updated_by_uuid"))?;

        // Check lock ownership
        match self.storage.get_lock_info(req.ticket_id).map_err(|e| Status::internal(format!("storage error: {}", e)))? {
            Some(lock) => {
                if lock.user_id != updater {
                    return Err(Status::permission_denied("user does not hold lock"));
                }
            }
            None => return Err(Status::permission_denied("ticket is not locked")),
        }

        // Fetch ticket from DB
        let mut ticket_proto: custodian::Ticket = if let Some(client) = &self.db_client {
            let mut client = client.lock().await;
            let key = req.ticket_id.to_be_bytes().to_vec();
            match client.get("ticket", key).await.map_err(|e| Status::internal(format!("db get error: {}", e)))? {
                Some(bytes) => prost::Message::decode(bytes.as_slice()).map_err(|e| Status::internal(format!("decode error: {}", e)))?,
                None => return Err(Status::not_found("ticket not found")),
            }
        } else {
            return Err(Status::unavailable("no db client configured"));
        };

        // Apply updates from request (only update provided fields)
        if let Some(title) = req.title { ticket_proto.title = title; }
        if let Some(project) = req.project { ticket_proto.project = project; }
        if let Some(symptom) = req.symptom { ticket_proto.symptom = symptom as i32; }
        if let Some(status_val) = req.status { ticket_proto.status = status_val as i32; }
        if let Some(next_action) = req.next_action { ticket_proto.next_action = next_action as i32; }
        if let Some(resolution) = req.resolution { ticket_proto.resolution = Some(resolution); }
        if let Some(assigned) = req.assigned_to_uuid { ticket_proto.assigned_to_uuid = Some(assigned); }
        ticket_proto.updated_by_uuid = req.updated_by_uuid.clone().unwrap_or_default();
        ticket_proto.updated_at = Some(prost_types::Timestamp::from(std::time::SystemTime::now()));

        // Persist updated ticket
        if let Some(client) = &self.db_client {
            let mut client = client.lock().await;
            let key = req.ticket_id.to_be_bytes().to_vec();
            let mut buf = Vec::new();
            ticket_proto.encode(&mut buf).map_err(|e| Status::internal(format!("encode error: {}", e)))?;
            client.put("ticket", key, buf).await.map_err(|e| Status::internal(format!("db put error: {}", e)))?;
        }

        Ok(Response::new(ticket_proto))
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let metrics = self.raft.metrics().borrow().clone();

        let status = match metrics.state {
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
            status,
        }))
    }

    async fn cluster_status(
        &self,
        _request: Request<custodian::ClusterStatusRequest>,
    ) -> Result<Response<custodian::ClusterStatusResponse>, Status> {
        let metrics = self.raft.metrics().borrow().clone();

        let leader_id = metrics
            .current_leader
            .map(|id| id.to_string())
            .unwrap_or_default();
        let follower_ids: Vec<String> = metrics
            .membership_config
            .membership()
            .nodes()
            .filter_map(|(id, _node)| {
                if Some(*id) != metrics.current_leader {
                    Some(id.to_string())
                } else {
                    None
                }
            })
            .collect();
        let term = metrics.current_term;
        let commit_index = metrics.last_applied.map_or(0, |id| id.index);

        Ok(Response::new(custodian::ClusterStatusResponse {
            leader_id,
            follower_ids,
            term,
            commit_index,
        }))
    }
}

/// Create the gRPC server
#[must_use]
pub fn create_server(service: CustodianServiceImpl) -> CustodianServiceServer<CustodianServiceImpl> {
    CustodianServiceServer::new(service)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_custodian_service_creation() {
        // This is a placeholder test - actual testing requires setting up a full Raft node
        // which we'll do in integration tests
    }
}