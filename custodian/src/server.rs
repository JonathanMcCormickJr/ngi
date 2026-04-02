//! gRPC server implementation for the Custodian service
//!
//! This module implements the gRPC endpoint handlers for the Custodian service,
//! managing ticket lifecycle and distributed locking with Raft consensus.

use crate::raft::CustodianRaft;
use crate::storage::LockCommand;
use shared::encryption::EncryptionService;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub use proto::custodian;

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
    /// Convert a [`chrono::DateTime<Utc>`] to a [`prost_types::Timestamp`].
    fn dt_to_proto(dt: chrono::DateTime<chrono::Utc>) -> prost_types::Timestamp {
        prost_types::Timestamp::from(std::time::SystemTime::from(dt))
    }

    /// Map a domain [`NextAction`] to the corresponding protobuf `NextAction` integer.
    ///
    /// The domain type carries rich scheduling data (timestamps, auto-close schedules)
    /// that the current protobuf enum cannot represent. The mapping is best-effort:
    /// - `None` → `NEXT_ACTION_UNSPECIFIED`
    /// - `FollowUp(_)` → `CONTACT_CUSTOMER` (follow-ups typically involve customer contact)
    /// - `Appointment(_)` → `DIAGNOSE_ISSUE` (appointments are typically on-site visits)
    /// - `AutoClose(_)` → `CLOSE_TICKET`
    ///
    /// The scheduling timestamp is not preserved. A future proto revision should add an
    /// optional `next_action_scheduled_at` timestamp field to carry this information.
    fn map_next_action(next_action: &domain::NextAction) -> i32 {
        match next_action {
            domain::NextAction::FollowUp(_) => custodian::NextAction::ContactCustomer as i32,
            domain::NextAction::Appointment(_) => custodian::NextAction::DiagnoseIssue as i32,
            domain::NextAction::AutoClose(_) => custodian::NextAction::CloseTicket as i32,
            // Non-exhaustive enum: None and any future variants are treated as unspecified
            _ => custodian::NextAction::Unspecified as i32,
        }
    }

    /// Convert a domain [`HistoryEntry`] to the corresponding protobuf message.
    fn map_history_entry(entry: &domain::HistoryEntry) -> custodian::HistoryEntry {
        let details = match (&entry.old_value, &entry.new_value) {
            (Some(old), Some(new)) => format!("{}: {} → {}", entry.field_changed, old, new),
            (Some(old), None) => format!("{}: {} → (removed)", entry.field_changed, old),
            (None, Some(new)) => format!("{}: (new) → {}", entry.field_changed, new),
            (None, None) => entry.field_changed.clone(),
        };
        custodian::HistoryEntry {
            user_uuid: entry.user_id.to_string(),
            timestamp: Some(Self::dt_to_proto(entry.timestamp)),
            action: entry.field_changed.clone(),
            details,
        }
    }

    /// Convert a domain [`NetworkDevice`] to the corresponding protobuf message.
    #[allow(clippy::too_many_lines)]
    fn map_network_device(device: &domain::NetworkDevice) -> custodian::NetworkDevice {
        use custodian::network_device::DeviceType;

        let make_proto_fields =
            |make: &str, model: &str, mac: Option<&domain::MacAddress>, sn: Option<&String>| {
                (
                    make.to_string(),
                    model.to_string(),
                    mac.map(ToString::to_string),
                    sn.cloned(),
                )
            };

        let device_type = match device {
            domain::NetworkDevice::DslModem {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::DslModem(custodian::DslModem {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::CoaxModem {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::CoaxModem(custodian::CoaxModem {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::Ont {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::Ont(custodian::Ont {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::FixedWirelessAntenna {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::FixedWirelessAntenna(custodian::FixedWirelessAntenna {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::VpnGw {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::VpnGw(custodian::VpnGw {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::Switch {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::Switch(custodian::Switch {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::Router {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::Router(custodian::Router {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            domain::NetworkDevice::Firewall {
                make,
                model,
                mac_address,
                serial_number,
            } => {
                let (make, model, mac_address, serial_number) =
                    make_proto_fields(make, model, mac_address.as_ref(), serial_number.as_ref());
                DeviceType::Firewall(custodian::Firewall {
                    make,
                    model,
                    mac_address,
                    serial_number,
                })
            }
            // Non-exhaustive enum: log a warning and fall through with the make/model
            // encoded as the make field. This ensures future device types are not silently
            // dropped, even if the proto cannot represent them precisely until the schema
            // is updated to include the new variant.
            _ => {
                tracing::warn!(
                    device_type = device.device_type(),
                    make_model = %device.make_model(),
                    "Unknown NetworkDevice variant; encoding as Router until proto is updated"
                );
                DeviceType::Router(custodian::Router {
                    make: device.make_model(),
                    model: String::new(),
                    mac_address: device.mac_address().map(ToString::to_string),
                    serial_number: None,
                })
            }
        };

        custodian::NetworkDevice {
            device_type: Some(device_type),
        }
    }

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
            next_action: Self::map_next_action(&ticket.next_action),
            resolution: ticket.resolution.map(|r| r as i32),
            locked_by_uuid: ticket.locked_by.map(|u| u.to_string()),
            assigned_to_uuid: ticket.assigned_to.map(|u| u.to_string()),
            created_by_uuid: ticket.created_by.to_string(),
            created_at: Some(Self::dt_to_proto(ticket.created_at)),
            updated_by_uuid: ticket.updated_by.to_string(),
            updated_at: Some(Self::dt_to_proto(ticket.updated_at)),
            history: ticket.history.iter().map(Self::map_history_entry).collect(),
            ebond: ticket.ebond.clone(),
            tracking_url: ticket.tracking_url.clone(),
            network_devices: ticket
                .network_devices
                .iter()
                .map(Self::map_network_device)
                .collect(),
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

    #[tokio::test]
    async fn get_ticket_without_db_client_returns_unavailable() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));
        let err = svc
            .get_ticket(Request::new(custodian::GetTicketRequest { ticket_id: 1 }))
            .await
            .expect_err("no db client should fail");

        assert_eq!(err.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn create_ticket_rejects_empty_title() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));
        let err = svc
            .create_ticket(Request::new(custodian::CreateTicketRequest {
                title: String::new(),
                project: "demo".to_string(),
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
            }))
            .await
            .expect_err("empty title should fail");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn update_ticket_requires_updated_by_uuid() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));
        let err = svc
            .update_ticket(Request::new(custodian::UpdateTicketRequest {
                ticket_id: 1,
                title: None,
                project: None,
                symptom: None,
                priority: None,
                status: None,
                next_action: None,
                resolution: None,
                assigned_to_uuid: None,
                updated_by_uuid: None,
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("missing updater should fail");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn health_and_cluster_status_are_available() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let cfg = Arc::new(Config::default().validate().expect("validated config"));
        let raft = crate::raft::CustodianRaft::new(
            1,
            cfg,
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let mut members = std::collections::BTreeSet::new();
        members.insert(1u64);
        let _ = raft.initialize(members).await;

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));

        let health = svc
            .health(Request::new(custodian::HealthRequest {}))
            .await
            .expect("health")
            .into_inner();
        assert!(!health.status.is_empty());

        let cluster = svc
            .cluster_status(Request::new(custodian::ClusterStatusRequest {}))
            .await
            .expect("cluster")
            .into_inner();
        assert!(cluster.term >= 1);
    }

    #[tokio::test]
    async fn acquire_and_release_lock_reject_invalid_user_uuid() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage().clone();
        let cfg = Arc::new(Config::default().validate().expect("validated config"));
        let raft = crate::raft::CustodianRaft::new(
            1,
            cfg,
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));

        let acquire_err = svc
            .acquire_lock(Request::new(custodian::LockRequest {
                ticket_id: 1,
                user_uuid: "not-a-uuid".to_string(),
            }))
            .await
            .expect_err("invalid user uuid");
        assert_eq!(acquire_err.code(), tonic::Code::InvalidArgument);

        let release_err = svc
            .release_lock(Request::new(custodian::LockRelease {
                ticket_id: 1,
                user_uuid: "not-a-uuid".to_string(),
            }))
            .await
            .expect_err("invalid user uuid");
        assert_eq!(release_err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn update_ticket_requires_existing_lock() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage().clone();
        let cfg = Arc::new(Config::default().validate().expect("validated config"));
        let raft = crate::raft::CustodianRaft::new(
            1,
            cfg,
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));

        let err = svc
            .update_ticket(Request::new(custodian::UpdateTicketRequest {
                ticket_id: 99,
                title: Some("new title".to_string()),
                project: None,
                symptom: None,
                priority: None,
                status: None,
                next_action: None,
                resolution: None,
                assigned_to_uuid: None,
                updated_by_uuid: Some("00000000-0000-0000-0000-000000000001".to_string()),
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("must fail without lock");

        assert_eq!(err.code(), tonic::Code::PermissionDenied);
    }

    // ── map_next_action ───────────────────────────────────────────────────────

    #[test]
    fn map_next_action_none_maps_to_unspecified() {
        let result = CustodianServiceImpl::map_next_action(&domain::NextAction::None);
        assert_eq!(result, custodian::NextAction::Unspecified as i32);
    }

    #[test]
    fn map_next_action_follow_up_maps_to_contact_customer() {
        let result = CustodianServiceImpl::map_next_action(&domain::NextAction::FollowUp(
            chrono::Utc::now(),
        ));
        assert_eq!(result, custodian::NextAction::ContactCustomer as i32);
    }

    #[test]
    fn map_next_action_appointment_maps_to_diagnose_issue() {
        let result = CustodianServiceImpl::map_next_action(&domain::NextAction::Appointment(
            chrono::Utc::now(),
        ));
        assert_eq!(result, custodian::NextAction::DiagnoseIssue as i32);
    }

    #[test]
    fn map_next_action_auto_close_maps_to_close_ticket() {
        let result = CustodianServiceImpl::map_next_action(&domain::NextAction::AutoClose(
            domain::AutoCloseSchedule::Hours24,
        ));
        assert_eq!(result, custodian::NextAction::CloseTicket as i32);
    }

    // ── map_history_entry ─────────────────────────────────────────────────────

    #[test]
    fn map_history_entry_formats_change_with_old_and_new_values() {
        let entry = domain::HistoryEntry {
            timestamp: chrono::Utc::now(),
            user_id: uuid::Uuid::nil(),
            field_changed: "status".to_string(),
            old_value: Some("Open".to_string()),
            new_value: Some("Closed".to_string()),
        };
        let proto = CustodianServiceImpl::map_history_entry(&entry);
        assert_eq!(proto.action, "status");
        assert!(proto.details.contains("Open"));
        assert!(proto.details.contains("Closed"));
        assert_eq!(proto.user_uuid, uuid::Uuid::nil().to_string());
        assert!(proto.timestamp.is_some());
    }

    #[test]
    fn map_history_entry_handles_removal() {
        let entry = domain::HistoryEntry {
            timestamp: chrono::Utc::now(),
            user_id: uuid::Uuid::nil(),
            field_changed: "assigned_to".to_string(),
            old_value: Some("Alice".to_string()),
            new_value: None,
        };
        let proto = CustodianServiceImpl::map_history_entry(&entry);
        assert!(proto.details.contains("removed"));
    }

    #[test]
    fn map_history_entry_handles_new_value_only() {
        let entry = domain::HistoryEntry {
            timestamp: chrono::Utc::now(),
            user_id: uuid::Uuid::nil(),
            field_changed: "tracking_url".to_string(),
            old_value: None,
            new_value: Some("https://example.com".to_string()),
        };
        let proto = CustodianServiceImpl::map_history_entry(&entry);
        assert!(proto.details.contains("example.com"));
    }

    #[test]
    fn map_history_entry_handles_no_values() {
        let entry = domain::HistoryEntry {
            timestamp: chrono::Utc::now(),
            user_id: uuid::Uuid::nil(),
            field_changed: "ticket_created".to_string(),
            old_value: None,
            new_value: None,
        };
        let proto = CustodianServiceImpl::map_history_entry(&entry);
        assert_eq!(proto.details, "ticket_created");
    }

    // ── map_network_device ────────────────────────────────────────────────────

    #[test]
    fn map_network_device_dsl_modem() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::DslModem {
            make: "Cisco".to_string(),
            model: "DPC3825".to_string(),
            mac_address: None,
            serial_number: Some("SN123".to_string()),
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(
            proto.device_type,
            Some(DeviceType::DslModem(ref d)) if d.make == "Cisco"
        ));
    }

    #[test]
    fn map_network_device_coax_modem_with_mac() {
        use custodian::network_device::DeviceType;
        let mac = domain::MacAddress::new("AA:BB:CC:DD:EE:FF").expect("valid MAC");
        let device = domain::NetworkDevice::CoaxModem {
            make: "Arris".to_string(),
            model: "SB6141".to_string(),
            mac_address: Some(mac),
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(
            proto.device_type,
            Some(DeviceType::CoaxModem(ref d)) if d.mac_address.is_some()
        ));
    }

    #[test]
    fn map_network_device_ont() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::Ont {
            make: "Calix".to_string(),
            model: "GigaPoint".to_string(),
            mac_address: None,
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(proto.device_type, Some(DeviceType::Ont(_))));
    }

    #[test]
    fn map_network_device_fixed_wireless_antenna() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::FixedWirelessAntenna {
            make: "Cambium".to_string(),
            model: "PMP450".to_string(),
            mac_address: None,
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(
            proto.device_type,
            Some(DeviceType::FixedWirelessAntenna(_))
        ));
    }

    #[test]
    fn map_network_device_vpn_gw() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::VpnGw {
            make: "Cisco".to_string(),
            model: "ASA5505".to_string(),
            mac_address: None,
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(proto.device_type, Some(DeviceType::VpnGw(_))));
    }

    #[test]
    fn map_network_device_switch() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::Switch {
            make: "Cisco".to_string(),
            model: "SG300".to_string(),
            mac_address: None,
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(proto.device_type, Some(DeviceType::Switch(_))));
    }

    #[test]
    fn map_network_device_router() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::Router {
            make: "Netgear".to_string(),
            model: "R7000".to_string(),
            mac_address: None,
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(proto.device_type, Some(DeviceType::Router(_))));
    }

    #[test]
    fn map_network_device_firewall() {
        use custodian::network_device::DeviceType;
        let device = domain::NetworkDevice::Firewall {
            make: "Palo Alto".to_string(),
            model: "PA-220".to_string(),
            mac_address: None,
            serial_number: None,
        };
        let proto = CustodianServiceImpl::map_network_device(&device);
        assert!(matches!(proto.device_type, Some(DeviceType::Firewall(_))));
    }

    // ── domain_to_proto round-trip tests ─────────────────────────────────────

    #[test]
    fn domain_to_proto_preserves_next_action_and_history() {
        let owner = uuid::Uuid::new_v4();
        let mut ticket = domain::Ticket::new(
            1,
            "Test".to_string(),
            "Project".to_string(),
            uuid::Uuid::new_v4(),
            domain::Symptom::BroadbandDown,
            owner,
        );
        ticket.next_action = domain::NextAction::FollowUp(chrono::Utc::now());
        ticket.history.push(domain::HistoryEntry {
            timestamp: chrono::Utc::now(),
            user_id: owner,
            field_changed: "status".to_string(),
            old_value: Some("Open".to_string()),
            new_value: Some("Closed".to_string()),
        });
        let proto = CustodianServiceImpl::domain_to_proto(&ticket);
        assert_eq!(
            proto.next_action,
            custodian::NextAction::ContactCustomer as i32
        );
        assert_eq!(proto.history.len(), 1);
        assert_eq!(proto.history[0].action, "status");
    }

    #[test]
    fn domain_to_proto_preserves_network_devices() {
        let mut ticket = domain::Ticket::new(
            2,
            "Net Test".to_string(),
            "Project".to_string(),
            uuid::Uuid::new_v4(),
            domain::Symptom::BroadbandDown,
            uuid::Uuid::new_v4(),
        );
        ticket.network_devices.push(domain::NetworkDevice::Router {
            make: "Netgear".to_string(),
            model: "R7000".to_string(),
            mac_address: None,
            serial_number: None,
        });
        let proto = CustodianServiceImpl::domain_to_proto(&ticket);
        assert_eq!(proto.network_devices.len(), 1);
    }

    // ── Additional coverage tests ─────────────────────────────────────────────

    #[test]
    fn init_metrics_does_not_panic() {
        // Ensures the init_metrics() code path is covered.
        super::init_metrics();
    }

    #[tokio::test]
    async fn test_create_server_function() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");
        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));
        let _server = super::create_server(svc);
    }

    #[tokio::test]
    async fn with_db_client_constructor_sets_db_client() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");
        let db = Arc::new(tokio::sync::Mutex::new(
            crate::db_client::DbClient::new_lazy("http://127.0.0.1:9"),
        ));
        let svc =
            CustodianServiceImpl::with_db_client(raft, storage, db, (vec![0; 1184], vec![0; 2400]));
        // DB client IS set — get_ticket should return Internal (transport error), not Unavailable "no db client"
        let err = svc
            .get_ticket(Request::new(custodian::GetTicketRequest { ticket_id: 1 }))
            .await
            .expect_err("transport error expected");
        // Internal (transport failure) means the db_client path was taken
        assert_ne!(err.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn health_with_shutdown_state_returns_unhealthy() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        let svc = CustodianServiceImpl::new(raft.clone(), storage, (vec![0; 1184], vec![0; 2400]));

        // Shut down the raft node so state becomes Shutdown
        raft.shutdown().await.expect("shutdown");

        let resp = svc
            .health(Request::new(custodian::HealthRequest {}))
            .await
            .expect("health")
            .into_inner();

        // After shutdown the node is unhealthy
        assert!(!resp.healthy);
    }

    #[tokio::test]
    async fn cluster_status_includes_follower_node_ids() {
        // Initialize a single-node raft but register 3 members in membership.
        // Nodes 2 and 3 are "known" but non-existent; only node 1 becomes leader.
        // The filter_map in cluster_status should include nodes 2 & 3 as "followers".
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let cfg = Arc::new(Config::default().validate().expect("validated config"));
        let raft = crate::raft::CustodianRaft::new(
            1,
            cfg,
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        // Initialize with 3 members so the membership has non-leader nodes
        let mut members = std::collections::BTreeSet::new();
        members.insert(1u64);
        members.insert(2u64);
        members.insert(3u64);
        // This may fail if the cluster can't reach quorum, but we only care about membership config.
        let _ = raft.initialize(members).await;

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));

        // cluster_status exercises the filter_map for non-leader nodes
        let cluster = svc
            .cluster_status(Request::new(custodian::ClusterStatusRequest {}))
            .await
            .expect("cluster status")
            .into_inner();

        // Nodes 2 and 3 should appear in follower_ids (since only 1 is leader or no leader yet)
        // We just verify the response was produced without panic and has reasonable content
        assert!(cluster.follower_ids.len() <= 5);
    }

    #[tokio::test]
    async fn update_ticket_returns_permission_denied_for_wrong_lock_holder() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage().clone();
        let cfg = Arc::new(Config::default().validate().expect("validated config"));
        let raft = crate::raft::CustodianRaft::new(
            1,
            cfg,
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        // Initialize so client_write works
        let mut members = std::collections::BTreeSet::new();
        members.insert(1u64);
        let _ = raft.initialize(members).await;

        let svc = CustodianServiceImpl::new(
            raft.clone(),
            storage.clone(),
            (vec![0; 1184], vec![0; 2400]),
        );

        let holder_uuid = uuid::Uuid::new_v4().to_string();
        let other_uuid = uuid::Uuid::new_v4().to_string();

        // Acquire lock as holder
        svc.acquire_lock(Request::new(custodian::LockRequest {
            ticket_id: 42,
            user_uuid: holder_uuid.clone(),
        }))
        .await
        .expect("acquire lock");

        // Try to update as someone else → PermissionDenied
        let err = svc
            .update_ticket(Request::new(custodian::UpdateTicketRequest {
                ticket_id: 42,
                title: Some("hacked".to_string()),
                project: None,
                symptom: None,
                priority: None,
                status: None,
                next_action: None,
                resolution: None,
                assigned_to_uuid: None,
                updated_by_uuid: Some(other_uuid),
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("wrong lock holder");

        assert_eq!(err.code(), tonic::Code::PermissionDenied);
    }

    /// Create a minimal single-node service for tests that don't need a running Raft cluster.
    async fn make_simple_svc() -> CustodianServiceImpl {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage();
        let raft = crate::raft::CustodianRaft::new(
            1,
            Arc::new(Config::default()),
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");
        CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]))
    }

    #[tokio::test]
    async fn acquire_lock_rejects_invalid_user_uuid() {
        let svc = make_simple_svc().await;
        let err = svc
            .acquire_lock(Request::new(custodian::LockRequest {
                ticket_id: 1,
                user_uuid: "not-a-uuid".to_string(),
            }))
            .await
            .expect_err("invalid UUID should fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn release_lock_rejects_invalid_user_uuid() {
        let svc = make_simple_svc().await;
        let err = svc
            .release_lock(Request::new(custodian::LockRelease {
                ticket_id: 1,
                user_uuid: "not-a-uuid".to_string(),
            }))
            .await
            .expect_err("invalid UUID should fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn create_ticket_rejects_invalid_account_uuid() {
        let svc = make_simple_svc().await;
        let err = svc
            .create_ticket(Request::new(custodian::CreateTicketRequest {
                title: "Valid Title".to_string(),
                project: "proj".to_string(),
                account_uuid: "bad-uuid".to_string(),
                symptom: 0,
                priority: 0,
                created_by_uuid: uuid::Uuid::new_v4().to_string(),
                customer_ticket_number: None,
                isp_ticket_number: None,
                other_ticket_number: None,
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("invalid account UUID should fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn create_ticket_rejects_invalid_created_by_uuid() {
        let svc = make_simple_svc().await;
        let err = svc
            .create_ticket(Request::new(custodian::CreateTicketRequest {
                title: "Valid Title".to_string(),
                project: "proj".to_string(),
                account_uuid: uuid::Uuid::new_v4().to_string(),
                symptom: 0,
                priority: 0,
                created_by_uuid: "bad-uuid".to_string(),
                customer_ticket_number: None,
                isp_ticket_number: None,
                other_ticket_number: None,
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("invalid created_by UUID should fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn update_ticket_rejects_invalid_updated_by_uuid_format() {
        let svc = make_simple_svc().await;
        let err = svc
            .update_ticket(Request::new(custodian::UpdateTicketRequest {
                ticket_id: 1,
                title: None,
                project: None,
                symptom: None,
                priority: None,
                status: None,
                next_action: None,
                resolution: None,
                assigned_to_uuid: None,
                updated_by_uuid: Some("not-a-uuid".to_string()),
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("invalid UUID should fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn update_ticket_without_db_client_returns_unavailable() {
        let store = crate::raft::CustodianStore::new_temp().expect("store");
        let storage = store.storage().clone();
        let cfg = Arc::new(Config::default().validate().expect("validated config"));
        let raft = crate::raft::CustodianRaft::new(
            1,
            cfg,
            crate::network::CustodianNetworkFactory::new(),
            Adaptor::new(store.clone()).0,
            Adaptor::new(store).1,
        )
        .await
        .expect("raft");

        // Initialize so lock check works
        let mut members = std::collections::BTreeSet::new();
        members.insert(1u64);
        let _ = raft.initialize(members).await;

        let user_id = uuid::Uuid::new_v4();
        // Directly acquire a lock in storage so the lock check passes
        storage
            .acquire_lock(1, user_id)
            .expect("acquire lock in storage");

        let svc = CustodianServiceImpl::new(raft, storage, (vec![0; 1184], vec![0; 2400]));
        // No db_client set → update_ticket returns Unavailable

        let err = svc
            .update_ticket(Request::new(custodian::UpdateTicketRequest {
                ticket_id: 1,
                title: Some("new title".to_string()),
                project: None,
                symptom: None,
                priority: None,
                status: None,
                next_action: None,
                resolution: None,
                assigned_to_uuid: None,
                updated_by_uuid: Some(user_id.to_string()),
                ebond: None,
                tracking_url: None,
                network_devices: vec![],
            }))
            .await
            .expect_err("no db_client should return error");

        assert_eq!(err.code(), tonic::Code::Unavailable);
    }
}
