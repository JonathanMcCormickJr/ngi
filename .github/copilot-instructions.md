# GitHub Copilot Instructions for NGI

## Project Overview

NGI (Next-Gen Infoman) is a distributed, microservices-based tech support ticketing system built entirely in Rust. The system prioritizes memory safety, strong consistency where needed, and post-quantum security. All code must be idiomatic Rust with zero unsafe code in business logic.

**Core Principles:**
- No unsafe code (`#![forbid(unsafe_code)]`) in business logic
- Distributed consensus (Raft) for critical services
- Post-quantum security (TLS 1.3 + Kyber)
- Type-safe APIs throughout (no SQL injection risks)
- Test-Driven Development (TDD) workflow with ≥90% code coverage
- Blazing fast performance (optimized for concurrent users and low latency)
- Ruggedness (see the [Rugged Manifesto](https://ruggedsoftware.org/) - secure by design, resilient to failure)
  - Graceful degradation: degrade service rather than fail completely when problems occur
- Comprehensive, clear, accurate, and enjoyable documentation
- Soft deletes only (no hard deletes via API for audit trails and compliance)
- Stateless services where possible (no coordination overhead)
- Zero-redundancy data entry (avoid requesting data already available from other sources)
- Schema versioning for live evolution (add/remove fields and workflow steps without downtime)
- Audit everything critical (ticket locks, user actions, permission changes)
- User-friendly UI/UX (intuitive and streamlined interfaces for technicians and admins)
- KISS (Keep It Simple, Stupid) - avoid unnecessary complexity
- Correctness over cleverness - prioritize clear, maintainable code over "clever" solutions
- Extensibility - design with future features and integrations in mind, allowing for easy addition of new functionality without major refactoring (e.g. for enums use attributes like `#[repr(u8)]` and `#[non_exhaustive]`).
- Don't panic - handle errors gracefully and provide meaningful error messages. The only acceptable use of panic-causing uses like `assert!`, `panic!`, `unwrap`, or `expect` is in unrecoverable situations during initialization (e.g., configuration errors), in which case the panic-able conditions must be clearly and exhaustively documented, or within tests.
  - **Rationale**: In a distributed system like NGI, panics can crash services, disrupt consensus (e.g., Raft leader election), or cause cascading failures. Graceful error handling enables the system to degrade rather than fail completely, aligning with the Rugged Manifesto.
  - **Unrecoverable Situations**: Limited to startup failures where the service cannot safely operate (e.g., invalid TLS certificates, missing critical dependencies, or corrupted Raft state that prevents initialization). These must be documented with clear failure modes and recovery steps.
  - **Testing Exceptions**: Panics are acceptable in tests to assert invariants and fail fast on unexpected conditions. Use `assert!`, `panic!`, or `unwrap()` in test code, but ensure error paths are also tested via `Result` handling.
  - **Alternatives**: Always prefer `Result<T, E>` with proper error propagation using `?`. Use `anyhow::Context` for debugging context. For fallible operations, return errors to callers rather than panicking.
  - **Examples**:
    - **Good**: `env::var("REQUIRED_CONFIG").context("missing REQUIRED_CONFIG")?;`
    - **Bad**: `env::var("REQUIRED_CONFIG").unwrap();` (unless in startup with documentation)
    - **Test-Only**: `assert_eq!(result.unwrap(), expected);` (in `#[test]` functions)
- Read the docs! - If you have tried and failed to properly implement something 3 times, then stop and fetch the documentation for the dependency(ies) involved before proceeding with your 4th attempt. 

---

## Technology Stack

### Core Dependencies
- **Web Framework:** `axum` for REST APIs (LBRP only)
- **Async Runtime:** `tokio` for I/O, task scheduling, and message passing (`tokio::sync::mpsc`)
- **Database:** `sled` for embedded key-value storage with ACID transactions
- **Consensus:** `openraft` for Raft protocol implementation
- **TLS:** `rustls` for pure Rust TLS 1.3
- **Post-Quantum Crypto:** `pqc_kyber` for CRYSTALS-Kyber KEM
- **gRPC:** `tonic` (framework) + `prost` (codegen) for service-to-service communication
- **Serialization:** `serde` + `bincode` for efficient binary serialization
- **HTTP Client:** `reqwest` for outbound integrations

### Development Tools
- Linting: `cargo clippy` with pedantic warnings
- Formatting: `cargo fmt` with default settings
- Testing: `cargo test` (TDD workflow)
- Coverage: `cargo tarpaulin` (90% minimum required)
- Security: `cargo audit` for dependency vulnerabilities
- Watch Mode: `cargo watch` for continuous test execution

---

## Code Organization & Patterns

### Workspace Structure

```
ngi/
├── shared/              # Shared library crate (error types, data models)
├── db/                  # Database service (Raft + Sled)
├── custodian/           # Ticket management (Raft-based locking)
├── auth/                # Authentication (stateless)
├── admin/               # User management & monitoring (stateless)
├── lbrp/                # Load balancer & reverse proxy
├── chaos/               # Fault injection service
├── honeypot/            # Intrusion detection (deceptive service)
└── tests/               # Integration tests
```

### Service Categorization

**Consensus-Based (Raft, requires 3+ instances):**
- `db` - Data persistence with strong consistency
- `custodian` - Distributed locking for ticket operations

**Stateless (can run 1+ instances, no coordination):**
- `auth` - Session state delegated to DB
- `admin` - Reads/writes through DB service
- `lbrp` - Routing and load balancing
- `chaos` - Fault injection (intentionally unpredictable)

### Error Handling

**Use the shared error types from `shared/src/error.rs`:**
- Define custom error types using `thiserror` crate
- All fallible functions return `Result<T, Error>`
- Convert external errors at API boundaries only
- Use `?` operator for error propagation
- Provide context with `.context()` when using `anyhow` for debugging

```rust
// Good: Type-safe custom error
#[derive(thiserror::Error)]
pub enum TicketError {
    #[error("ticket not found: {0}")]
    NotFound(u64),
    #[error("ticket already locked by {user}")]
    AlreadyLocked { user: String },
}

// Use context for operational errors
result.context("failed to acquire lock")?;
```

### Data Models

**Location:** `shared/src/` contains all shared types:
- `ticket.rs` - Ticket enums and structs (Status, Resolution, NextAction, Symptom)
- `user.rs` - User types and roles
- `error.rs` - Shared error types

**Design Principles:**
- Use enums for Status, Resolution, NextAction, Symptom (stored as `u8` for efficiency)
- All timestamps are `SystemTime` or `DateTime<Utc>`
- IDs are `u64` (auto-incremented)
- UUIDs for account identifiers
- Serialize with `#[derive(serde::Serialize, serde::Deserialize)]`

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TicketStatus {
    Open = 0,
    AwaitingCustomer = 1,
    AwaitingISP = 2,
    Closed = 3,
    AutoClosed = 4,
    // ...
}

pub struct Ticket {
    pub id: u64,
    pub title: String,
    pub status: TicketStatus,
    pub lock: Option<UserId>,
    pub created_at: SystemTime,
    pub deleted: bool,  // Soft delete flag
    pub deleted_at: Option<SystemTime>,
    // ...
}
```

---

## Service Communication

### gRPC Over HTTP/2

**Implement all inter-service communication using gRPC with `tonic` and `prost`:**

```rust
// In Cargo.toml
[dependencies]
tonic = "0.x"
prost = "0.x"
tokio = { version = "1", features = ["full"] }

// In build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/service.proto")?;
    Ok(())
}
```

**Client Usage:**
```rust
use tonic::transport::Channel;
use db::db_client::DbClient;

// Create client (with leader-aware routing for Raft services)
let channel = Channel::from_static("http://db-leader:8080").connect().await?;
let mut client = DbClient::new(channel);

// Make request
let response = client.get_ticket(GetTicketRequest { id: 42 }).await?;
```

**Server Implementation (tonic):**
```rust
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl db_server::Db for DbServiceImpl {
    async fn get_ticket(
        &self,
        request: Request<GetTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();
        // Implementation
        Ok(Response::new(ticket))
    }
}
```

### mTLS (Mutual TLS)

**Configure every inter-service connection with mTLS:**
- Services have unique certificates and private keys
- Use `rustls` for TLS 1.3 with client certificates
- Configure in connection setup, not per-request

```rust
let client_config = rustls::ClientConfig::builder()
    .with_safe_defaults()
    .with_root_certificates(root_cert_store)
    .with_client_auth_credentials(client_certs)
    .build()?;

let tonic_client_config = tonic::transport::ClientTlsConfig::new()
    .rustls_client_config(client_config);

let channel = Channel::from_static("https://db:8080")
    .tls_config(tonic_client_config)?
    .connect()
    .await?;
```

### REST API (LBRP Only)

**Expose REST/JSON APIs exclusively through the LBRP service.**

Use `axum` framework:
```rust
use axum::{routing::get, Router, Json, extract::Path};

async fn get_ticket(Path(id): Path<u64>) -> Json<Ticket> {
    // Call gRPC client to custodian service
    let ticket = custodian_client.get_ticket(id).await?;
    Json(ticket)
}

let app = Router::new()
    .route("/api/ticket/:id", get(get_ticket));
```

---

## Raft Consensus Pattern

### Services Using Raft

**Database Service (`db`):**
- 3+ instances for quorum (tolerates 1 failure)
- Leader handles all writes
- Followers replicate the log
- Automatic leader election on failure

**Custodian Service (`custodian`):**
- 3+ instances for quorum
- Lock operations go through leader
- Prevents race conditions on ticket locks

### Implementation Pattern

```rust
// 1. Define your state machine
pub struct TicketLockStateMachine {
    locks: HashMap<u64, LockInfo>,
}

// 2. Implement openraft's RaftStateMachine trait
#[async_trait]
impl RaftStateMachine for TicketLockStateMachine {
    type D = LogEntry;
    type R = CommandResponse;
    type E = Error;
    
    async fn apply(&mut self, entries: Vec<Self::D>) -> Result<Vec<Self::R>> {
        for entry in entries {
            match entry.command {
                Command::AcquireLock(ticket_id, user_id) => {
                    if self.locks.contains_key(&ticket_id) {
                        return Err(Error::AlreadyLocked);
                    }
                    self.locks.insert(ticket_id, LockInfo { owner: user_id });
                }
                Command::ReleaseLock(ticket_id) => {
                    self.locks.remove(&ticket_id);
                }
            }
        }
        Ok(vec![CommandResponse::Ok])
    }
}

// 3. Wrap with openraft and expose via gRPC
pub struct RaftNode {
    raft: Raft<TypeConfig>,
    sm: Arc<RwLock<TicketLockStateMachine>>,
}

impl RaftNode {
    pub async fn acquire_lock(&self, ticket_id: u64, user_id: UserId) -> Result<()> {
        let entry = LogEntry {
            command: Command::AcquireLock(ticket_id, user_id),
        };
        self.raft.append_entries(vec![entry]).await?;
        Ok(())
    }
}
```

### Leader-Aware Routing (LBRP)

**The LBRP service must identify the active Raft leader for routing:**
```rust
// Query each instance for leadership status
async fn find_leader(instances: &[String]) -> Result<String> {
    for instance in instances {
        if is_leader(instance).await? {
            return Ok(instance.clone());
        }
    }
    Err(Error::NoLeaderElected)
}

// Route lock operations to leader
async fn acquire_lock(req: LockRequest) -> Response {
    let leader = find_leader(&custodian_instances).await?;
    let mut client = CustodianClient::connect(leader).await?;
    client.acquire_lock(req).await?
}
```

---

## Database Storage Pattern (Sled)

### Key Design

**Use prefixed keys to simulate table-like structures:**
```
ticket:{ticket_id} -> Ticket struct (bincode)
ticket:index:status:{status}:{ticket_id} -> empty value
ticket:index:assigned:{user_id}:{ticket_id} -> empty value
user:{user_id} -> User struct
lock:{ticket_id} -> LockInfo struct
```

### Implementation

```rust
use sled::{Db, Tree};
use serde::{Serialize, Deserialize};

pub struct StorageLayer {
    db: Db,
    tickets: Tree,
    users: Tree,
}

impl StorageLayer {
    // Insert with secondary indexes
    pub fn insert_ticket(&self, ticket: &Ticket) -> Result<()> {
        let key = format!("ticket:{}", ticket.id);
        let value = bincode::serialize(ticket)?;
        self.tickets.insert(key.as_bytes(), value)?;
        
        // Create secondary index
        let status_key = format!("ticket:index:status:{}:{}", 
            ticket.status as u8, ticket.id);
        self.tickets.insert(status_key.as_bytes(), &[])?;
        
        Ok(())
    }
    
    // Query by secondary index
    pub fn tickets_by_status(&self, status: TicketStatus) -> Result<Vec<Ticket>> {
        let prefix = format!("ticket:index:status:{}:", status as u8);
        let mut tickets = Vec::new();
        
        for item in self.tickets.scan_prefix(prefix.as_bytes()) {
            let (key, _) = item?;
            let key_str = String::from_utf8(key.to_vec())?;
            let ticket_id: u64 = key_str.split(':').nth(3).unwrap().parse()?;
            
            let ticket_key = format!("ticket:{}", ticket_id);
            if let Some(value) = self.tickets.get(ticket_key.as_bytes())? {
                let ticket = bincode::deserialize(&value)?;
                tickets.push(ticket);
            }
        }
        
        Ok(tickets)
    }
}
```

### Transactions

```rust
// Sled provides ACID transactions
let result = self.db.transaction(|txn| {
    txn.insert(b"ticket:42", bincode::serialize(&ticket)?)?;
    txn.insert(b"lock:42", bincode::serialize(&lock_info)?)?;
    Ok(())
})?;
```

---

## Soft Deletes & Data Retention

**Implementation details for soft deletes:**

```rust
pub struct Ticket {
    // ...
    pub deleted: bool,
    pub deleted_at: Option<SystemTime>,
}

// Soft delete operation
pub async fn soft_delete_ticket(&self, id: u64) -> Result<()> {
    let mut ticket = self.get_ticket(id).await?;
    ticket.deleted = true;
    ticket.deleted_at = Some(SystemTime::now());
    self.update_ticket(&ticket).await?;
    Ok(())
}

// Queries exclude soft-deleted records
pub fn get_active_tickets(&self) -> Result<Vec<Ticket>> {
    self.db.scan_prefix(b"ticket:")
        .filter_map(|item| {
            let ticket: Ticket = bincode::deserialize(&item.1).ok()?;
            if !ticket.deleted { Some(ticket) } else { None }
        })
        .collect()
}

// Hard delete (admin only, not exposed via API):
// - Performed during maintenance windows with explicit approval
// - May be subject to regulatory retention requirements
```

---

## Security Architecture

### Double-Layer Encryption

**Layer 1: TLS 1.3 (Transport)**
- All HTTP traffic uses HTTPS via `rustls`
- mTLS for inter-service communication
- Perfect forward secrecy

**Layer 2: Post-Quantum KEM (Application)**
- Kyber-768 for sensitive payloads
- Provides resilience against future quantum computing threats

```rust
use pqc_kyber::Kem768;

// Encrypt sensitive data
let (ciphertext, secret) = Kem768::encapsulate(&public_key)?;
let encrypted_payload = encrypt_with_secret(&sensitive_data, &secret)?;

// Recipient decrypts
let secret = Kem768::decapsulate(&ciphertext, &private_key)?;
let plaintext = decrypt_with_secret(&encrypted_payload, &secret)?;
```

### Authentication & Authorization (MFA)

**Current Methods:**
- Password
- TOTP (Time-based One-Time Password)
- WebAuthn
- U2F

**Planned Additions:**
- Active Directory integration (OS-level authentication counts toward MFA)
- LDAP/Kerberos support

**RBAC Roles:**
- Admin
- Manager
- Technician
- EbondPartner
- ReadOnly

**Enforce in Service Handlers:**
```rust
pub async fn update_ticket(&self, request: Request<UpdateRequest>) -> Result<Response<Ticket>> {
    let user = verify_session(&request).await?;
    check_permission(user, Permission::UpdateTicket)?;
    
    // Implementation
    Ok(Response::new(ticket))
}
```

---

## Testing & TDD

### Test-Driven Development Workflow

1. **Write test first** - Define intended behavior
2. **Run test** - Confirm it fails
3. **Implement code** - Make test pass
4. **Refactor** - Clean up implementation

### Integration Tests

**Location:** `tests/src/lib.rs` or service-specific `src/tests.rs`

```rust
#[tokio::test]
async fn test_acquire_lock() {
    let client = create_test_client().await;
    
    // First acquire should succeed
    let resp = client.acquire_lock(LockRequest { 
        ticket_id: 1, 
        user_id: 42 
    }).await.unwrap();
    assert!(resp.success);
    
    // Second acquire should fail (already locked)
    let resp = client.acquire_lock(LockRequest { 
        ticket_id: 1, 
        user_id: 99 
    }).await;
    assert!(resp.is_err());
    
    // Release and re-acquire should work
    client.release_lock(LockRelease { ticket_id: 1 }).await.unwrap();
    let resp = client.acquire_lock(LockRequest { 
        ticket_id: 1, 
        user_id: 99 
    }).await.unwrap();
    assert!(resp.success);
}
```

### Coverage Requirements

- **Minimum 90%** code coverage verified with `cargo tarpaulin`
- Cover both happy paths and error cases
- Include edge cases (empty inputs, boundary values)
- Test distributed scenarios (leader election, network partitions)

```bash
cargo tarpaulin --out Html --output-dir coverage
```

### Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p custodian

# Watch mode (continuous)
cargo watch -x test
```

---

## Code Quality Standards

### Linting (cargo clippy)

**Enable and run pedantic lints:**
```bash
cargo clippy --all-targets --all-features -- -W clippy::pedantic
```

**Fix style issues automatically:**
```bash
cargo fmt --all
```

### Forbidden Unsafe Code in Business Logic

**Do NOT use `unsafe` except in special cases (ffi, performance-critical code):**
```rust
// BAD: Never do this in business logic
unsafe fn process_ticket() { /* ... */ }

// GOOD: If unsafe is absolutely necessary, document why
/// SAFETY: This function interfaces with C FFI.
/// The caller must ensure the pointer is valid and properly aligned.
unsafe fn call_c_function(ptr: *const u8) { /* ... */ }
```

**Enforce with:**
```rust
// In lib.rs or main.rs
#![forbid(unsafe_code)]
```

### Documentation

**Require doc comments for all public items:**
```rust
/// Acquires an exclusive lock on a ticket.
///
/// # Arguments
/// * `ticket_id` - The ticket to lock
/// * `user_id` - The user acquiring the lock
///
/// # Returns
/// Returns `Ok(())` on success, or an error if the ticket is already locked.
///
/// # Panics
/// Never panics.
///
/// # Examples
/// ```
/// let ticket_id = 42;
/// let user_id = 1;
/// custodian.acquire_lock(ticket_id, user_id).await?;
/// ```
pub async fn acquire_lock(&self, ticket_id: u64, user_id: UserId) -> Result<()> {
    // Implementation
}
```

### Dependencies

**Audit dependencies regularly:**
```bash
cargo audit
```

**Review before adding new dependencies:**
- Is it well-maintained?
- Does it pull in excessive dependencies?
- Does it have known vulnerabilities?
- Prefer `no_std` when possible for security

---

## Common Patterns & Anti-Patterns

### Good Patterns ✓

**Type-safe IDs:**
Use newtype wrappers to prevent mixing incompatible ID types:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

pub struct Ticket {
    pub assigned_to: Option<UserId>,
}
```

**Result types for errors:**
Return `Result<T>` (not `Option<T>`) for operations that may fail:
```rust
pub async fn get_ticket(&self, id: u64) -> Result<Ticket> {
    // Use Result, not Option, for operations that can fail
}
```

**Async/await:**
```rust
// Good: Natural async syntax
let ticket = custodian.get_ticket(id).await?;

// Avoid: Unnecessary blocking
// tokio::task::block_in_place { /* ... */ }  // Only when necessary
```

**Structured logging:**
```rust
tracing::info!(ticket_id = %id, status = ?new_status, "ticket status changed");
```

### Anti-Patterns to Avoid ✗

**Don't use `.unwrap()` in production code:**
```rust
// BAD
let ticket = self.get_ticket(id).await.unwrap();  // Panics on error

// GOOD
let ticket = self.get_ticket(id).await.context("failed to fetch ticket")?;
```

**Don't use `clone()` excessively:**
```rust
// BAD: Unnecessary clone
let ticket = self.tickets.get(&id)?.clone();

// GOOD: Use references when possible
let ticket = &self.tickets[&id];
```

**Don't ignore errors:**
```rust
// BAD
let _ = self.update_ticket(&ticket).await;

// GOOD
self.update_ticket(&ticket).await.context("failed to update ticket")?;

// Or if intentional:
if let Err(e) = self.update_ticket(&ticket).await {
    tracing::warn!("failed to update ticket: {}", e);
}
```

---

## Deployment & Configuration

### Service Ports (808x range for memorability)

- **DB:** `8080` (gRPC)
- **Custodian:** `8081` (gRPC)
- **Auth:** `8082` (gRPC)
- **Admin:** `8083` (gRPC)
- **LBRP:** `443` (HTTPS) / `80` (HTTP redirect)

**Note:** Port numbers are for memorability. Security is ensured through mTLS and network isolation, not port obscurity.

### Deployment Requirements

- Minimum 3 instances for Raft services (DB, Custodian)
- All tests must pass: `cargo test`
- Coverage ≥ 90%: `cargo tarpaulin`
- No vulnerabilities: `cargo audit`
- Code style: `cargo fmt` & `cargo clippy`
- Documentation up-to-date

### Environment Variables

**Use environment variables for configuration only:**
```rust
std::env::var("RUST_LOG")  // Logging level
std::env::var("RAFT_NODES")  // Comma-separated list of Raft peer addresses
```

**Store secrets externally, never in environment variables:**
- TLS private keys
- Certificate files
- Database encryption keys

---

## Troubleshooting

### Common Issues

**1. Raft cluster won't start:**
- Ensure 3+ instances are running
- Check network connectivity between instances
- Verify certificate validity (mTLS)

**2. Lock timeouts:**
- Check lock timeout configuration
- Verify custodian leader is running
- Look for lock holder crashes

**3. Slow queries:**
- Use Sled secondary indexes for filtering
- Avoid full table scans
- Profile with `cargo flamegraph`

**4. Test failures:**
- Run failed test in isolation: `cargo test test_name -- --nocapture`
- Check for timing issues in distributed tests
- Ensure test teardown properly cleans up resources

---

## References

- **ARCHITECTURE.md** - Detailed system design
- **README.md** - Project overview and features
- **Tokio Documentation** - Async runtime guide
- **tonic Documentation** - gRPC framework
- **openraft Documentation** - Raft consensus implementation
- **Sled Documentation** - Embedded database guide
- **Rust Book** - Core language concepts

---

## Questions?

If requirements are unclear while implementing:
1. Check ARCHITECTURE.md for service responsibilities
2. Review existing service patterns in similar services
3. Prioritize type safety and correctness over cleverness
4. Ask for clarification rather than guessing
