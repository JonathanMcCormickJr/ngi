# Tonic 0.14

**Repository:** https://github.com/hyperium/tonic  
**Documentation:** https://docs.rs/tonic/  
**Crates.io:** https://crates.io/crates/tonic

## Related Packages in NGI
```toml
tonic = "0.14"
prost = "0.14"
tonic-prost = "0.14"

[build-dependencies]
tonic-prost-build = "0.14"
```

## Overview
Tonic is a gRPC framework for Rust built on top of Hyper HTTP/2. It provides type-safe, high-performance service-to-service communication with streaming support. All inter-service communication in NGI uses tonic/gRPC.

## Key Concepts

### Protocol Buffers (Protobuf)
Define service contracts in `.proto` files:

```protobuf
syntax = "proto3";

package db;

message Ticket {
    uint64 id = 1;
    string title = 2;
    string status = 3;
}

message GetTicketRequest {
    uint64 id = 1;
}

service Db {
    rpc GetTicket(GetTicketRequest) returns (Ticket);
    rpc UpdateTicket(Ticket) returns (Ticket);
}
```

### Code Generation
Build script automatically generates Rust types and service traits:

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/db.proto")?;
    Ok(())
}
```

## Server Implementation

```rust
use tonic::{Request, Response, Status, transport::Server};

// Generated service trait
#[tonic::async_trait]
impl db_server::Db for DbServiceImpl {
    async fn get_ticket(
        &self,
        request: Request<GetTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();
        
        // Process request
        let ticket = self.db.get_ticket(req.id)
            .await
            .map_err(|e| Status::internal(format!("failed: {}", e)))?;
        
        Ok(Response::new(ticket))
    }
}

// Service startup
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:8080".parse()?;
    let db_service = DbServiceImpl::new(/* ... */);
    
    Server::builder()
        .add_service(DbServer::new(db_service))
        .serve(addr)
        .await?;
    
    Ok(())
}
```

## Client Usage

```rust
use tonic::transport::Channel;
use db::db_client::DbClient;

async fn fetch_ticket(id: u64) -> Result<Ticket, Box<dyn std::error::Error>> {
    // Create channel to service
    let channel = Channel::from_static("http://db-leader:8080")
        .connect()
        .await?;
    
    // Create client
    let mut client = DbClient::new(channel);
    
    // Make request
    let request = Request::new(GetTicketRequest { id });
    let response = client.get_ticket(request).await?;
    
    Ok(response.into_inner())
}
```

## Streaming

### Server Streaming
Server sends multiple responses for one request:

```protobuf
service Db {
    rpc StreamTickets(StreamRequest) returns (stream Ticket);
}
```

```rust
async fn stream_tickets(
    &self,
    request: Request<StreamRequest>,
) -> Result<Response<impl Stream<Item = Result<Ticket, Status>>>, Status> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    
    let db = self.db.clone();
    tokio::spawn(async move {
        for ticket in db.get_all_tickets().await {
            let _ = tx.send(Ok(ticket)).await;
        }
    });
    
    Ok(Response::new(ReceiverStream::new(rx)))
}
```

### Client Streaming
Client sends multiple requests, server sends one response:

```protobuf
service Db {
    rpc UpdateTickets(stream UpdateRequest) returns (Response);
}
```

### Bidirectional Streaming
Both sides can send multiple messages:

```protobuf
service Db {
    rpc SyncTickets(stream Ticket) returns (stream Ticket);
}
```

## Error Handling

gRPC status codes map to HTTP status codes:

```rust
// Status codes
Status::ok() -> 200
Status::invalid_argument() -> 400
Status::not_found() -> 404
Status::already_exists() -> 409
Status::internal() -> 500
Status::unavailable() -> 503

// Usage
if ticket.is_none() {
    return Err(Status::not_found("ticket not found"));
}

if already_locked {
    return Err(Status::already_exists("ticket already locked"));
}
```

## Metadata & Headers

```rust
// Add metadata to response
let mut response = Response::new(ticket);
response.metadata_mut().insert("x-trace-id", trace_id.parse()?);

// Read metadata from request
let metadata = request.metadata();
let trace_id = metadata.get("x-trace-id")?;
```

## Interceptors (Middleware)

```rust
use tonic::service::Interceptor;

struct AuthInterceptor;

impl Interceptor for AuthInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, Status> {
        let token = request
            .metadata()
            .get("authorization")
            .ok_or_else(|| Status::unauthenticated("missing token"))?;
        
        // Validate token
        
        Ok(request)
    }
}

// Apply interceptor to client
let channel = Channel::from_static("http://db:8080")
    .connect()
    .await?;
let mut client = DbClient::with_interceptor(channel, AuthInterceptor);
```

## NGI Service Ports
- **DB:** `8080`
- **Custodian:** `8081`
- **Auth:** `8082`
- **Admin:** `8083`

All use gRPC over HTTP/2 with mTLS.

## Best Practices
1. Use protocol buffers for all service contracts
2. Always implement proper error handling with Status codes
3. Use interceptors for cross-cutting concerns (auth, logging)
4. Implement timeouts to prevent hanging requests
5. Validate all inputs at service boundaries
6. Return meaningful error messages for debugging

## Common Issues
- **Deadline Exceeded:** Service taking too long, increase timeout
- **Unavailable:** Service down or unreachable
- **Unauthenticated:** Missing or invalid credentials
- **PermissionDenied:** User lacks required permissions

---

## Official API Documentation

### Main Types

- **[Server](https://docs.rs/tonic/latest/tonic/transport/struct.Server.html)** - gRPC server
  - Methods:
    - [builder](https://docs.rs/tonic/latest/tonic/transport/struct.Server.html#method.builder) - Create builder
    - [add_service](https://docs.rs/tonic/latest/tonic/transport/server/struct.Routes.html#method.add_service) - Add service
    - [serve](https://docs.rs/tonic/latest/tonic/transport/server/struct.Routes.html#method.serve) - Start server

- **[Channel](https://docs.rs/tonic/latest/tonic/transport/struct.Channel.html)** - gRPC client channel
  - Methods:
    - [from_static](https://docs.rs/tonic/latest/tonic/transport/struct.Channel.html#method.from_static) - Connect to URI
    - [connect](https://docs.rs/tonic/latest/tonic/transport/struct.Endpoint.html#method.connect) - Establish connection
    - [tls_config](https://docs.rs/tonic/latest/tonic/transport/struct.Endpoint.html#method.tls_config) - Configure TLS

- **[Request](https://docs.rs/tonic/latest/tonic/struct.Request.html)** - gRPC request
  - Methods:
    - [into_inner](https://docs.rs/tonic/latest/tonic/struct.Request.html#method.into_inner) - Extract message
    - [metadata](https://docs.rs/tonic/latest/tonic/struct.Request.html#method.metadata) - Access metadata
    - [metadata_mut](https://docs.rs/tonic/latest/tonic/struct.Request.html#method.metadata_mut) - Modify metadata

- **[Response](https://docs.rs/tonic/latest/tonic/struct.Response.html)** - gRPC response
  - Methods:
    - [new](https://docs.rs/tonic/latest/tonic/struct.Response.html#method.new) - Create response
    - [metadata_mut](https://docs.rs/tonic/latest/tonic/struct.Response.html#method.metadata_mut) - Add metadata

- **[Status](https://docs.rs/tonic/latest/tonic/struct.Status.html)** - gRPC error status
  - Methods:
    - [ok](https://docs.rs/tonic/latest/tonic/struct.Status.html#method.ok) - Status code 0
    - [not_found](https://docs.rs/tonic/latest/tonic/struct.Status.html#method.not_found) - Status code 5
    - [already_exists](https://docs.rs/tonic/latest/tonic/struct.Status.html#method.already_exists) - Status code 6
    - [internal](https://docs.rs/tonic/latest/tonic/struct.Status.html#method.internal) - Status code 13
    - [unavailable](https://docs.rs/tonic/latest/tonic/struct.Status.html#method.unavailable) - Status code 14

- **[Streaming](https://docs.rs/tonic/latest/tonic/struct.Streaming.html)** - Streaming responses
  - Methods: recv(), message() for receiving streamed messages

- **[Code](https://docs.rs/tonic/latest/tonic/enum.Code.html)** - Status code enum

### Traits

- **[IntoRequest](https://docs.rs/tonic/latest/tonic/trait.IntoRequest.html)** - Convert to request
- **[IntoStreamingRequest](https://docs.rs/tonic/latest/tonic/trait.IntoStreamingRequest.html)** - Convert to streaming request

### Modules

- **[metadata](https://docs.rs/tonic/latest/tonic/metadata/index.html)** - Request/response metadata
- **[codec](https://docs.rs/tonic/latest/tonic/codec/index.html)** - Encoding/decoding
- **[service](https://docs.rs/tonic/latest/tonic/service/index.html)** - Tower service utilities

### Macros

- **[include_proto!](https://docs.rs/tonic/latest/tonic/macro.include_proto.html)** - Include generated proto
- **[include_file_descriptor_set!](https://docs.rs/tonic/latest/tonic/macro.include_file_descriptor_set.html)** - Include descriptors
- **[#[async_trait]](https://docs.rs/tonic/latest/tonic/attr.async_trait.html)** - Async trait attribute
