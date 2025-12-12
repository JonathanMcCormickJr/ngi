# Tonic - gRPC Framework for NGI

> A native Rust gRPC implementation with async/await and HTTP/2.

**Official Docs:** https://docs.rs/tonic/latest/tonic/

**Current Version:** 0.14.0+

## Overview

Tonic powers all inter-service communication in NGI using gRPC with protocol buffers and mTLS for security. It provides efficient, strongly-typed RPC interfaces across microservices.

## Architecture in NGI

### gRPC Services
```
client (HTTP/2 + mTLS) ──→ server
      ↓                      ↓
   Tonic Client          Tonic Server
      ↓                      ↓
   request/response streaming (async)
```

### NGI Services Using Tonic
- **DB Service** - Raft log operations, state queries
- **Custodian Service** - Lock acquire/release RPCs
- **Auth Service** - Session validation
- **Admin Service** - User management APIs

## Documentation Index

- **[framework.md](framework.md)** - Complete Tonic framework guide
  - Protocol Buffers (Protobuf) definition
  - Server implementation patterns
  - Client usage and configuration
  - Streaming (server, client, bidirectional)
  - Error handling with Status codes
  - Metadata and headers
  - Interceptors (middleware)
  - NGI service ports and best practices
  - Official API documentation

## Architecture in NGI

### gRPC Services
```
client (HTTP/2 + mTLS) ──→ server
      ↓                      ↓
   Tonic Client          Tonic Server
      ↓                      ↓
   request/response streaming (async)
```

### NGI Services Using Tonic
- **DB Service** - Raft log operations, state queries
- **Custodian Service** - Lock acquire/release RPCs
- **Auth Service** - Session validation
- **Admin Service** - User management APIs

## Core Components

### Server-Side (tonic::server)

```rust
use tonic::{transport::Server, Request, Response, Status};
use db::db_server::{Db, DbServer};

pub struct DbServiceImpl;

#[tonic::async_trait]
impl Db for DbServiceImpl {
    async fn get_ticket(
        &self,
        request: Request<GetTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();
        let ticket = fetch_ticket(req.id).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ticket))
    }
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:8080".parse()?;
    Server::builder()
        .add_service(DbServer::new(DbServiceImpl))
        .serve(addr)
        .await?;
}
```

### Client-Side (tonic::client)

```rust
use tonic::transport::Channel;
use db::db_client::DbClient;

async fn query_ticket(id: u64) -> Result<Ticket> {
    // Connect to DB service
    let channel = Channel::from_static("http://db:8080")
        .connect()
        .await?;
    
    let mut client = DbClient::new(channel);
    
    // Make RPC call
    let request = GetTicketRequest { id };
    let response = client.get_ticket(request).await?;
    
    Ok(response.into_inner())
}
```

## mTLS Configuration (Security Layer 1)

### Server-Side mTLS
```rust
use tonic::transport::Identity;
use std::fs;

let cert = fs::read("cert.pem")?;
let key = fs::read("key.pem")?;
let identity = Identity::from_pem(cert, key);

Server::builder()
    .tls_config(ServerTlsConfig::new().identity(identity))?
    .add_service(DbServer::new(DbServiceImpl))
    .serve(addr)
    .await?;
```

### Client-Side mTLS
```rust
use tonic::transport::ClientTlsConfig;
use std::fs;

let cert = fs::read("ca.pem")?;
let ca = Certificate::from_pem(cert);

let tls = ClientTlsConfig::new()
    .ca_certificate(ca)
    .identity(Identity::from_pem(client_cert, client_key));

let channel = Channel::from_static("https://db:8080")
    .tls_config(tls)?
    .connect()
    .await?;
```

## Proto Definition Example

```protobuf
// db.proto
syntax = "proto3";
package db;

service Db {
    rpc GetTicket (GetTicketRequest) returns (Ticket);
    rpc UpdateTicket (UpdateRequest) returns (Ticket);
    rpc StreamTickets (StreamRequest) returns (stream Ticket);
}

message GetTicketRequest {
    uint64 id = 1;
}

message Ticket {
    uint64 id = 1;
    string title = 2;
    int32 status = 3;  // 0=Open, 1=Assigned, etc.
}
```

## Build Integration (build.rs)

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/db.proto")?;
    Ok(())
}

// Cargo.toml
[build-dependencies]
tonic-build = "0.14"
```

## Streaming RPCs

### Server-Side Streaming
```rust
async fn stream_tickets(
    &self,
    request: Request<StreamRequest>,
) -> Result<Response<impl Stream<Item = Result<Ticket, Status>>>, Status> {
    let req = request.into_inner();
    let stream = stream_tickets_from_db(req.status)
        .await
        .map(|ticket| Ok(ticket))
        .map_err(|e| Status::internal(e.to_string()));
    Ok(Response::new(stream))
}
```

### Client-Side Streaming Consumption
```rust
let mut stream = client.stream_tickets(request).await?.into_inner();
while let Some(ticket) = stream.message().await? {
    process_ticket(ticket);
}
```

## Metadata Handling (Headers)

```rust
// Server: Extract metadata
async fn get_ticket(
    &self,
    request: Request<GetTicketRequest>,
) -> Result<Response<Ticket>, Status> {
    let metadata = request.metadata();
    let user_id = metadata
        .get("user-id")
        .and_then(|h| h.to_str().ok())?;
    
    // Authenticate user...
    Ok(Response::new(ticket))
}

// Client: Add metadata
let mut request = GetTicketRequest { id: 42 };
request.set_header("user-id", "123");
client.get_ticket(request).await?;
```

## Error Handling

```rust
use tonic::Status;

// Map domain errors to gRPC Status
match db.get_ticket(id).await {
    Ok(ticket) => Ok(Response::new(ticket)),
    Err(TicketError::NotFound) => {
        Err(Status::not_found("ticket not found"))
    }
    Err(TicketError::PermissionDenied) => {
        Err(Status::permission_denied("access denied"))
    }
    Err(e) => Err(Status::internal(e.to_string())),
}
```

## NGI-Specific Patterns

### Leader-Aware Routing (LBRP)
```rust
// LBRP identifies leader for Raft services
async fn get_db_leader() -> Result<String> {
    for instance in &DB_INSTANCES {
        let channel = Channel::from_static(instance).connect().await?;
        let mut client = DbClient::new(channel);
        if client.is_leader(Empty {}).await.is_ok() {
            return Ok(instance.to_string());
        }
    }
    Err("No leader elected")
}
```

### Inter-Service Call Pattern
```rust
pub async fn get_user_profile(user_id: u64) -> Result<UserProfile> {
    // 1. Connect to Admin service
    let channel = Channel::from_static("https://admin:8083")
        .tls_config(get_mTls_config())?
        .connect()
        .await?;
    
    // 2. Create client
    let mut client = AdminClient::new(channel);
    
    // 3. Call RPC with timeout
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        client.get_user(GetUserRequest { id: user_id })
    ).await??;
    
    Ok(response.into_inner())
}
```

## Configuration & Tuning

### Server Builder Options
```rust
Server::builder()
    .http2_keep_alive_interval(Some(Duration::from_secs(30)))
    .http2_keep_alive_timeout(Some(Duration::from_secs(5)))
    .max_concurrent_streams(Some(1000))
    .add_service(DbServer::new(impl_service))
    .serve(addr)
    .await?;
```

### Connection Management
```rust
let channel = Channel::from_static("https://db:8080")
    .http2_keep_alive_interval(Duration::from_secs(30))
    .http2_keep_alive_timeout(Duration::from_secs(5))
    .connect()
    .await?;
```

## Testing Patterns

```rust
#[tokio::test]
async fn test_get_ticket_rpc() {
    // Create mock implementation
    let service = DbServer::new(MockDbImpl);
    
    // Start server
    let addr = "127.0.0.1:50051".parse()?;
    tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve(addr)
            .await
    });
    
    // Test client call
    let channel = Channel::from_static("http://127.0.0.1:50051")
        .connect()
        .await?;
    let mut client = DbClient::new(channel);
    let response = client.get_ticket(GetTicketRequest { id: 1 }).await?;
    assert_eq!(response.get_ref().id, 1);
}
```

## References

- **Official Modules:**
  - [transport](https://docs.rs/tonic/latest/tonic/transport/) - Connection setup
  - [server](https://docs.rs/tonic/latest/tonic/server/) - Server implementation
  - [client](https://docs.rs/tonic/latest/tonic/client/) - Client creation
  - [metadata](https://docs.rs/tonic/latest/tonic/metadata/) - Headers/metadata

- **NGI Services:**
  - [db/proto/db.proto](../../../db/proto/db.proto) - Database service RPC
  - [custodian](../../../custodian/) - Lock service RPC

---

**Last Updated:** December 2025  
**Documentation Version:** Tonic 0.14.0
