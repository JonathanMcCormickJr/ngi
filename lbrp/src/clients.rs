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
