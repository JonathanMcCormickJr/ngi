//! gRPC client implementations for LBRP service communication
#![allow(dead_code)]

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

// Include generated protobuf code
pub mod custodian {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("custodian");
}

pub mod db {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("db");
}

pub mod auth {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("auth");
}

pub mod admin {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("admin");
}

/// Custodian service client
#[derive(Clone)]
pub struct CustodianClient {
    pub client: Arc<Mutex<custodian::custodian_service_client::CustodianServiceClient<Channel>>>,
}

impl CustodianClient {
    pub async fn connect(addr: String) -> Result<Self> {
        let channel = Channel::from_shared(addr)?.connect().await?;
        let client = custodian::custodian_service_client::CustodianServiceClient::new(channel);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub async fn create_ticket(
        &self,
        req: custodian::CreateTicketRequest,
    ) -> Result<custodian::Ticket> {
        let mut client = self.client.lock().await;
        let response = client.create_ticket(req).await?;
        Ok(response.into_inner())
    }

    pub async fn acquire_lock(
        &self,
        req: custodian::LockRequest,
    ) -> Result<custodian::LockResponse> {
        let mut client = self.client.lock().await;
        let response = client.acquire_lock(req).await?;
        Ok(response.into_inner())
    }

    pub async fn release_lock(
        &self,
        req: custodian::LockRelease,
    ) -> Result<custodian::LockResponse> {
        let mut client = self.client.lock().await;
        let response = client.release_lock(req).await?;
        Ok(response.into_inner())
    }

    pub async fn update_ticket(
        &self,
        req: custodian::UpdateTicketRequest,
    ) -> Result<custodian::Ticket> {
        let mut client = self.client.lock().await;
        let response = client.update_ticket(req).await?;
        Ok(response.into_inner())
    }

    pub async fn get_ticket(&self, req: custodian::GetTicketRequest) -> Result<custodian::Ticket> {
        let mut client = self.client.lock().await;
        let response = client.get_ticket(req).await?;
        Ok(response.into_inner())
    }

    pub async fn cluster_status(&self) -> Result<custodian::ClusterStatusResponse> {
        let mut client = self.client.lock().await;
        let response = client
            .cluster_status(custodian::ClusterStatusRequest {})
            .await?;
        Ok(response.into_inner())
    }
}

/// Auth service client
#[derive(Clone)]
pub struct AuthClient {
    pub client: Arc<Mutex<auth::auth_service_client::AuthServiceClient<Channel>>>,
}

impl AuthClient {
    pub async fn connect(addr: String) -> Result<Self> {
        let channel = Channel::from_shared(addr)?.connect().await?;
        let client = auth::auth_service_client::AuthServiceClient::new(channel);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }
}

/// Admin service client
#[derive(Clone)]
pub struct AdminClient {
    pub client: Arc<Mutex<admin::admin_service_client::AdminServiceClient<Channel>>>,
}

impl AdminClient {
    pub async fn connect(addr: String) -> Result<Self> {
        let channel = Channel::from_shared(addr)?.connect().await?;
        let client = admin::admin_service_client::AdminServiceClient::new(channel);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }
}

/// DB service client
#[derive(Clone)]
pub struct DbClient {
    pub client: Arc<Mutex<db::database_client::DatabaseClient<Channel>>>,
}

impl DbClient {
    pub async fn connect(addr: String) -> Result<Self> {
        let channel = Channel::from_shared(addr)?.connect().await?;
        let client = db::database_client::DatabaseClient::new(channel);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    // Add DB client methods as needed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unreachable_channel() -> Channel {
        Channel::from_static("http://127.0.0.1:9").connect_lazy()
    }

    fn test_custodian_client() -> CustodianClient {
        CustodianClient {
            client: Arc::new(Mutex::new(
                custodian::custodian_service_client::CustodianServiceClient::new(
                    unreachable_channel(),
                ),
            )),
        }
    }

    #[tokio::test]
    async fn connect_rejects_invalid_address_format() {
        assert!(
            CustodianClient::connect("not-a-url".to_string())
                .await
                .is_err()
        );
        assert!(AuthClient::connect("not-a-url".to_string()).await.is_err());
        assert!(AdminClient::connect("not-a-url".to_string()).await.is_err());
        assert!(DbClient::connect("not-a-url".to_string()).await.is_err());
    }

    #[tokio::test]
    async fn custodian_wrappers_propagate_transport_errors() {
        let client = test_custodian_client();

        assert!(
            client
                .create_ticket(custodian::CreateTicketRequest {
                    title: "t".to_string(),
                    project: "p".to_string(),
                    account_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    symptom: 0,
                    priority: 0,
                    created_by_uuid: "00000000-0000-0000-0000-000000000002".to_string(),
                    customer_ticket_number: None,
                    isp_ticket_number: None,
                    other_ticket_number: None,
                    ebond: None,
                    tracking_url: None,
                    network_devices: vec![],
                })
                .await
                .is_err()
        );

        assert!(
            client
                .acquire_lock(custodian::LockRequest {
                    ticket_id: 1,
                    user_uuid: "00000000-0000-0000-0000-000000000003".to_string(),
                })
                .await
                .is_err()
        );

        assert!(
            client
                .release_lock(custodian::LockRelease {
                    ticket_id: 1,
                    user_uuid: "00000000-0000-0000-0000-000000000004".to_string(),
                })
                .await
                .is_err()
        );

        assert!(
            client
                .update_ticket(custodian::UpdateTicketRequest {
                    ticket_id: 1,
                    title: None,
                    project: None,
                    symptom: None,
                    priority: None,
                    status: None,
                    next_action: None,
                    resolution: None,
                    assigned_to_uuid: None,
                    updated_by_uuid: Some("00000000-0000-0000-0000-000000000005".to_string()),
                    ebond: None,
                    tracking_url: None,
                    network_devices: vec![],
                })
                .await
                .is_err()
        );

        assert!(
            client
                .get_ticket(custodian::GetTicketRequest { ticket_id: 1 })
                .await
                .is_err()
        );

        assert!(client.cluster_status().await.is_err());
    }
}
