//! REST API route handlers for LBRP

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
    middleware,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::clients::{AuthClient, AdminClient, CustodianClient};
use crate::middleware::{auth_middleware, AuthState, Claims};

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

    let resp = client.authenticate(req).await
        .map_err(|e| {
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

    let _resp = client.create_user(req).await
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
}

async fn create_ticket(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateTicketRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut client = state.custodian_client.client.lock().await;
    
    let req = crate::clients::custodian::CreateTicketRequest {
        title: payload.title,
        project: payload.project,
        account_uuid: payload.account_uuid,
        symptom: payload.symptom,
        created_by_uuid: claims.sub,
        customer_ticket_number: None,
        isp_ticket_number: None,
        other_ticket_number: None,
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    let _resp = client.create_ticket(req).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.message().to_string()))?;
        
    Ok(StatusCode::CREATED)
}

async fn get_ticket(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut client = state.custodian_client.client.lock().await;
    
    let req = crate::clients::custodian::GetTicketRequest { ticket_id: id };

    let _resp = client.get_ticket(req).await
        .map_err(|e| (StatusCode::NOT_FOUND, e.message().to_string()))?;
        
    // TODO: Map protobuf Ticket to JSON
    Ok(StatusCode::OK)
}

pub fn app(state: AppState) -> Router {
    let auth_routes = Router::new()
        .route("/login", post(login));

    let api_routes = Router::new()
        .route("/admin/users", post(create_user))
        .route("/tickets", post(create_ticket))
        .route("/tickets/{id}", get(get_ticket))
        .layer(middleware::from_fn_with_state(
            state.auth_state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/api", api_routes)
        .with_state(state)
}
