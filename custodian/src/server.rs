//! gRPC server implementation for the Custodian service
//!
//! This module implements the gRPC endpoint handlers for the Custodian service,
//! managing ticket lifecycle and distributed locking with Raft consensus.

use crate::raft::CustodianRaft;
use crate::storage::LockCommand;
use shared::encryption::EncryptionService;
use tonic::{Request, Response, Status};
use uuid::Uuid;

// Include generated protobuf code
pub mod custodian {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("custodian");
}

use custodian::custodian_service_server::{CustodianService, CustodianServiceServer};
use custodian::{
    CreateTicketRequest, GetTicketRequest, HealthRequest, HealthResponse, LockRelease, LockRequest,
    LockResponse as ProtoLockResponse, Ticket, UpdateTicketRequest,
};
use shared::ticket as domain;

// Expose metrics endpoint via gRPC is handled elsewhere; ensure metrics module is initialized
#[allow(dead_code)]
fn init_metrics() {
    // Touch metrics to ensure they are registered
    let _ = crate::metrics::SNAPSHOT_CREATED_TOTAL.get();
}

/// Custodian service implementation
///
/// Implements the gRPC `CustodianService` with the following behavior:
/// - Ticket operations (Create, Update) are forwarded to the DB service
/// - Lock operations (Acquire, Release) use Raft consensus for coordination
/// - Health checks report Raft cluster state
pub struct CustodianServiceImpl {
    raft: CustodianRaft,
    storage: crate::storage::Storage,
    db_client: Option<std::sync::Arc<tokio::sync::Mutex<crate::db_client::DbClient>>>,
    keypair: (Vec<u8>, Vec<u8>),
}

impl CustodianServiceImpl {
    pub fn with_db_client(
        raft: CustodianRaft,
        storage: crate::storage::Storage,
        db_client: std::sync::Arc<tokio::sync::Mutex<crate::db_client::DbClient>>,
        keypair: (Vec<u8>, Vec<u8>),
    ) -> Self {
        Self {
            raft,
            storage,
            db_client: Some(db_client),
            keypair,
        }
    }

    #[must_use]
    pub fn new(
        raft: CustodianRaft,
        storage: crate::storage::Storage,
        keypair: (Vec<u8>, Vec<u8>),
    ) -> Self {
        Self {
            raft,
            storage,
            db_client: None,
            keypair,
        }
    }
}

impl CustodianServiceImpl {
    /// Convert our domain Ticket to protobuf
    fn domain_to_proto(ticket: &domain::Ticket) -> custodian::Ticket {
        custodian::Ticket {
            ticket_id: ticket.ticket_id,
            customer_ticket_number: ticket.customer_ticket_number.clone(),
            isp_ticket_number: ticket.isp_ticket_number.clone(),
            other_ticket_number: ticket.other_ticket_number.clone(),
            title: ticket.title.clone(),
            project: ticket.project.clone(),
            account_uuid: ticket.account_uuid.to_string(),
            symptom: ticket.symptom as i32,
            priority: ticket.priority as i32,
            status: ticket.status as i32,
            next_action: 0, // TODO: Map NextAction properly
            resolution: ticket.resolution.map(|r| r as i32),
            locked_by_uuid: ticket.locked_by.map(|u| u.to_string()),
            assigned_to_uuid: ticket.assigned_to.map(|u| u.to_string()),
            created_by_uuid: ticket.created_by.to_string(),
            created_at: Some(prost_types::Timestamp::from(std::time::SystemTime::from(
                ticket.created_at,
            ))),
            updated_by_uuid: ticket.updated_by.to_string(),
            updated_at: Some(prost_types::Timestamp::from(std::time::SystemTime::from(
                ticket.updated_at,
            ))),
            history: vec![], // TODO: Convert history
            ebond: ticket.ebond.clone(),
            tracking_url: ticket.tracking_url.clone(),
            network_devices: vec![], // TODO: Convert devices
            schema_version: ticket.schema_version,
        }
    }
}

#[tonic::async_trait]
impl CustodianService for CustodianServiceImpl {
    async fn get_ticket(
        &self,
        request: Request<GetTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();

        if let Some(client) = &self.db_client {
            let mut client = client.lock().await;
            let key = req.ticket_id.to_be_bytes().to_vec();

            match client
                .get("ticket", key)
                .await
                .map_err(|e| Status::internal(format!("db get error: {e}")))?
            {
                Some(bytes) => {
                    // Decrypt
                    let encrypted_data: shared::encryption::EncryptedData =
                        serde_json::from_slice(&bytes).map_err(|e| {
                            Status::internal(format!("deserialize encrypted data error: {e}"))
                        })?;

                    let decrypted_bytes = EncryptionService::decrypt_with_private_key(
                        &encrypted_data,
                        &self.keypair.1,
                    )
                    .map_err(|e| Status::internal(format!("decryption error: {e}")))?;

                    // Deserialize domain object
                    let ticket: domain::Ticket = serde_json::from_slice(&decrypted_bytes)
                        .map_err(|e| Status::internal(format!("deserialize ticket error: {e}")))?;

                    Ok(Response::new(Self::domain_to_proto(&ticket)))
                }
                None => Err(Status::not_found("ticket not found")),
            }
        } else {
            Err(Status::unavailable("no db client configured"))
        }
    }

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
        let ticket_id: u64 = chrono::Utc::now()
            .timestamp_millis()
            .try_into()
            .unwrap_or_default();

        let account_uuid = Uuid::parse_str(&req.account_uuid)
            .map_err(|_| Status::invalid_argument("Invalid account UUID"))?;

        let created_by_uuid = Uuid::parse_str(&req.created_by_uuid)
            .map_err(|_| Status::invalid_argument("Invalid created_by UUID"))?;

        // Create domain ticket
        let symptom = domain::Symptom::from_u8(u8::try_from(req.symptom).unwrap_or(0));
        let priority = domain::TicketPriority::from_u8(u8::try_from(req.priority).unwrap_or(0));

        let mut ticket = domain::Ticket::new(
            ticket_id,
            req.title,
            req.project,
            account_uuid,
            symptom,
            created_by_uuid,
        );

        ticket.priority = priority;

        ticket.customer_ticket_number = req.customer_ticket_number;
        ticket.isp_ticket_number = req.isp_ticket_number;
        ticket.other_ticket_number = req.other_ticket_number;
        ticket.ebond = req.ebond;
        ticket.tracking_url = req.tracking_url;

        // Serialize and Encrypt
        let ticket_bytes = serde_json::to_vec(&ticket)
            .map_err(|e| Status::internal(format!("serialize error: {e}")))?;

        let encrypted = EncryptionService::encrypt_with_public_key(&ticket_bytes, &self.keypair.0)
            .map_err(|e| Status::internal(format!("encryption error: {e}")))?;

        let encrypted_bytes = serde_json::to_vec(&encrypted)
            .map_err(|e| Status::internal(format!("serialize encrypted data error: {e}")))?;

        // Persist to DB if client available
        if let Some(client) = &self.db_client {
            let mut lock = client.lock().await;
            let key = ticket_id.to_be_bytes().to_vec();
            lock.put("ticket", key, encrypted_bytes)
                .await
                .map_err(|e| Status::internal(format!("db put error: {e}")))?;
        }

        Ok(Response::new(Self::domain_to_proto(&ticket)))
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
        let updater_str = req
            .updated_by_uuid
            .as_deref()
            .ok_or_else(|| Status::invalid_argument("updated_by_uuid is required"))?;
        let updater = Uuid::parse_str(updater_str)
            .map_err(|_| Status::invalid_argument("Invalid updated_by_uuid"))?;

        // Check lock ownership
        match self
            .storage
            .get_lock_info(req.ticket_id)
            .map_err(|e| Status::internal(format!("storage error: {e}")))?
        {
            Some(lock) => {
                if lock.user_id != updater {
                    return Err(Status::permission_denied("user does not hold lock"));
                }
            }
            None => return Err(Status::permission_denied("ticket is not locked")),
        }

        // Fetch ticket from DB
        let mut ticket: domain::Ticket = if let Some(client) = &self.db_client {
            let mut client = client.lock().await;
            let key = req.ticket_id.to_be_bytes().to_vec();
            match client
                .get("ticket", key)
                .await
                .map_err(|e| Status::internal(format!("db get error: {e}")))?
            {
                Some(bytes) => {
                    // Decrypt
                    let encrypted_data: shared::encryption::EncryptedData =
                        serde_json::from_slice(&bytes).map_err(|e| {
                            Status::internal(format!("deserialize encrypted data error: {e}"))
                        })?;

                    let decrypted_bytes = EncryptionService::decrypt_with_private_key(
                        &encrypted_data,
                        &self.keypair.1,
                    )
                    .map_err(|e| Status::internal(format!("decryption error: {e}")))?;

                    let ticket: domain::Ticket = serde_json::from_slice(&decrypted_bytes)
                        .map_err(|e| Status::internal(format!("deserialize ticket error: {e}")))?;
                    ticket
                }
                None => return Err(Status::not_found("ticket not found")),
            }
        } else {
            return Err(Status::unavailable("no db client configured"));
        };

        // Apply updates from request (only update provided fields)
        if let Some(title) = req.title {
            ticket.title = title;
        }
        if let Some(project) = req.project {
            ticket.project = project;
        }
        if let Some(symptom) = req.symptom {
            ticket.symptom = domain::Symptom::from_u8(u8::try_from(symptom).unwrap_or(0));
        }
        if let Some(priority) = req.priority {
            ticket.priority = domain::TicketPriority::from_u8(u8::try_from(priority).unwrap_or(0));
        }
        if let Some(status_val) = req.status {
            ticket.status = domain::TicketStatus::from_u8(u8::try_from(status_val).unwrap_or(0));
        }
        if let Some(next_action) = req.next_action {
            // TODO: Map NextAction properly. For now, if unspecified (0), we leave it.
            // If specified, we map to None because we don't have data.
            if next_action != 0 {
                ticket.next_action = domain::NextAction::None;
            }
        }
        if let Some(resolution) = req.resolution {
            ticket.resolution = Some(domain::Resolution::from_u8(
                u8::try_from(resolution).unwrap_or(0),
            ));
        }
        if let Some(assigned) = req.assigned_to_uuid {
            ticket.assigned_to = Some(
                Uuid::parse_str(&assigned)
                    .map_err(|_| Status::invalid_argument("Invalid assigned_to UUID"))?,
            );
        }

        ticket.updated_by = updater;
        ticket.updated_at = chrono::Utc::now();

        // Serialize and Encrypt
        let ticket_bytes = serde_json::to_vec(&ticket)
            .map_err(|e| Status::internal(format!("serialize error: {e}")))?;

        let encrypted = EncryptionService::encrypt_with_public_key(&ticket_bytes, &self.keypair.0)
            .map_err(|e| Status::internal(format!("encryption error: {e}")))?;

        let encrypted_bytes = serde_json::to_vec(&encrypted)
            .map_err(|e| Status::internal(format!("serialize encrypted data error: {e}")))?;

        // Persist updated ticket
        if let Some(client) = &self.db_client {
            let mut client = client.lock().await;
            let key = req.ticket_id.to_be_bytes().to_vec();
            client
                .put("ticket", key, encrypted_bytes)
                .await
                .map_err(|e| Status::internal(format!("db put error: {e}")))?;
        }

        Ok(Response::new(Self::domain_to_proto(&ticket)))
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
                if Some(*id) == metrics.current_leader {
                    None
                } else {
                    Some(id.to_string())
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
pub fn create_server(
    service: CustodianServiceImpl,
) -> CustodianServiceServer<CustodianServiceImpl> {
    CustodianServiceServer::new(service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openraft::Config;
    use openraft::storage::Adaptor;
    use std::sync::Arc;
    use tonic::Request;

    #[tokio::test]
    async fn test_custodian_service_creation() {
        // basic instantiation
        let store = crate::raft::CustodianStore::new_temp().unwrap();
        let storage = store.storage();
        let svc = CustodianServiceImpl::new(
            crate::raft::CustodianRaft::new(
                1,
                Arc::new(Config::default()),
                crate::network::CustodianNetworkFactory::new(),
                Adaptor::new(store.clone()).0,
                Adaptor::new(store).1,
            )
            .await
            .unwrap(),
            storage,
            (vec![0; 1184], vec![0; 2400]),
        );
        let _ = svc;
    }

    #[tokio::test]
    async fn test_create_ticket_and_lock_flow() {
        // Create backing store and raft
        let store = crate::raft::CustodianStore::new_temp().unwrap();
        let storage = store.storage().clone();

        let cfg = Config::default();
        let cfg = Arc::new(cfg.validate().unwrap());
        let network_factory = crate::network::CustodianNetworkFactory::new();
        let (log_store, state_machine) = Adaptor::new(store.clone());

        let raft = crate::raft::CustodianRaft::new(
            1u64,
            cfg.clone(),
            network_factory,
            log_store,
            state_machine,
        )
        .await
        .expect("create raft");
        // initialize single-node cluster so client_write works
        let mut members = std::collections::BTreeSet::new();
        members.insert(1u64);
        let _ = raft.initialize(members).await;

        let svc_impl = CustodianServiceImpl::new(
            raft.clone(),
            storage.clone(),
            (vec![0; 1184], vec![0; 2400]),
        );

        // create ticket
        let req = custodian::CreateTicketRequest {
            title: "Test".to_string(),
            project: "proj".to_string(),
            account_uuid: uuid::Uuid::new_v4().to_string(),
            symptom: 0,
            priority: 0,
            created_by_uuid: uuid::Uuid::new_v4().to_string(),
            customer_ticket_number: None,
            isp_ticket_number: None,
            other_ticket_number: None,
            ebond: None,
            tracking_url: None,
            network_devices: vec![],
        };
        let resp = svc_impl
            .create_ticket(Request::new(req))
            .await
            .expect("create ticket");
        let ticket = resp.into_inner();
        assert_eq!(ticket.title, "Test");
        assert_eq!(ticket.priority, 0);

        // acquire lock using service (should go through raft)
        let user_uuid = uuid::Uuid::new_v4().to_string();
        let lock_req = custodian::LockRequest {
            ticket_id: ticket.ticket_id,
            user_uuid: user_uuid.clone(),
        };
        let lock_resp = svc_impl
            .acquire_lock(Request::new(lock_req))
            .await
            .expect("acquire");
        assert!(lock_resp.get_ref().success);

        // release lock
        let release_req = custodian::LockRelease {
            ticket_id: ticket.ticket_id,
            user_uuid,
        };
        let release_resp = svc_impl
            .release_lock(Request::new(release_req))
            .await
            .expect("release");
        assert!(release_resp.get_ref().success);
    }

    #[test]
    fn test_domain_to_proto_priority() {
        let mut ticket = domain::Ticket::new(
            1,
            "Test".to_string(),
            "Project".to_string(),
            uuid::Uuid::new_v4(),
            domain::Symptom::BroadbandDown,
            uuid::Uuid::new_v4(),
        );

        // Test Unknown (default)
        let proto = CustodianServiceImpl::domain_to_proto(&ticket);
        assert_eq!(proto.priority, 0); // Unknown = 0

        // Test Specific Priority
        ticket.priority = domain::TicketPriority::HardDown;
        let proto = CustodianServiceImpl::domain_to_proto(&ticket);
        assert_eq!(proto.priority, 1); // HardDown = 1
    }
}
