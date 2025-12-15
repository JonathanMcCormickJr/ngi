//! REST API route handlers for LBRP

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::AppState;

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub services: HashMap<String, String>,
}

/// Ticket creation request (REST format)
#[derive(Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub project: String,
    pub account_uuid: String,
    pub symptom: u32,
    pub created_by_uuid: String,
    pub customer_ticket_number: Option<String>,
}

/// Ticket update request (REST format)
#[derive(Deserialize)]
pub struct UpdateTicketRequest {
    pub title: Option<String>,
    pub project: Option<String>,
    pub symptom: Option<u32>,
    pub status: Option<u32>,
    pub next_action: Option<u32>,
    pub resolution: Option<u32>,
    pub assigned_to_uuid: Option<String>,
    pub updated_by_uuid: Option<String>,
}

/// Lock request (REST format)
#[derive(Deserialize)]
pub struct LockRequest {
    pub user_uuid: String,
}

/// Lock response (REST format)
#[derive(Serialize)]
pub struct LockResponse {
    pub success: bool,
    pub error: Option<String>,
    pub current_holder: Option<String>,
}

/// Ticket response (REST format)
#[derive(Serialize)]
pub struct TicketResponse {
    pub ticket_id: u64,
    pub customer_ticket_number: Option<String>,
    pub isp_ticket_number: Option<String>,
    pub other_ticket_number: Option<String>,
    pub title: String,
    pub project: String,
    pub account_uuid: String,
    pub symptom: u32,
    pub status: u32,
    pub next_action: u32,
    pub resolution: Option<u32>,
    pub locked_by_uuid: Option<String>,
    pub assigned_to_uuid: Option<String>,
    pub created_by_uuid: String,
    pub updated_by_uuid: String,
    pub ebond: Option<String>,
    pub tracking_url: Option<String>,
    pub schema_version: u32,
}

/// Cluster status response
#[derive(Serialize)]
pub struct ClusterStatusResponse {
    pub leader_id: String,
    pub follower_ids: Vec<String>,
    pub term: u64,
    pub commit_index: u64,
}

/// Convert protobuf Ticket to REST TicketResponse
impl From<crate::clients::custodian::Ticket> for TicketResponse {
    fn from(ticket: crate::clients::custodian::Ticket) -> Self {
        Self {
            ticket_id: ticket.ticket_id,
            customer_ticket_number: ticket.customer_ticket_number,
            isp_ticket_number: ticket.isp_ticket_number,
            other_ticket_number: ticket.other_ticket_number,
            title: ticket.title,
            project: ticket.project,
            account_uuid: ticket.account_uuid,
            symptom: ticket.symptom as u32,
            status: ticket.status as u32,
            next_action: ticket.next_action as u32,
            resolution: ticket.resolution.map(|r| r as u32),
            locked_by_uuid: ticket.locked_by_uuid,
            assigned_to_uuid: ticket.assigned_to_uuid,
            created_by_uuid: ticket.created_by_uuid,
            updated_by_uuid: ticket.updated_by_uuid,
            ebond: ticket.ebond,
            tracking_url: ticket.tracking_url,
            schema_version: ticket.schema_version,
        }
    }
}

/// Convert protobuf LockResponse to REST LockResponse
impl From<crate::clients::custodian::LockResponse> for LockResponse {
    fn from(resp: crate::clients::custodian::LockResponse) -> Self {
        Self {
            success: resp.success,
            error: if resp.error.is_empty() { None } else { Some(resp.error) },
            current_holder: resp.current_holder,
        }
    }
}

/// Create a new ticket
pub async fn create_ticket(
    State(state): State<AppState>,
    Json(req): Json<CreateTicketRequest>,
) -> impl IntoResponse {
    // Convert REST request to gRPC request
    let grpc_req = crate::clients::custodian::CreateTicketRequest {
        title: req.title,
        project: req.project,
        account_uuid: req.account_uuid,
        symptom: req.symptom as i32,
        created_by_uuid: req.created_by_uuid,
        customer_ticket_number: req.customer_ticket_number,
        isp_ticket_number: None,
        other_ticket_number: None,
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    match state.custodian.create_ticket(grpc_req).await {
        Ok(ticket) => Json(TicketResponse::from(ticket)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to create ticket: {}", e) })),
        ).into_response(),
    }
}

/// Get a ticket by ID
pub async fn get_ticket(
    State(_state): State<AppState>,
    Path(_id): Path<u64>,
) -> impl IntoResponse {
    // TODO: Implement ticket retrieval through DB service
    // For now, return not implemented
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({ "error": "Ticket retrieval not yet implemented" })),
    ).into_response()
}

/// Update an existing ticket
pub async fn update_ticket(
    State(state): State<AppState>,
    Path(ticket_id): Path<u64>,
    Json(req): Json<UpdateTicketRequest>,
) -> impl IntoResponse {
    // Convert REST request to gRPC request
    let grpc_req = crate::clients::custodian::UpdateTicketRequest {
        ticket_id,
        title: req.title,
        project: req.project,
        symptom: req.symptom.map(|s| s as i32),
        status: req.status.map(|s| s as i32),
        next_action: req.next_action.map(|a| a as i32),
        resolution: req.resolution.map(|r| r as i32),
        assigned_to_uuid: req.assigned_to_uuid,
        updated_by_uuid: req.updated_by_uuid,
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    match state.custodian.update_ticket(grpc_req).await {
        Ok(ticket) => Json(TicketResponse::from(ticket)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to update ticket: {}", e) })),
        ).into_response(),
    }
}

/// Acquire a lock on a ticket
pub async fn acquire_lock(
    State(state): State<AppState>,
    Path(ticket_id): Path<u64>,
    Json(req): Json<LockRequest>,
) -> impl IntoResponse {
    let grpc_req = crate::clients::custodian::LockRequest {
        ticket_id,
        user_uuid: req.user_uuid,
    };

    match state.custodian.acquire_lock(grpc_req).await {
        Ok(response) => Json(LockResponse::from(response)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to acquire lock: {}", e) })),
        ).into_response(),
    }
}

/// Release a lock on a ticket
pub async fn release_lock(
    State(state): State<AppState>,
    Path(ticket_id): Path<u64>,
    Json(req): Json<LockRequest>,
) -> impl IntoResponse {
    let grpc_req = crate::clients::custodian::LockRelease {
        ticket_id,
        user_uuid: req.user_uuid,
    };

    match state.custodian.release_lock(grpc_req).await {
        Ok(response) => Json(LockResponse::from(response)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to release lock: {}", e) })),
        ).into_response(),
    }
}

/// Get cluster status
pub async fn cluster_status(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.custodian.cluster_status().await {
        Ok(status) => {
            let response = ClusterStatusResponse {
                leader_id: status.leader_id,
                follower_ids: status.follower_ids,
                term: status.term,
                commit_index: status.commit_index,
            };
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to get cluster status: {}", e) })),
        ).into_response(),
    }
}

/// Get Prometheus metrics
pub async fn metrics(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // TODO: Implement metrics scraping from admin service
    // For now, return a simple response
    "This would serve Prometheus metrics from the admin service".into_response()
}