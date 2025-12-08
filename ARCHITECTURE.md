# NGI Architecture Documentation

**Version:** 0.1.0-mvp  
**Date:** December 7, 2025  
**Author:** Jonathan A. McCormick, Jr.

---

## Table of Contents

1. [Overview](#overview)
2. [Technology Stack](#technology-stack)
3. [Workspace Structure](#workspace-structure)
4. [Consensus & Distributed Systems](#consensus--distributed-systems)
5. [Database Architecture](#database-architecture)
6. [Security Architecture](#security-architecture)
7. [Service Descriptions](#service-descriptions)
8. [Inter-Service Communication](#inter-service-communication)
9. [Configuration Management](#configuration-management)
10. [Deployment Model](#deployment-model)
11. [Data Models](#data-models)
12. [API Specifications](#api-specifications)

---

## Overview

NGI (Next-Gen Infoman) is a distributed, microservices-based tech support ticketing system designed for high availability, strong consistency where needed, and quantum-resistant security. The system is built entirely in Rust to leverage memory safety, fearless concurrency, and high performance.

### Core Principles

- **Safety First**: No unsafe code (`#![forbid(unsafe_code)]`) in business logic
- **Distributed Consistency**: Raft consensus for critical services
- **Post-Quantum Security**: Double encryption (TLS 1.3 + Kyber)
- **Zero Downtime**: Multi-instance deployment with automatic failover
- **Type Safety**: Rust-native APIs throughout (no SQL injection risks)

---

## Technology Stack

### Core Dependencies

| Component | Crate | Version | Purpose |
|-----------|-------|---------|---------|
| **Web Framework** | `axum` | latest | HTTP routing, handlers, middleware |
| **Async Runtime** | `tokio` | latest | Async I/O, task scheduling |
| **Database** | `sled` | latest | Embedded key-value store |
| **Consensus** | `openraft` | latest | Raft protocol for distributed consensus |
| **TLS** | `rustls` | latest | Pure Rust TLS 1.3 implementation |
| **Post-Quantum Crypto** | `pqc_kyber` | latest | CRYSTALS-Kyber KEM |
| **Serialization** | `serde` + `bincode` | latest | Efficient binary serialization |
| **Message Passing** | `tokio::sync::mpsc` | (built-in) | Inter-component communication |
| **HTTP Client** | `reqwest` | latest | Inter-service HTTP requests |

### Development Tools

- **Linting**: `cargo clippy` with pedantic warnings
- **Formatting**: `cargo fmt` with default settings
- **Testing**: `cargo test` with integration tests
- **Coverage**: `cargo tarpaulin` (90% minimum)
- **Security**: `cargo audit` for dependency vulnerabilities

---

## Workspace Structure

```
ngi/
├── Cargo.toml              # Workspace definition
├── README.md               # User-facing documentation
├── ARCHITECTURE.md         # This file
├── .github/
│   └── workflows/
│       └── rust.yml        # CI/CD pipeline
│
├── common/                 # Shared library crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── ticket.rs       # Ticket types and enums
│       ├── user.rs         # User types
│       ├── error.rs        # Error types
│       └── config.rs       # Configuration utilities
│
├── consensus/              # Raft wrapper library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── raft_node.rs    # Raft node abstraction
│       └── state_machine.rs
│
├── config/                 # Service discovery library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── discovery.rs    # Service registry
│
├── db/                     # Database service (binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── storage.rs      # Sled wrapper
│       ├── raft_sm.rs      # Raft state machine
│       └── api.rs          # HTTP API handlers
│
├── custodian/              # Ticket management service (binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── locks.rs        # Distributed lock management
│       ├── raft_sm.rs      # Raft state machine
│       └── api.rs          # HTTP API handlers
│
├── auth/                   # Authentication service (binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── mfa.rs          # Multi-factor auth logic
│       ├── session.rs      # Session management
│       └── api.rs          # HTTP API handlers
│
├── admin/                  # Admin & monitoring service (binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── monitoring.rs   # Metrics collection
│       ├── users.rs        # User/role management
│       └── api.rs          # HTTP API handlers
│
├── lbrp/                   # Load Balancer & Reverse Proxy (binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── proxy.rs        # Reverse proxy logic
│       ├── balancer.rs     # Load balancing algorithms
│       └── static_files.rs # Frontend asset serving
│
├── chaos/                  # Fault injection service (binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── injector.rs     # Chaos scenarios
│
└── tests/                  # Integration tests
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

---

## Consensus & Distributed Systems

### Why Raft?

**Raft** was chosen over Paxos for the following reasons:

1. **Understandability**: Raft is designed to be understandable. Its leader-based approach and clear role separation make it easier to reason about correctness.
2. **Proven in Production**: Used by etcd (Kubernetes), TiKV (TiDB), and Consul
3. **Strong Consistency**: Provides linearizable reads/writes through leader
4. **Leader Election**: Automatic failover when leader fails
5. **Log Replication**: Ensures all nodes eventually agree on state
6. **Rust Implementation**: `openraft` is a mature, pure-Rust implementation

### Services Requiring Consensus

#### 1. Database Service (`db`)

**Why Consensus?**
- Data integrity across replicas
- Consistent reads/writes
- Durable storage guarantees

**Configuration:**
- **Minimum instances**: 3 (required for Raft quorum and fault tolerance)
- **Replication mode**: Leader-based (writes to leader, reads from any)
- **State machine**: Key-value operations on Sled
- **Quorum math**: With 3 nodes, system tolerates 1 failure (needs 2/3 for consensus)

**Raft Role:**
- Leader handles all write operations
- Followers replicate the log
- If leader fails, a follower becomes leader automatically

#### 2. Custodian Service (`custodian`)

**Why Consensus?**
- **Critical**: Prevent ticket lock race conditions
- Ensure only one user can lock a ticket at any time
- Coordinate across multiple custodian instances

**Configuration:**
- **Minimum instances**: 3 (required for Raft quorum and fault tolerance)
- **Replication mode**: Leader-based (lock operations through leader)
- **State machine**: Ticket lock state
- **Quorum math**: With 3 nodes, system tolerates 1 failure (needs 2/3 for consensus)

**Raft Role:**
- Lock acquisition/release goes through leader
- Linearizable consistency guarantees no double-locking
- If leader fails, locks are preserved in replicated log

### Services Without Consensus (Stateless)

- **Auth**: Session state stored in DB, service is stateless
- **Admin**: Reads/writes through DB service
- **LBRP**: Routes requests, no state to coordinate
- **Chaos**: Test service, intentionally unpredictable

These can run single-instance for MVP and scale horizontally without coordination.

---

## Database Architecture

### Storage Layer: Sled

**Sled** is an embedded ordered key-value store written in pure Rust. It provides:

- **ACID transactions**: Atomic, consistent, isolated, durable
- **Embedded**: No separate server process
- **Fast**: Lock-free, optimized for modern hardware
- **Safe**: Written in safe Rust

### Relational Layer: Custom

We build a **type-safe relational layer** on top of Sled's key-value interface.

#### Key Design

Keys are structured with prefixes to simulate tables:

```
ticket:{ticket_id} -> Ticket struct (bincode)
ticket:index:status:{status}:{ticket_id} -> empty value (for queries)
ticket:index:assigned:{user_id}:{ticket_id} -> empty value
user:{user_id} -> User struct
lock:{ticket_id} -> LockInfo struct
```

#### Example Operations

**Insert Ticket:**
```rust
// Main record
db.insert(b"ticket:12345", bincode::serialize(&ticket)?)?;

// Secondary indexes
db.insert(b"ticket:index:status:Open:12345", &[])?;
db.insert(b"ticket:index:assigned:user42:12345", &[])?;
```

**Query by Status:**
```rust
let prefix = b"ticket:index:status:Open:";
for item in db.scan_prefix(prefix) {
    let (key, _) = item?;
    let ticket_id = extract_id_from_key(&key);
    let ticket = get_ticket(ticket_id)?;
    // ...
}
```

### Raft Integration

The Sled database is wrapped in a **Raft state machine**:

1. Client sends write request to any DB instance
2. Non-leader forwards to leader
3. Leader appends to Raft log
4. Log replicates to followers (quorum)
5. Leader applies to Sled, returns success
6. Followers apply to their Sled instances

This ensures all DB replicas have identical state.

---

## Security Architecture

### Network Encryption: Double-Layer

**Layer 1: TLS 1.3 (Transport)**
- All HTTP traffic uses HTTPS via `rustls`
- Certificate-based authentication between services (mTLS)
- Perfect forward secrecy with modern cipher suites

**Layer 2: Post-Quantum KEM (Application)**
- Kyber-768 key encapsulation for additional protection
- Wraps sensitive payloads (e.g., session tokens, ticket data)
- Protects against future quantum computer attacks

#### Encryption Flow

```
Client -> LBRP -> Service

1. TLS 1.3 handshake establishes secure channel
2. Application payload encrypted with Kyber-derived key
3. Encrypted payload sent over TLS connection
4. Recipient decrypts Kyber layer, then processes
```

### Authentication & Authorization

**Multi-Factor Authentication (MFA) Methods:**

*Pre-v1.0:*

1. **Password**
2. **TOTP**
3. **WebAuthn**
4. **U2F**

*Anticipated v1.0 Addition:*

5. **Active Directory Integration**
   - OS-level authentication counts toward MFA
   - LDAP/Kerberos integration

**Role-Based Access Control (RBAC):**
- Roles: Admin, Manager, Technician, EbondPartner, ReadOnly
- Permissions: CreateTicket, LockTicket, UpdateTicket, ManageUsers, etc.
- Stored in DB service, enforced by all services

### Certificate Management (mTLS)

Each service has:
- **Private key**: Never leaves the service
- **Certificate**: Signed by internal CA
- **CA certificate**: Used to verify peer certificates

Services only accept connections from other services with valid certificates.

---

## Service Descriptions

### Database Service (`db`)

**Purpose**: Centralized data persistence with distributed consensus

**Port**: `8080` (configurable)

> **Note on Port Numbers**: The 808x port range is used for memorability and to avoid common port conflicts. Real security comes from mTLS authentication and network isolation, not port obscurity. Attackers scan all ports regardless of number.

**API Endpoints**:
- `POST /ticket` - Create ticket
- `GET /ticket/:id` - Get ticket by ID
- `PUT /ticket/:id` - Update ticket
- `DELETE /ticket/:id` - Delete ticket (soft delete: marks as deleted but retains data)
- `GET /tickets/query` - Query tickets with filters

> **Soft Delete vs Hard Delete**: NGI uses **soft deletes** exclusively for tickets and users. Records are marked `deleted: true` with a `deleted_at` timestamp but remain in the database. This enables:
> - Audit trails (who deleted what, when)
> - Accidental deletion recovery
> - Regulatory compliance (data retention policies)
> - Historical reporting and analytics
> Hard deletes (permanent removal) are never exposed via API and only performed during database maintenance with explicit approval.
- `POST /user` - Create user
- `GET /user/:id` - Get user by ID
- `PUT /user/:id` - Update user
- `DELETE /user/:id` - Delete user (soft delete)

**Dependencies**:
- `sled` for storage
- `openraft` for consensus
- `axum` for HTTP API

**Clustering**:
- **3 instances minimum** (recommended for Raft quorum)
- Leader handles writes
- Followers can serve reads (eventual consistency option)
- Quorum: 2 out of 3 nodes must agree for writes

---

### Custodian Service (`custodian`)

**Purpose**: Ticket lifecycle management with distributed locking

**Port**: `8081` (configurable)

**API Endpoints**:
- `POST /ticket` - Create new ticket (delegates to DB)
- `POST /ticket/:id/lock` - Acquire exclusive lock
- `DELETE /ticket/:id/lock` - Release lock
- `PUT /ticket/:id` - Update ticket information (requires lock)

**Key Features**:
- **Distributed locks**: Raft consensus prevents race conditions
- **Auto-lock expiry**: Locks expire after timeout (configurable)
- **Lock stealing prevention**: Only lock owner can release

**Dependencies**:
- `openraft` for lock coordination
- `db` service for persistence

**Clustering**:
- **3 instances minimum** (recommended for Raft quorum)
- Lock operations go through leader
- Quorum: 2 out of 3 nodes must agree for lock operations

---

### Auth Service (`auth`)

**Purpose**: User authentication and session management

**Port**: `8082` (configurable)

**API Endpoints**:
- `POST /login/creds` - Username + password
- `POST /login/totp` - TOTP verification
- `POST /login/webauthn` - WebAuthn challenge/response
- `POST /logout` - Invalidate session
- `GET /session/verify` - Verify session token
- `POST /:user/enroll/:method` - Enroll a new MFA method
- `DELETE /:user/mfa/:method` - Remove specified MFA method
- `PUT /:user/password` - Change user password

**Session Storage**:
- Sessions stored in DB service
- Short-lived JWT tokens (15 min)
- Refresh tokens (7 days)

**Dependencies**:
- `db` service for user data and sessions

**Clustering**:
- Stateless, can run 1+ instances
- No coordination needed (delegates to DB)

---

### Admin Service (`admin`)

**Purpose**: User management, roles, permissions, monitoring, and metrics

**Port**: `8083` (configurable)

**API Endpoints**:
- `POST /user` - Create user
- `PUT /user/:id` - Update user
- `DELETE /user/:id` - Delete user (soft delete)
- `POST /role` - Create role
- `PUT /user/:id/role` - Assign role
- `GET /metrics` - System metrics
- `GET /health` - Service health checks

**Monitoring Features**:
- Service health aggregation
- Request metrics (latency, throughput)
- Error rate tracking
- Resource usage (CPU, memory)
- Data export in CSV, XLSX, JSON, & Prometheus formats

**Dependencies**:
- `db` service for user/role data
- All services for health checks

**Clustering**:
- Stateless, single instance for MVP

---

### LBRP Service (`lbrp`)

**Purpose**: Load balancing, reverse proxy, and static file serving

**Port**: `443` (HTTPS) / `80` (HTTP redirect)

**Routes**:
- `/api/ticket/*` → `custodian` service
- `/api/auth/*` → `auth` service
- `/api/admin/*` → `admin` service
- `/api/db/*` → `db` service (internal only)
- `/*` → Static frontend files

**Load Balancing Algorithm**:
- Round-robin for stateless services (auth, admin)
- Leader-aware routing for stateful services (db, custodian)

**Features**:
- TLS termination
- Request rate limiting
- CORS handling
- Compression (gzip, brotli)

**Dependencies**:
- `config` crate for service discovery

**Clustering**:
- Single instance for MVP
- Can use external LB (nginx, HAProxy) in front

---

### Chaos Service (`chaos`)

**Purpose**: Fault injection for resilience testing

**Port**: `8084` (configurable)

**Injection Types**:
- Network latency injection
- Service crash simulation
- Disk I/O delays
- CPU/memory pressure
- Raft leader failure

**API Endpoints**:
- `POST /chaos/inject` - Start chaos scenario
- `DELETE /chaos/stop` - Stop active scenario
- `GET /chaos/scenarios` - List available scenarios

**Safety**:
- Only enabled in test environments
- Requires admin authentication

**Clustering**:
- Single instance (chaos doesn't need HA!)

---

## Inter-Service Communication

### Communication Patterns

#### 1. HTTP/REST (Primary)

Services expose RESTful APIs over HTTPS (mTLS).

**Example: Create Ticket Flow**
```
Client → LBRP → Custodian → DB
```

1. Client sends `POST /api/ticket` to LBRP
2. LBRP routes to Custodian instance
3. Custodian validates request and assigns lock if needed
4. Custodian sends `POST /ticket` to DB leader
5. DB writes to Raft log, replicates
6. DB returns ticket ID
7. Custodian returns response to LBRP
8. LBRP returns response to Client. User is now able to see the ticket in their UI and manually enter data into it and submit updates.
```

#### 2. Internal Message Passing (tokio::mpsc)

Within a service, components use `tokio::sync::mpsc` channels.

**Example: DB Service Internal**
```
HTTP Handler → Channel → Raft Module → Sled
```

Channels decouple the HTTP layer from the storage layer.

### Service Discovery

Services discover each other via **static configuration file** (`services.toml`):

```toml
[services.db]
instances = [
  { id = "db1", url = "https://db1.internal:8080", role = "leader" },
  { id = "db2", url = "https://db2.internal:8080", role = "follower" },
]

[services.custodian]
instances = [
  { id = "custodian1", url = "https://custodian1.internal:8081", role = "leader" },
  { id = "custodian2", url = "https://custodian2.internal:8081", role = "follower" },
]

[services.auth]
instances = [
  { id = "auth1", url = "https://auth1.internal:8082" },
]

[services.admin]
instances = [
  { id = "admin1", url = "https://admin1.internal:8083" },
]

[services.lbrp]
instances = [
  { id = "lbrp1", url = "https://lbrp1.internal:443" },
]
```

**Dynamic Configuration Updates:**

Services reload `services.toml` periodically (every 30 seconds) to detect changes. When Raft leadership changes:

1. New leader updates its `role` in config file (atomic write)
2. All services detect the change on next reload
3. Clients automatically redirect requests to new leader
4. No manual intervention required

**Implementation**: Use file watching (via `notify` crate) to detect config changes immediately rather than waiting for 30s poll. Leader election triggers config file update via consensus - only the leader writes its status.

**Config file ownership**: 
- Shared filesystem (NFS, Ceph) OR
- Each service maintains local copy, leader broadcasts updates via Raft OR
- External config service (etcd, Consul) - future enhancement

For MVP: Each service maintains local `services.toml`, admin manually updates when deploying/removing instances. Raft handles leader election internally; services query Raft status via API to determine current leader.

---

## Configuration Management

### Configuration File Format

Each service reads:
1. **services.toml** (service discovery)
2. **{service_name}.toml** (service-specific config)

**Example: `db.toml`**
```toml
[server]
bind_address = "0.0.0.0:8080"
tls_cert = "/etc/ngi/certs/db.crt"
tls_key = "/etc/ngi/certs/db.key"
ca_cert = "/etc/ngi/certs/ca.crt"

[raft]
node_id = "550e8400-e29b-41d4-a716-446655440000"  # UUIDv4 for unique node identity
peers = [
  "7c9e6679-7425-40de-944b-e07fc1f90ae7",
  "9f4e2ae1-82c3-4f3e-8d6b-4c5e7a8f0123"
]
election_timeout_ms = 1000
heartbeat_interval_ms = 300

[storage]
data_dir = "/var/lib/ngi/db"
max_log_size_mb = 1024
```

### Environment Variables

Override config with environment variables:

```bash
NGI_DB_BIND_ADDRESS=0.0.0.0:9999
NGI_DB_NODE_ID=1
```

---

## Deployment Model

### MVP Deployment Topology

```mermaid
graph TD
  subgraph Gateway
    LBRP["LBRP<br/>(port 443)"]
  end

  subgraph Authentication
    Auth["Auth<br/>(8082)"]
  end

  subgraph Administration
    Admin["Admin<br/>(8083)"]
  end

  subgraph "Custodian Cluster"
    CustLeader["Custodian<br/>Leader<br/>(8081)"]
    CustFollower1["Custodian<br/>Follower<br/>(8081)"]
    CustFollower2["Custodian<br/>Follower<br/>(8081)"]
  end

  subgraph "DB Cluster"
    DBLeader["DB<br/>Leader<br/>(8080)"]
    DBFollower1["DB<br/>Follower<br/>(8080)"]
    DBFollower2["DB<br/>Follower<br/>(8080)"]
  end

  LBRP --- Auth
  LBRP --- Admin
  LBRP --- CustLeader
  LBRP --- CustFollower1
  LBRP --- CustFollower2
  LBRP --- DBLeader
  LBRP --- DBFollower1
  LBRP --- DBFollower2

  Auth --- Admin
  Auth --- CustLeader
  Auth --- CustFollower1
  Auth --- CustFollower2
  Auth --- DBLeader
  Auth --- DBFollower1
  Auth --- DBFollower2

  Admin --- CustLeader
  Admin --- CustFollower1
  Admin --- CustFollower2
  Admin --- DBLeader
  Admin --- DBFollower1
  Admin --- DBFollower2

  CustLeader --- CustFollower1
  CustLeader --- CustFollower2
  CustLeader --- DBLeader
  CustLeader --- DBFollower1
  CustLeader --- DBFollower2

  CustFollower1 --- CustFollower2
  CustFollower1 --- DBLeader
  CustFollower1 --- DBFollower1
  CustFollower1 --- DBFollower2

  CustFollower2 --- DBLeader
  CustFollower2 --- DBFollower1
  CustFollower2 --- DBFollower2

  DBLeader --- DBFollower1
  DBLeader --- DBFollower2

  DBFollower1 --- DBFollower2
```

### Unikernel Deployment (OPS)

Each service is packaged as a **Nanos unikernel** using OPS:

```bash
# Build service binary
cargo build --release -p db

# Create unikernel image
ops image create target/release/db \
  -c db-config.json \
  -t db-unikernel

# Run instance
ops instance create db-unikernel \
  -n db1 \
  -p 8080:8080
```

**Benefits**:
- Minimal attack surface (no shell, no unnecessary binaries)
- Fast boot times (~10ms)
- Small image size
- Immutable infrastructure

---

## Data Models

### Ticket

```rust
pub struct Ticket {
    pub id: u64,                          // Auto-incremented
    pub customer_ticket_number: Option<String>,
    pub isp_ticket_number: Option<String>,
    pub other_ticket_number: Option<String>,
    pub title: String,
    pub project: String,
    pub account_uuid: Uuid,
    pub symptom: Symptom,                 // u8 enum
    pub status: Status,                   // u8 enum
    pub next_action: NextAction,          // u8 enum
    pub resolution: Resolution,           // u8 enum
    pub lock: Option<LockInfo>,
    pub assigned_to: Option<UserId>,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_by: UserId,
    pub updated_at: DateTime<Utc>,
    pub history: Vec<TicketHistoryEntry>,
    pub ebond: Option<String>,
    
    // Billable hours tracking
    pub billable_hours: Vec<BillableSession>,
    pub total_billable_time: Duration,    // Sum of all sessions
    
    // Interaction metrics
    pub interaction_metrics: TicketMetrics,
}

/// Tracks time spent with ticket lock for billing
pub struct BillableSession {
    pub user_id: UserId,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,  // None if lock still held
    pub duration: Duration,               // Calculated when lock released
    pub billable: bool,                   // Some time may be non-billable
    pub notes: Option<String>,            // Optional billing notes
}

/// Metrics on user interactions with ticket
pub struct TicketMetrics {
    pub total_views: u32,                 // How many times opened
    pub unique_viewers: HashSet<UserId>,  // Unique users who viewed
    pub comments_count: u32,              // Number of comments posted
    pub comments_by_user: HashMap<UserId, u32>,  // Comments per user
    pub lock_acquisitions: u32,           // Times lock was acquired
    pub average_response_time: Duration,  // Avg time between status changes
    pub time_in_status: HashMap<Status, Duration>,  // Time spent in each status
}
```

### Enums (u8 for efficiency)

```rust
#[repr(u8)]
pub enum Symptom {
    Unknown = 0,
    BroadbandDown = 1,
    BroadbandIntermittent = 2,
    PacketLoss = 3,
    Power = 4,
    VpnIssue = 5,
    ConfigurationError = 6,
    HardwareFailure = 7,
    SoftwareBug = 8,
    SecurityIncident = 9,
    SlowBandwidth = 10,
    DuplexingMismatch = 11,
    LatencyIssues = 12,
    JitterProblems = 13,
    DnsIssues = 14,
    Other = 255,
}

#[repr(u8)]
pub enum Status {
    Open = 0,
    AwaitingCustomer = 1,
    AwaitingISP = 2,
    AwaitingPartner = 3,
    SupportHold = 4,
    HandedOff = 5,
    AppointmentScheduled = 6,
    EbondReceived = 7,
    VoicemailReceived = 8,
    AutoClose = 254,
    Closed = 255,
}

#[repr(u8)]
pub enum Resolution {
    None = 0,
    Resolved = 1,
    Workaround = 2,
    CannotReproduce = 3,
    WontFix = 4,
    Duplicate = 5,
}

pub enum NextAction {
    None,
    FollowUp { scheduled_at: DateTime<Utc> },
    Appointment { scheduled_at: DateTime<Utc> },
    AutoClose { close_at: DateTime<Utc>, timeframe: AutoCloseTimeframe },
}

pub enum AutoCloseTimeframe {
    EndOfDay,
    TwentyFourHours,
    FortyEightHours,
    SeventyTwoHours,
}
```

### User

```rust
pub struct User {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub full_name: String,
    pub role: Role,
    pub mfa_enabled: bool,
    pub mfa_methods: Vec<MfaMethod>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub active: bool,
}

pub enum Role {
    Admin,
    Manager,
    Supervisor,
    Technician,
    EbondPartner,
    ReadOnly,
}

pub enum MfaMethod {
    TOTP { secret: String },
    WebAuthn { credential_id: Vec<u8> },
    ActiveDirectory,
}
```

### Lock Info

```rust
pub struct LockInfo {
    pub user_id: UserId,
    pub locked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub instance_id: Uuid,  // Which custodian instance granted the lock
}
```

---

## API Specifications

### REST API Conventions

**Base URL**: `https://{lbrp-host}/api`

**Authentication**: Bearer token in `Authorization` header
```
Authorization: Bearer <jwt-token>
```

**Request/Response Format**: JSON

**Error Response**:
```json
{
  "error": {
    "code": "TICKET_NOT_FOUND",
    "message": "Ticket with ID 12345 does not exist",
    "details": {}
  }
}
```

### Custodian API

#### Create Ticket
```http
POST /api/ticket
Content-Type: application/json

{
  "title": "Customer cannot connect to VPN",
  "project": "ACME Corp",
  "account_uuid": "550e8400-e29b-41d4-a716-446655440000",
  "symptom": "NetworkOutage",
  "assigned_to": 42
}

Response 201:
{
  "ticket_id": 12345,
  "created_at": "2025-12-07T15:30:00Z"
}
```

#### Acquire Lock
```http
POST /api/ticket/12345/lock

Response 200:
{
  "locked": true,
  "expires_at": "2025-12-07T16:00:00Z"
}

Response 409 (already locked):
{
  "error": {
    "code": "TICKET_LOCKED",
    "message": "Ticket is already locked by user 'johndoe'",
    "details": {
      "locked_by": "johndoe",
      "locked_at": "2025-12-07T15:25:00Z"
    }
  }
}
```

#### Update Status
```http
PUT /api/ticket/12345/status
Content-Type: application/json

{
  "status": "Closed",
  "resolution": "Resolved",
  "comment": "VPN credentials reset, issue resolved"
}

Response 200:
{
  "success": true,
  "updated_at": "2025-12-07T15:45:00Z"
}
```

### Auth API

#### Login (Step 1: Password)
```http
POST /api/auth/login
Content-Type: application/json

{
  "username": "johndoe",
  "password": "SecurePass123!"
}

Response 200 (MFA required):
{
  "mfa_required": true,
  "mfa_methods": ["totp", "webauthn"],
  "session_id": "temp-session-abc123"
}
```

#### Login (Step 2: TOTP)
```http
POST /api/auth/login/totp
Content-Type: application/json

{
  "session_id": "temp-session-abc123",
  "totp_code": "123456"
}

Response 200:
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "refresh_token": "refresh-token-xyz789",
  "expires_in": 900
}
```

---

## Performance Architecture

### Concurrency Strategy

NGI leverages Rust's powerful concurrency primitives to maximize throughput and efficiency:

#### Asynchronous I/O with Tokio

**All services use async/await** with Tokio runtime for I/O-bound operations:

```rust
#[tokio::main]
async fn main() {
    // Multi-threaded runtime with work-stealing scheduler
    let app = build_app().await;
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

**Benefits of Async:**
- **High concurrency**: Handle thousands of connections with minimal memory overhead
- **Non-blocking I/O**: Network requests, database operations, and file I/O don't block threads
- **Efficient resource usage**: One thread can manage many concurrent tasks
- **Backpressure handling**: Tokio's channels provide natural flow control

**Async Use Cases in NGI:**
- HTTP request handling (Axum handlers are async)
- Database operations (Sled operations wrapped in async)
- Inter-service communication (async HTTP clients)
- Raft consensus operations (async state machine)
- Message passing (tokio::sync::mpsc channels)

#### Multi-Threading for CPU-Bound Work

**Tokio multi-threaded runtime** automatically parallelizes work across available CPU cores:

```rust
// Runtime configuration
tokio::runtime::Builder::new_multi_thread()
    .worker_threads(num_cpus::get())  // One thread per CPU core
    .thread_name("ngi-worker")
    .enable_all()
    .build()
    .unwrap()
```

**Parallel Processing Strategies:**

1. **Request parallelism**: Multiple HTTP requests processed simultaneously on different threads
2. **Database sharding**: Future enhancement to partition data across threads
3. **Background tasks**: Spawn blocking tasks for CPU-intensive operations:
   ```rust
   tokio::task::spawn_blocking(|| {
       // CPU-intensive work (encryption, compression, serialization)
       expensive_computation()
   }).await?;
   ```

4. **Batch processing**: Use `rayon` for data-parallel operations:
   ```rust
   use rayon::prelude::*;
   
   tickets.par_iter()
       .filter(|t| t.status == Status::Open)
       .map(|t| calculate_metrics(t))
       .collect()
   ```

#### Concurrency Limits and Tuning

**Connection pooling:**
- HTTP client pool: 100 connections per target service
- Database connections: Configurable per-service (default: 10)

**Rate limiting:**
- Per-user: 100 req/min
- Per-IP: 1000 req/min
- Global: 10,000 req/min

**Tokio task limits:**
- Prevent unbounded task spawning with semaphores
- Use tower's `load-shed` middleware to reject requests under extreme load

**Thread pool sizing:**
```toml
[runtime]
worker_threads = 8        # Number of worker threads (default: num CPUs)
max_blocking_threads = 32 # For spawn_blocking tasks
thread_keep_alive = 10    # Seconds to keep idle threads alive
```

### Performance Optimization Techniques

1. **Zero-copy where possible**: Use `Bytes` instead of `Vec<u8>` for network data
2. **Efficient serialization**: `bincode` for internal data, JSON only at API boundaries
3. **Connection reuse**: HTTP/2 multiplexing for inter-service communication
4. **Lazy evaluation**: Stream large result sets instead of loading into memory
5. **Smart caching**: In-memory LRU cache for frequently accessed tickets (future)

---

## Performance Targets

### MVP Goals

| Metric | Target | Notes |
|--------|--------|-------|
| **API Latency (p50)** | < 50ms | Read operations |
| **API Latency (p99)** | < 200ms | Write operations |
| **Throughput** | 1000 req/s | Per service instance |
| **Concurrent Users** | 500 | Total system |
| **Database Size** | 10GB | MVP dataset |
| **Ticket Creation** | < 100ms | End-to-end |
| **Lock Acquisition** | < 50ms | With Raft consensus |

### Scaling Plan (Post-MVP)

- Horizontal scaling: Add more instances
- Read replicas: Add DB followers for read-heavy workloads
- Caching: Option for session data and frequently accessed tickets
- Database sharding: Partition tickets by year or project

---

## Testing Strategy

### Unit Tests
- Per-function testing in each crate
- Mock external dependencies
- Target: 90% code coverage

### Integration Tests
- Test inter-service communication
- Located in `tests/` crate
- Use OPS to spin up services as unikernels

### Chaos Testing
- Use `chaos` service to inject faults
- Scenarios:
  - Leader failure (Raft recovery)
  - Follower lag/failure
  - Non-Raft service crash
  - Network partition
  - Disk full
  - High latency
  - CPU/memory pressure

### Load Testing
- Use `k6` or `wrk` for HTTP load generation
- Simulate 1000 concurrent users
- Measure latency, throughput, error rate

---

## Future Enhancements (Post-MVP)

1. **Active Directory Integration** (v1.0)
2. **Real-Time Notifications** (WebSocket push)
3. **Advanced Search** (Full-text search with Tantivy)
4. **Audit Logging** (Immutable log to separate storage)
5. **Multi-Tenancy** (Separate data per organization)

---

## Security Considerations

### Threat Model

| Threat | Mitigation |
|--------|------------|
| **SQL Injection** | No SQL; Rust-native API |
| **XSS** | Frontend sanitization; CSP headers |
| **CSRF** | SameSite cookies; CSRF tokens |
| **MITM** | TLS 1.3 + Kyber (double encryption) |
| **Brute Force** | Rate limiting; account lockout |
| **Privilege Escalation** | RBAC; principle of least privilege |
| **DoS** | Rate limiting; circuit breakers; redundancy |
| **Quantum Attacks** | Post-quantum cryptography (Kyber) |

### Compliance

- **GDPR**: User data deletion, data portability
- **SOC 2**: Audit logging, access controls
- **HIPAA**: Encryption at rest/in transit
- **NIST CSF**: Risk management, incident response
- **ISO 27001**: Information security management
- **CMMC**: Cybersecurity maturity for DoD contractors

---

## Maintenance & Operations

### Monitoring

All services expose:
- `GET /health` - Health check endpoint
- `GET /metrics` - Prometheus-compatible metrics

Metrics collected:
- Request count, latency (histogram)
- Error rate
- Raft leader status
- Database size, query time
- Lock contention

### Logging

Structured logging with `tracing` crate:
```rust
tracing::info!(
    ticket_id = 12345,
    user_id = "johndoe",
    "Ticket lock acquired"
);
```

Logs aggregated in `admin` service for centralized viewing.

### Backups

**Raft Snapshots (Automatic):**
- Raft snapshots every 1000 log entries
- Each DB instance maintains its own snapshots
- Used for fast node recovery and log compaction

**Filesystem-Level Backups (Daily):**
- Snapshot Sled data directory on each DB instance
- All 3+ instances have identical data (via Raft replication)
- **Recovery**: If one node fails, surviving nodes continue serving. Can restore failed node from any healthy node's snapshot.

**Off-Site Backups (Daily):**
- Export one instance's snapshot to separate storage (S3, NAS, tape)
- **Why needed despite replication?**
  - Protects against cluster-wide failures (datacenter outage, simultaneous hardware failure)
  - Protects against software bugs (bad data replicated to all nodes)
  - Enables point-in-time recovery (restore to state before corruption)
  - Disaster recovery (ransomware, fire, catastrophic failures)

**Retention**: 30 days of daily backups

**Recovery Scenarios:**
1. **Node(s) fail**: Remaining nodes continue normally (as long as quorum maintained); rebuild failed node(s) from any healthy node's snapshot
2. **All nodes but one fail**: Last node serves read-only mode (no quorum for writes); restore other nodes from snapshot to regain quorum
3. **All nodes fail**: Restore from off-site backup to new cluster
4. **Data corruption**: Restore all nodes from last-known-good off-site backup


### Updates

- **Zero-downtime deployment**: Rolling updates (one instance at a time)
- **Raft leadership transfer**: Gracefully transfer leadership before allowing the current leader to shut down
- **Database migrations**: Versioned schema with backward compatibility

---

## Glossary

- **Axum**: Web framework built on Tokio for building HTTP services
- **Kyber**: Post-quantum key encapsulation mechanism (CRYSTALS-Kyber)
- **mTLS**: Mutual TLS - both client and server authenticate each other with certificates
- **Raft**: Consensus algorithm for distributed systems, provides strong consistency
- **Sled**: Embedded key-value database written in pure Rust
- **TOTP**: Time-based One-Time Password (used in MFA with authenticator apps)
- **Unikernel**: Specialized, single-purpose OS image with minimal attack surface
- **WebAuthn**: Web Authentication API (FIDO2 standard) for hardware security keys

---

## Contact

For questions or clarifications, contact:

**Jonathan A. McCormick, Jr.**  
Email: jamccormick[at]dsrglobal[dot]com  
GitHub: JonathanMcCormickJr
X: Jonathan_M_Jr

---

*This document will be updated as the architecture evolves.*
