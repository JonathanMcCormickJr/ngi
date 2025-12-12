# Axum

**Repository:** https://github.com/tokio-rs/axum  
**Documentation:** https://docs.rs/axum/  
**Crates.io:** https://crates.io/crates/axum

## Overview
Axum is a modular web framework for building REST APIs. **NGI uses Axum exclusively in the LBRP (Load Balancer Reverse Proxy) service** for HTTP/REST endpoints that expose the system to clients. All inter-service communication uses gRPC via Tonic.

## Architecture

The LBRP service serves as the gateway:
1. Receives HTTP/REST requests from clients
2. Routes to appropriate gRPC backend service
3. Translates between HTTP/JSON and gRPC/binary
4. Handles load balancing across service instances
5. Provides publicly accessible endpoints

## Basic Handler Pattern

```rust
use axum::{
    routing::{get, post, put, delete},
    Router, Json, extract::Path,
    http::StatusCode,
};

// Simple GET handler
async fn get_ticket(Path(id): Path<u64>) -> Json<Ticket> {
    let ticket = custodian_client.get_ticket(id).await?;
    Json(ticket)
}

// POST handler with status code
async fn create_ticket(
    Json(req): Json<CreateTicketRequest>,
) -> (StatusCode, Json<Ticket>) {
    let ticket = custodian_client.create_ticket(req).await?;
    (StatusCode::CREATED, Json(ticket))
}

// PUT handler
async fn update_ticket(
    Path(id): Path<u64>,
    Json(req): Json<UpdateTicketRequest>,
) -> Json<Ticket> {
    req.id = id;
    let ticket = custodian_client.update_ticket(req).await?;
    Json(ticket)
}

// DELETE handler
async fn delete_ticket(Path(id): Path<u64>) -> StatusCode {
    custodian_client.soft_delete_ticket(id).await?;
    StatusCode::NO_CONTENT
}
```

## Router Setup

```rust
let app = Router::new()
    .route("/api/ticket/:id", get(get_ticket))
    .route("/api/ticket", post(create_ticket))
    .route("/api/ticket/:id", put(update_ticket))
    .route("/api/ticket/:id", delete(delete_ticket))
    .route("/health", get(health_check))
    .route("/metrics", get(metrics))
    .layer(TraceLayer::new_for_http())
    .layer(CorsLayer::permissive());
```

## Error Handling

```rust
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
};

#[async_trait]
impl<S> FromRequest<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(
        req: Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let user_id = verify_session(&req).await
            .map_err(|e| (
                StatusCode::UNAUTHORIZED,
                format!("Auth failed: {}", e),
            ).into_response())?;
        Ok(user_id)
    }
}
```

## Middleware

```rust
use tower_http::trace::TraceLayer;
use tower_http::cors::CorsLayer;

let app = Router::new()
    .route("/api/*path", get(handler))
    .layer(TraceLayer::new_for_http())      // Request tracing
    .layer(CorsLayer::permissive())         // CORS handling
    .layer(Extension(db_client))             // Shared state
    .with_state(AppState::default());
```

## NGI Integration Examples

### Authentication Service Endpoint

```rust
#[derive(Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
    pub totp: Option<String>,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub session_token: String,
    pub expires_at: SystemTime,
}

async fn authenticate(
    Json(req): Json<AuthRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), AppError> {
    let response = auth_client.authenticate(req).await?;
    Ok((StatusCode::OK, Json(response)))
}
```

### Ticket Management Endpoints

```rust
// List tickets with filtering
async fn list_tickets(
    Query(filter): Query<TicketFilter>,
) -> Result<Json<Vec<Ticket>>, AppError> {
    let tickets = custodian_client.list_tickets(filter).await?;
    Ok(Json(tickets))
}

// Acquire ticket lock (for editing)
async fn lock_ticket(
    Path(id): Path<u64>,
    user_id: UserId,
) -> Result<StatusCode, AppError> {
    custodian_client.acquire_lock(id, user_id).await?;
    Ok(StatusCode::OK)
}

// Release ticket lock
async fn release_ticket(
    Path(id): Path<u64>,
    user_id: UserId,
) -> Result<StatusCode, AppError> {
    custodian_client.release_lock(id, user_id).await?;
    Ok(StatusCode::OK)
}
```

## Best Practices for NGI

1. **Use extractors for type safety** - Avoid manual request parsing
2. **Implement consistent error handling** - All errors should return proper HTTP status codes
3. **Add request tracing** - Use TraceLayer for distributed tracing
4. **Validate input at handler level** - Use serde validation or manual checks
5. **Keep handlers thin** - Delegate business logic to gRPC clients
6. **Use middleware for cross-cutting concerns** - Auth, logging, metrics
7. **Implement graceful shutdown** - Allow in-flight requests to complete

## Performance Considerations

- Axum is built on `tower-service` trait - extremely efficient
- Handlers are zero-copy where possible
- Use streaming for large payloads
- Connection pooling for gRPC clients essential
- Enable HTTP/2 for better performance

## Testing

```rust
#[cfg(test)]
mod tests {
    use axum_test_helper::TestClient;

    #[tokio::test]
    async fn test_get_ticket() {
        let app = create_router();
        let client = TestClient::new(app);
        
        let res = client.get("/api/ticket/1").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
```

## See Also

- [Tokio Runtime](../tokio/README.md) - Async runtime used by Axum
- [Tonic for Service Communication](../tonic/README.md) - Backend gRPC communication
- [Error Handling Pattern](../_concepts/error-handling.md)
