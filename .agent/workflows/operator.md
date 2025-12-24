---
description: Operator agent workflow for deploying and managing NGI services in production
---

# Operator Agent

## Role

The Operator Agent is responsible for:
- Deploying and managing NGI services in production
- Configuring service mesh and network communication
- Monitoring system health and performance
- Managing configuration and secrets
- Executing incident response and recovery procedures
- Optimizing resource usage and scaling

**Key Constraint:** Zero-downtime deployments with automatic failover and graceful degradation.

---

## Technology Stack

### Deployment Platform
- **OPS** - NanoVMs unikernel build tool
- **Nanos** - Secure unikernel operating system
- **Service Mesh** - Internal load balancing and service discovery

### Infrastructure
- **mTLS** - Mutual TLS for all inter-service communication
- **Raft Consensus** - Distributed coordination for critical services
- **Sled** - Embedded database with ACID transactions

### Monitoring & Observability
- **Metrics** - Performance and health monitoring
- **Tracing** - Distributed request tracing
- **Logging** - Structured logging with `tracing`

---

## Deployment Architecture

### Service Topology
```
Internet → LBRP (443) → Internal Services (808x)
                     ↓
            ┌─────────────┐
            │  Service    │
            │ Discovery   │
            └─────────────┘
                    │
          ┌─────────┼─────────┐
          │         │         │
    ┌─────▼───┐ ┌───▼────┐ ┌──▼────┐
    │  DB     │ │Custodian│ │ Auth  │
    │ (Raft)  │ │ (Raft)  │ │(Stateless)
    └─────────┘ └────────┘ └───────┘
          │         │         │
    ┌─────▼─────────▼─────────▼────┐
    │         Admin & Monitor      │
    └──────────────────────────────┘
```

### Port Assignments
- **LBRP:** 443 (HTTPS) / 80 (HTTP redirect)
- **DB:** 8080 (gRPC)
- **Custodian:** 8081 (gRPC)
- **Auth:** 8082 (gRPC)
- **Admin:** 8083 (gRPC)
- **Chaos:** 8084 (gRPC)
- **Honeypot:** 8085 (gRPC)

---

## Deployment Process

### Build Process
```bash
# 1. Run full test suite
cargo test
cargo tarpaulin --out Xml

# 2. Security audit
cargo audit

# 3. Build optimized release
cargo build --release

# 4. Create unikernel images
ops build -c config.json
```

### Configuration Files
```json
{
  "BaseVolumeSz": "5GB",
  "Boot": "target/release/db",
  "Args": ["--raft-node-id", "1", "--raft-peers", "db1:8080,db2:8080,db3:8080"],
  "Dirs": ["data"],
  "Files": ["config/tls.crt", "config/tls.key"],
  "MapDirs": {
    "data": "persistent-data"
  },
  "Env": {
    "RUST_LOG": "info",
    "NGI_ENV": "production"
  }
}
```

### Service Deployment
```bash
# Deploy DB cluster (3 instances for quorum)
ops instance create db1 -c db-config.json -i db-image
ops instance create db2 -c db-config.json -i db-image
ops instance create db3 -c db-config.json -i db-image

# Deploy Custodian cluster (3 instances for quorum)
ops instance create custodian1 -c custodian-config.json -i custodian-image
ops instance create custodian2 -c custodian-config.json -i custodian-image
ops instance create custodian3 -c custodian-config.json -i custodian-image

# Deploy stateless services
ops instance create auth -c auth-config.json -i auth-image
ops instance create admin -c admin-config.json -i admin-image

# Deploy LBRP with TLS certificates
ops instance create lbrp -c lbrp-config.json -i lbrp-image
```

---

## Configuration Management

### Environment Variables
```bash
# Core Configuration
RUST_LOG=info,ngi=debug
NGI_ENV=production

# Service Discovery
NGI_DB_ENDPOINTS=https://db1:8080,https://db2:8080,https://db3:8080
NGI_CUSTODIAN_ENDPOINTS=https://custodian1:8081,https://custodian2:8081

# TLS Configuration
TLS_CERT_PATH=/etc/ssl/certs/ngi.crt
TLS_KEY_PATH=/etc/ssl/private/ngi.key
CA_CERT_PATH=/etc/ssl/certs/ca.crt

# Security
POST_QUANTUM_ENABLED=true
MTLS_REQUIRED=true
```

### Secrets Management
- **TLS Certificates:** Stored securely, rotated automatically
- **Service Keys:** Managed separately from configuration
- **Database Encryption:** Keys derived from service identity
- **API Keys:** For external integrations (when needed)

---

## Service Mesh & Communication

### mTLS Configuration
```rust
// All inter-service communication uses mTLS
let client_config = rustls::ClientConfig::builder()
    .with_safe_defaults()
    .with_root_certificates(root_store)
    .with_client_auth_credentials(client_certs)
    .build()?;

let channel = Channel::from_static("https://db-leader:8080")
    .tls_config(tonic::transport::ClientTlsConfig::new()
        .rustls_client_config(client_config))?
    .connect()
    .await?;
```

### Leader-Aware Routing
```rust
async fn find_leader(endpoints: &[String]) -> Result<String> {
    for endpoint in endpoints {
        if is_leader(endpoint).await? {
            return Ok(endpoint.clone());
        }
    }
    Err(Error::NoLeaderFound)
}

// Route critical operations to current leader
let leader = find_leader(&custodian_endpoints).await?;
let mut client = CustodianClient::connect(leader).await?;
```

### Load Balancing
- **Stateless Services:** Round-robin across instances
- **Consensus Services:** Route to leader for writes, any node for reads
- **Health Checks:** Automatic removal of unhealthy instances

---

## Monitoring & Observability

### Health Checks
```rust
// Readiness probe
async fn readiness_check() -> Result<(), Status> {
    // Check database connectivity
    db_client.health_check().await?;

    // Check Raft cluster status
    let metrics = raft.metrics().borrow();
    if !matches!(metrics.state, ServerState::Leader | ServerState::Follower) {
        return Err(Status::unavailable("Not part of healthy cluster"));
    }

    Ok(())
}

// Liveness probe
async fn liveness_check() -> Result<(), Status> {
    // Basic service responsiveness
    tokio::time::timeout(
        Duration::from_secs(5),
        async { /* service operation */ }
    ).await?;
    Ok(())
}
```

### Metrics Collection
```rust
// Core metrics to collect
let metrics = Metrics {
    // Request metrics
    requests_total: counter!("ngi_requests_total"),
    request_duration: histogram!("ngi_request_duration_seconds"),

    // System metrics
    memory_usage: gauge!("ngi_memory_usage_bytes"),
    cpu_usage: gauge!("ngi_cpu_usage_percent"),

    // Business metrics
    tickets_created: counter!("ngi_tickets_created_total"),
    locks_acquired: counter!("ngi_locks_acquired_total"),

    // Raft metrics
    raft_state: gauge!("ngi_raft_state"),
    raft_commit_index: gauge!("ngi_raft_commit_index"),
};
```

### Distributed Tracing
```rust
use tracing::{info_span, instrument};

#[instrument(name = "create_ticket", fields(ticket_id))]
async fn create_ticket(request: CreateTicketRequest) -> Result<Ticket> {
    let span = info_span!("validate_request");
    let _guard = span.enter();

    // Validation logic
    validate_request(&request).await?;

    drop(_guard);
    let span = info_span!("persist_ticket");
    let _guard = span.enter();

    // Persistence logic
    let ticket = db_client.create_ticket(request).await?;

    info!(ticket.id = %ticket.id, "ticket created successfully");
    Ok(ticket)
}
```

---

## Incident Response

### Common Failure Scenarios

#### Raft Leader Failure
```bash
# 1. Detect leader failure (monitoring alerts)
# 2. Automatic leader election occurs
# 3. Verify new leader is healthy
curl -k https://db-leader:8080/health

# 4. Check cluster status
curl -k https://admin:8083/cluster/status

# 5. Verify service availability
curl -k https://lbrp:443/api/health
```

#### Network Partition
```bash
# 1. Detect partition (increased latency/errors)
# 2. Identify affected nodes
# 3. Raft handles split-brain prevention
# 4. Monitor for automatic healing
# 5. Manual intervention if needed
```

#### Certificate Expiration
```bash
# 1. Monitor certificate expiry (alerts 30 days before)
# 2. Generate new certificates
# 3. Rolling update of services
# 4. Verify mTLS still works
# 5. Clean up old certificates
```

### Recovery Procedures

#### Database Recovery
```bash
# 1. Stop failed instance
ops instance stop db-failed

# 2. Restore from healthy node snapshot
ops instance create db-new -c db-config.json \
  --restore-from db-healthy:/data/snapshot

# 3. Join cluster
curl -X POST https://db-new:8080/cluster/join \
  -H "Content-Type: application/json" \
  -d '{"leader": "https://db-leader:8080"}'

# 4. Verify cluster health
curl https://admin:8083/cluster/status
```

#### Full System Recovery
```bash
# 1. Identify root cause
# 2. Isolate affected components
# 3. Restore from backups if needed
# 4. Rolling restart of services
# 5. Verify end-to-end functionality
```

---

## Scaling & Performance

### Horizontal Scaling
```bash
# Add new stateless service instance
ops instance create auth-new -c auth-config.json -i auth-image

# Service discovery automatically includes new instance
# Load balancer distributes traffic
```

### Vertical Scaling
```bash
# Increase resources for memory-intensive service
ops instance update db-leader \
  --memory 4GB \
  --cpu 2 \
  --restart
```

### Performance Optimization
- **Connection Pooling:** Reuse gRPC connections
- **Batch Operations:** Combine multiple requests
- **Caching:** Cache frequently accessed data
- **Async Processing:** Non-blocking operations throughout

---

## Maintenance Procedures

### Certificate Rotation
```bash
# 1. Generate new certificates
./scripts/generate-certs.sh

# 2. Update configuration
./scripts/update-config.sh

# 3. Rolling restart
./scripts/rolling-restart.sh

# 4. Verify
./scripts/verify-mtls.sh
```

### Database Maintenance
```bash
# 1. Create snapshot
curl -X POST https://db-leader:8080/admin/snapshot

# 2. Compact logs
curl -X POST https://db-leader:8080/admin/compact

# 3. Verify integrity
curl https://db-leader:8080/admin/verify
```

### Log Rotation
```bash
# Logs automatically rotated by Nanos
# Archive old logs for compliance
./scripts/archive-logs.sh

# Compress and encrypt
./scripts/compress-logs.sh
```

---

## Security Operations

### Access Control
- **Network Security:** All inter-service traffic encrypted with mTLS
- **Authentication:** Service-to-service authentication required
- **Authorization:** Role-based access control enforced
- **Audit Logging:** All operations logged for compliance

### Threat Detection
- **Honeypot Service:** Detects and logs intrusion attempts
- **Anomaly Detection:** Unusual patterns trigger alerts
- **Rate Limiting:** Prevents abuse and DoS attacks

### Compliance
- **Data Retention:** Soft deletes maintain audit trails
- **Encryption:** Post-quantum encryption for sensitive data
- **Access Logging:** All operations logged with user context

---

## Troubleshooting

### Common Issues

#### Service Unavailable
```bash
# Check service health
curl -k https://service:808x/health

# Check logs
ops instance logs service-name

# Check network connectivity
telnet service.internal 808x
```

#### High Latency
```bash
# Check system resources
ops instance stats service-name

# Check database performance
curl -k https://db-leader:8080/metrics

# Profile with tracing
curl -k https://admin:8083/debug/pprof/profile
```

#### Certificate Issues
```bash
# Verify certificate validity
openssl x509 -in /etc/ssl/certs/ngi.crt -text -noout

# Test mTLS connection
openssl s_client -connect service:808x -cert client.crt -key client.key
```

---

## Development Environment

### Local Multi-Node Setup
```bash
# Start local database cluster
docker-compose up -d db-cluster

# Start development services
cargo run --bin db -- --dev-mode
cargo run --bin auth -- --dev-mode

# Run integration tests
cargo test --test integration_test
```

### Debugging Tools
- **Tracing:** `RUST_LOG=debug cargo run`
- **Metrics:** `curl localhost:8080/metrics`
- **Health Checks:** `curl localhost:8080/health`
- **Database Inspection:** Direct Sled file access in development

---

## References

- [OPS Documentation](https://github.com/nanovms/ops)
- [Nanos Unikernel](https://github.com/nanovms/nanos)
- [Raft Consensus](https://raft.github.io/)
- [mTLS Best Practices](https://tools.ietf.org/html/rfc8446)</content>
<parameter name="filePath">/home/jonathan/projects/ngi/.github/agents/operator.agent.md