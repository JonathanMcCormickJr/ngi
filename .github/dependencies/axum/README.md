# Axum - Web Framework for NGI

> Modular web framework built with Tokio, focused on ergonomics and modularity.

**Official Docs:** https://docs.rs/axum/latest/axum/

**Current Version:** Latest

## Overview

Axum provides NGI's REST/JSON API layer exclusively through the LBRP (Load Balancer & Reverse Proxy) service. It handles HTTP requests, routing, middleware, and response serialization.

## Documentation Index

- **[framework.md](framework.md)** - Complete Axum framework guide
  - Basic handler patterns (GET, POST, PUT, DELETE)
  - Router setup and routing
  - Error handling and custom responses
  - Middleware and request logging
  - Content negotiation
  - CORS configuration
  - Health checks
  - NGI LBRP service architecture
  - Best practices for LBRP
  - Official API documentation

## Architecture in NGI

### Service Boundary
```
Internet/Client (JSON over HTTPS)
    ↓
Axum Router (REST API, LBRP Service)
    ├── GET /api/ticket/:id
    ├── POST /api/ticket (create)
    ├── PATCH /api/ticket/:id (update)
    ├── DELETE /api/ticket/:id (soft delete)
    └── WebSocket /api/ticket/:id/events
    ↓
Internal Services (gRPC over mTLS)
    ├── DB Service (read/write state)
    ├── Custodian Service (lock operations)
    └── Auth Service (validation)
```

## Core Components

### Basic Router Setup
```rust
use axum::{routing::{get, post, patch}, Router, Json, extract::Path};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct TicketResponse {
    pub id: u64,
    pub title: String,
    pub status: String,
}

async fn create_ticket(
    Json(req): Json<CreateTicketRequest>,
) -> Json<TicketResponse> {
    // Call custodian/db gRPC service
    let ticket = create_ticket_internal(&req).await;
    
    Json(TicketResponse {
        id: ticket.id,
        title: ticket.title,
        status: format!("{:?}", ticket.status),
    })
}

async fn get_ticket(Path(id): Path<u64>) -> Json<TicketResponse> {
    let ticket = fetch_ticket(id).await;
    Json(TicketResponse { /* ... */ })
}

async fn update_ticket(
    Path(id): Path<u64>,
    Json(req): Json<UpdateTicketRequest>,
) -> Json<TicketResponse> {
    let ticket = update_ticket_internal(id, &req).await;
    Json(TicketResponse { /* ... */ })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/ticket", post(create_ticket))
        .route("/api/ticket/:id", get(get_ticket).patch(update_ticket))
        .layer(middleware::trace_layer())
        .layer(middleware::auth_layer());
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:443").await?;
    axum::serve(listener, app).await?;
}
```

### Path Parameters & Extractors
```rust
use axum::extract::{Path, Query};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListTicketsQuery {
    pub status: Option<String>,
    pub limit: Option<u64>,
}

async fn list_tickets(
    Query(params): Query<ListTicketsQuery>,
) -> Json<Vec<TicketResponse>> {
    let status = params.status.as_deref().unwrap_or("open");
    let limit = params.limit.unwrap_or(50);
    
    let tickets = fetch_tickets(status, limit).await;
    Json(tickets)
}

// Called with: GET /api/tickets?status=open&limit=100
```

### JSON Request/Response
```rust
use axum::Json;

async fn create_user(Json(req): Json<CreateUserRequest>) -> Json<UserResponse> {
    // req is automatically deserialized from JSON request body
    let user = create_user_internal(&req).await;
    Json(user)  // Automatically serialized to JSON response
}
```

### State Management
```rust
use axum::extract::State;
use std::sync::Arc;

pub struct AppState {
    pub custodian_client: Arc<CustodianClient>,
    pub db_client: Arc<DbClient>,
}

async fn create_ticket(
    State(state): State<AppState>,
    Json(req): Json<CreateTicketRequest>,
) -> Json<TicketResponse> {
    // Use clients from state
    let ticket = state.custodian_client.create_ticket(req).await?;
    Json(ticket)
}

// In main:
let state = AppState {
    custodian_client: Arc::new(create_custodian_client().await?),
    db_client: Arc::new(create_db_client().await?),
};

let app = Router::new()
    .route("/api/ticket", post(create_ticket))
    .with_state(state);
```

## Middleware

### Authentication Middleware
```rust
use axum::middleware::{self, Next};
use axum::http::Request;

pub async fn auth_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let user_id = validate_token(auth_header)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    
    request.extensions_mut().insert(user_id);
    Ok(next.run(request).await)
}

// Use in router:
let app = Router::new()
    .route("/api/ticket", post(create_ticket))
    .route_layer(middleware::from_fn(auth_middleware));
```

### Logging Middleware
```rust
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/api/ticket/:id", get(get_ticket))
    .layer(TraceLayer::new_for_http());
    
// Logs all requests/responses
```

### CORS Middleware
```rust
use tower_http::cors::{CorsLayer, Any};

let cors = CorsLayer::permissive();  // Allow all origins (dev only!)

let app = Router::new()
    .route("/api/ticket/:id", get(get_ticket))
    .layer(cors);
```

## Error Handling

### Custom Error Response
```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub enum ApiError {
    NotFound(String),
    Unauthorized,
    AlreadyLocked(String),
    InvalidInput(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::AlreadyLocked(msg) => (StatusCode::CONFLICT, msg),
            ApiError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
        };
        
        let body = Json(json!({
            "error": message,
            "status": status.as_u16(),
        }));
        
        (status, body).into_response()
    }
}

// Handler usage:
async fn get_ticket(Path(id): Path<u64>) -> Result<Json<TicketResponse>, ApiError> {
    let ticket = fetch_ticket(id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Ticket {} not found", id)))?;
    Ok(Json(ticket))
}
```

## WebSocket Support

```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use futures::stream::{StreamExt, SinkExt};

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Process ticket subscription: "subscribe:ticket:42"
                let response = handle_subscription(&text).await;
                if socket.send(Message::Text(response)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

// In router:
Router::new()
    .route("/api/ticket/:id/events", get(websocket_handler))
```

## Nested Routing

```rust
async fn ticket_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_ticket))
        .route("/:id", get(get_ticket).patch(update_ticket))
        .route("/:id/lock", post(acquire_lock).delete(release_lock))
}

async fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users).post(create_user))
        .route("/:id", get(get_user).patch(update_user))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .nest("/api/ticket", ticket_routes().await)
        .nest("/api/user", user_routes().await)
        .route("/health", get(|| async { "OK" }));
}
```

## Response Types

### JSON Response
```rust
async fn get_ticket(id: u64) -> Json<TicketResponse> {
    Json(TicketResponse { /* ... */ })
}
```

### Custom Status Code
```rust
async fn create_ticket(req: CreateTicketRequest) -> (StatusCode, Json<TicketResponse>) {
    let ticket = create_internal(req).await;
    (StatusCode::CREATED, Json(ticket))
}
```

### Streaming Response
```rust
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream};

async fn stream_events() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::iter(vec![
        Ok(Event::default().data("ticket created")),
        Ok(Event::default().data("ticket updated")),
    ]);
    Sse::new(stream).keep_alive(KeepAlive::default())
}
```

## Testing

```rust
#[tokio::test]
async fn test_get_ticket() {
    let app = create_app().await;
    
    let response = app
        .oneshot(Request::builder().uri("/api/ticket/1").build().unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let ticket: TicketResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(ticket.id, 1);
}
```

## NGI REST API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/ticket` | GET | List tickets with filters |
| `/api/ticket` | POST | Create new ticket |
| `/api/ticket/:id` | GET | Get ticket details |
| `/api/ticket/:id` | PATCH | Update ticket |
| `/api/ticket/:id` | DELETE | Soft delete ticket |
| `/api/ticket/:id/lock` | POST | Acquire exclusive lock |
| `/api/ticket/:id/lock` | DELETE | Release lock |
| `/api/ticket/:id/events` | WebSocket | Stream live events |
| `/api/user` | GET/POST | User management |
| `/health` | GET | Health check |

## Configuration

```toml
# Cargo.toml
[dependencies]
axum = { version = "0.7", features = ["macros", "ws"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["trace", "cors", "sse"] }
```

## References

- **Official Modules:**
  - [routing](https://docs.rs/axum/latest/axum/routing/) - Route definition
  - [extract](https://docs.rs/axum/latest/axum/extract/) - Request extraction
  - [response](https://docs.rs/axum/latest/axum/response/) - Response types
  - [middleware](https://docs.rs/axum/latest/axum/middleware/) - Middleware system

- **NGI Service:**
  - [lbrp/](../../../lbrp/) - Load Balancer implementation

---

**Last Updated:** December 2025  
**Documentation Version:** Axum Latest
