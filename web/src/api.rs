use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

const TOKEN_KEY: &str = "ngi_demo_token";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ticket {
    pub ticket_id: u64,
    pub title: String,
    pub project: String,
    pub priority: i32,
    pub status: i32,
}

#[derive(Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub mfa_token: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub project: String,
    pub account_uuid: String,
    pub symptom: i32,
    pub priority: i32,
}

#[derive(Serialize)]
pub struct UpdateTicketRequest {
    pub title: Option<String>,
    pub project: Option<String>,
    pub priority: Option<i32>,
    pub status: Option<i32>,
}

#[derive(Serialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    pub display_name: String,
    pub role: i32,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn get_token() -> Option<String> {
    storage()?.get_item(TOKEN_KEY).ok()?
}

pub fn clear_token() {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(TOKEN_KEY);
    }
}

fn set_token(token: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(TOKEN_KEY, token);
    }
}

pub async fn login(username: String, password: String) -> Result<(), String> {
    let payload = LoginRequest {
        username,
        password,
        mfa_token: None,
    };

    let response = Request::post("/auth/login")
        .json(&payload)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Login failed".to_string()));
    }

    let login_response: LoginResponse = response.json().await.map_err(|e| e.to_string())?;
    set_token(&login_response.token);
    Ok(())
}

pub async fn create_user(token: &str, payload: &CreateUserRequest) -> Result<(), String> {
    let response = Request::post("/api/admin/users")
        .header("Authorization", &format!("Bearer {token}"))
        .json(payload)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.ok() {
        Ok(())
    } else {
        Err(response
            .text()
            .await
            .unwrap_or_else(|_| "User creation failed".to_string()))
    }
}

pub async fn create_ticket(token: &str, payload: &CreateTicketRequest) -> Result<Ticket, String> {
    let response = Request::post("/api/tickets")
        .header("Authorization", &format!("Bearer {token}"))
        .json(payload)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Ticket creation failed".to_string()));
    }

    response.json().await.map_err(|e| e.to_string())
}

pub async fn fetch_ticket(token: &str, ticket_id: u64) -> Result<Ticket, String> {
    let response = Request::get(&format!("/api/tickets/{ticket_id}"))
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Ticket lookup failed".to_string()));
    }

    response.json().await.map_err(|e| e.to_string())
}

pub async fn update_ticket(
    token: &str,
    ticket_id: u64,
    payload: &UpdateTicketRequest,
) -> Result<Ticket, String> {
    let response = Request::put(&format!("/api/tickets/{ticket_id}"))
        .header("Authorization", &format!("Bearer {token}"))
        .json(payload)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Ticket update failed".to_string()));
    }

    response.json().await.map_err(|e| e.to_string())
}
