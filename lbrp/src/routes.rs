//! REST API route handlers for LBRP

use crate::clients::{AdminClient, AuthClient, CustodianClient};
use crate::middleware::{AuthState, Claims, auth_middleware};
use axum::{
    Extension, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub auth_client: AuthClient,
    pub admin_client: AdminClient,
    pub custodian_client: CustodianClient,
    pub auth_state: Arc<AuthState>,
}

// --- Auth Handlers ---

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub mfa_token: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct ApiTicket {
    pub ticket_id: u64,
    pub title: String,
    pub project: String,
    pub priority: i32,
    pub status: i32,
}

fn map_ticket(ticket: crate::clients::custodian::Ticket) -> ApiTicket {
    ApiTicket {
        ticket_id: ticket.ticket_id,
        title: ticket.title,
        project: ticket.project,
        priority: ticket.priority,
        status: ticket.status,
    }
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut client = state.auth_client.client.lock().await;

    let req = crate::clients::auth::AuthenticateRequest {
        username: payload.username,
        password: payload.password,
        mfa_token: payload.mfa_token.unwrap_or_default(),
    };

    let resp = client.authenticate(req).await.map_err(|e| {
        tracing::error!("Auth service error: {}", e);
        match e.code() {
            tonic::Code::Unauthenticated => (StatusCode::UNAUTHORIZED, e.message().to_string()),
            tonic::Code::InvalidArgument => (StatusCode::BAD_REQUEST, e.message().to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, e.message().to_string()),
        }
    })?;

    let resp_inner = resp.into_inner();
    if resp_inner.success {
        Ok(Json(LoginResponse {
            token: resp_inner.session_token,
        }))
    } else {
        Err((StatusCode::UNAUTHORIZED, resp_inner.error))
    }
}

// --- Admin Handlers ---

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    pub display_name: String,
    pub role: i32,
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut client = state.admin_client.client.lock().await;

    let req = crate::clients::admin::CreateUserRequest {
        username: payload.username,
        password: payload.password,
        email: payload.email,
        display_name: payload.display_name,
        role: payload.role,
    };

    let _resp = client
        .create_user(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.message().to_string()))?;

    Ok(StatusCode::CREATED)
}

// --- Custodian Handlers ---

#[derive(Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub project: String,
    pub account_uuid: String,
    pub symptom: i32,
    pub priority: i32,
}

async fn create_ticket(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateTicketRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let req = crate::clients::custodian::CreateTicketRequest {
        title: payload.title,
        project: payload.project,
        account_uuid: payload.account_uuid,
        symptom: payload.symptom,
        priority: payload.priority,
        created_by_uuid: claims.sub,
        customer_ticket_number: None,
        isp_ticket_number: None,
        other_ticket_number: None,
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    let resp = state
        .custodian_client
        .create_ticket(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(map_ticket(resp))))
}

async fn get_ticket(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let req = crate::clients::custodian::GetTicketRequest { ticket_id: id };

    let resp = state
        .custodian_client
        .get_ticket(req)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    Ok(Json(map_ticket(resp)))
}

#[derive(Deserialize)]
pub struct UpdateTicketRequest {
    pub title: Option<String>,
    pub project: Option<String>,
    pub priority: Option<i32>,
    pub status: Option<i32>,
}

async fn update_ticket(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UpdateTicketRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let req = crate::clients::custodian::UpdateTicketRequest {
        ticket_id: id,
        title: payload.title,
        project: payload.project,
        symptom: None,
        priority: payload.priority,
        status: payload.status,
        next_action: None,
        resolution: None,
        assigned_to_uuid: None,
        updated_by_uuid: Some(claims.sub),
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    let resp = state
        .custodian_client
        .update_ticket(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(map_ticket(resp)))
}

pub fn app(state: AppState) -> Router {
    let auth_routes = Router::new().route("/login", post(login));

    let api_routes = Router::new()
        .route("/admin/users", post(create_user))
        .route("/tickets", post(create_ticket))
        .route("/tickets/{id}", get(get_ticket).put(update_ticket))
        .layer(middleware::from_fn_with_state(
            state.auth_state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/api", api_routes)
        .with_state(state)
}
