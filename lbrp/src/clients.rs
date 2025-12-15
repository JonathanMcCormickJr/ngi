//! gRPC client implementations for LBRP service communication

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

// Include generated protobuf code
pub mod custodian {
    tonic::include_proto!("custodian");
}

pub mod db {
    tonic::include_proto!("db");
}

/// Custodian service client
#[derive(Clone)]
pub struct CustodianClient {
    client: Arc<Mutex<custodian::custodian_service_client::CustodianServiceClient<Channel>>>,
}

impl CustodianClient {
    pub async fn connect(addr: String) -> Result<Self> {
        let channel = Channel::from_shared(addr)?
            .connect()
            .await?;
        let client = custodian::custodian_service_client::CustodianServiceClient::new(channel);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub async fn create_ticket(&self, req: custodian::CreateTicketRequest) -> Result<custodian::Ticket> {
        let mut client = self.client.lock().await;
        let response = client.create_ticket(req).await?;
        Ok(response.into_inner())
    }

    pub async fn acquire_lock(&self, req: custodian::LockRequest) -> Result<custodian::LockResponse> {
        let mut client = self.client.lock().await;
        let response = client.acquire_lock(req).await?;
        Ok(response.into_inner())
    }

    pub async fn release_lock(&self, req: custodian::LockRelease) -> Result<custodian::LockResponse> {
        let mut client = self.client.lock().await;
        let response = client.release_lock(req).await?;
        Ok(response.into_inner())
    }

    pub async fn update_ticket(&self, req: custodian::UpdateTicketRequest) -> Result<custodian::Ticket> {
        let mut client = self.client.lock().await;
        let response = client.update_ticket(req).await?;
        Ok(response.into_inner())
    }

    pub async fn cluster_status(&self) -> Result<custodian::ClusterStatusResponse> {
        let mut client = self.client.lock().await;
        let response = client.cluster_status(custodian::ClusterStatusRequest {}).await?;
        Ok(response.into_inner())
    }
}

/// DB service client
#[derive(Clone)]
pub struct DbClient {
    client: Arc<Mutex<db::database_client::DatabaseClient<Channel>>>,
}

impl DbClient {
    pub async fn connect(addr: String) -> Result<Self> {
        let channel = Channel::from_shared(addr)?
            .connect()
            .await?;
        let client = db::database_client::DatabaseClient::new(channel);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    // Add DB client methods as needed
}