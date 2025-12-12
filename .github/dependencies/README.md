# NGI Dependencies Reference

> Comprehensive documentation for all 13 major dependencies used in NGI, organized by dependency with cross-cutting concepts.

**Last Updated:** December 2025
**Quality:** Production-ready for implementor agent training

## What's Here

This directory contains **~5,100+ lines** of high-quality, official dependency documentation organized by dependency folder. All documentation is sourced directly from the latest docs.rs pages and organized for easy navigation.

### Directory Structure

```
.github/dependencies/
├── README.md (This file - quick navigation)
├── INDEX.md (Detailed index of all documentation)
│
├── _concepts/ (Cross-cutting concepts)
│   ├── README.md
│   ├── error-handling.md (thiserror & anyhow)
│   ├── security.md (rustls & pqc_kyber)
│   └── serialization.md (serde & bincode)
│
├── tokio/ (Async runtime)
│   ├── README.md
│   └── core-concepts.md
├── sled/ (Embedded database)
│   ├── README.md
│   └── architecture.md
├── bincode/ (Binary serialization)
│   └── README.md
├── serde/ (Serialization framework)
│   └── README.md
├── tonic/ (gRPC framework)
│   ├── README.md
│   └── framework.md
├── axum/ (REST framework)
│   ├── README.md
│   └── framework.md
├── openraft/ (Consensus)
│   ├── README.md
│   └── consensus.md
├── rustls/ (TLS 1.3 security)
│   └── README.md
├── pqc_kyber/ (Post-quantum crypto)
│   └── README.md
├── thiserror/ (Error types)
│   └── README.md
├── anyhow/ (Error context)
│   └── README.md
├── ops/ (Unikernel deployment)
│   └── README.md
└── nanos/ (Unikernel OS)
    └── README.md
```

## Related Documentation

### Agent Guidance
- **[Implementor Agent](../agents/implementor.agent.md)** - Code implementation patterns and TDD workflow
- **[Tester Agent](../agents/tester.agent.md)** - Testing strategies and quality assurance
- **[Operator Agent](../agents/operator.agent.md)** - Deployment, monitoring, and operations

### Development Resources
- **[Main README](../../README.md)** - Project overview, setup, and operations
- **[Architecture](../../ARCHITECTURE.md)** - System design and service interactions

### Each Dependency Folder Includes

1. **README.md** - Overview, NGI integration, and documentation index
2. **Detailed section files** - Complete implementation guides
3. **Official API documentation** - Links and examples
4. **NGI-specific patterns** - Real usage from the codebase
5. **Best practices** - Do's and don'ts for NGI

## Quick Navigation

### Runtime & Concurrency
- [Tokio](tokio/) - Async runtime with channels and synchronization
- See also: [Tokio core concepts](tokio/core-concepts.md)

### Data Persistence
- [Sled](sled/) - Embedded ACID key-value database
- Patterns: Prefixed keys, transactions, secondary indexes

### Consensus & Distribution
- [OpenRaft](openraft/) - Raft consensus for DB and Custodian services
- Key concepts: State machines, log replication, leader election

### Service Communication
- [Tonic](tonic/) - gRPC framework for inter-service communication
- [Axum](axum/) - REST API framework for LBRP service only

### Data Handling
- [Serialization](_concepts/serialization.md) - Type-safe serialization (serde & bincode)
- Patterns: Derive macros, custom serialization, storage

### Error Management
- [Error Handling](_concepts/error-handling.md) - Typed errors and context (thiserror & anyhow)
- Pattern: Domain errors with thiserror, context with anyhow

### Security
- [Security](_concepts/security.md) - TLS 1.3 and post-quantum cryptography (rustls & pqc_kyber)
- Pattern: Double-layer encryption (transport + application)

### Deployment
- [OPS](ops/) - Unikernel build and deployment tool
- [Nanos](nanos/) - Unikernel operating system

## NGI-Specific Patterns

### Architecture
- **Consensus Services** (DB, Custodian): Use OpenRaft + Sled
- **Stateless Services** (Auth, Admin, LBRP): No inter-service coordination
- **All Communication**: gRPC with mTLS (tonic + rustls)
- **Client Gateway**: LBRP exposes REST/JSON via Axum to public clients

### Data Flow
```
Client → LBRP (Axum/REST) → Service (gRPC/Tonic)
         ↓
    [mTLS via rustls]
         ↓
    Backend Service
    (Sled storage)
         ↓
    [Raft consensus for distributed services]
```

### Error Flow
```
Service Error (thiserror)
    ↓
Add Context (anyhow)
    ↓
Convert to gRPC Status
    ↓
Convert to HTTP Status in Axum
    ↓
JSON Response to Client
```

## Version Matrix

All versions are from the workspace defined in `/Cargo.toml`:

| Crate | Version | Status | Service(s) |
|-------|---------|--------|-----------|
| tokio | 1.48 | ✅ Implemented | All services |
| tonic | 0.14 | ✅ Implemented | DB service |
| prost | 0.14 | ✅ Implemented | DB service |
| openraft | 0.9 | ✅ Implemented | DB service |
| sled | 0.34 | ✅ Implemented | DB service |
| serde | 1.0 | ✅ Implemented | Shared, DB services |
| bincode | 2 | ✅ Implemented | DB service |
| thiserror | 2.0 | ✅ Implemented | Shared, DB services |
| anyhow | 1.0 | ✅ Implemented | DB service |
| tracing | 0.1 | ✅ Implemented | DB service |
| chrono | 0.4 | ✅ Implemented | Shared, DB services |
| uuid | 1.10 | ✅ Implemented | Shared service |
| axum | 0.7 | 📋 Planned | LBRP service |
| rustls | 0.23 | 📋 Planned | All services |
| pqc_kyber | 0.7 | 📋 Planned | All services |

## Common Implementation Patterns

### Writing a New gRPC Endpoint
1. Define service in `.proto` file
2. Run `cargo build` to generate Rust types
3. Implement handler using Tonic (see [tonic/framework.md](tonic/framework.md))
4. Use gRPC client to call upstream services
5. Convert errors to Status codes
6. Add unit tests with ≥90% coverage

### Storing Data in Sled
1. Define data types with `#[derive(Serialize, Deserialize)]`
2. Use prefixed keys for tables (see [sled/architecture.md](sled/architecture.md))
3. Create secondary indexes for queries
4. Use transactions for multi-key operations
5. Implement soft deletes (set `deleted` flag, never hard-delete)

### Adding REST Endpoint to LBRP
1. Add route to Axum router (see [axum/framework.md](axum/framework.md))
2. Create handler that calls gRPC backend
3. Convert gRPC response to JSON
4. Add error handling for all failure cases
5. Document endpoint with examples

### Distributing with Raft
1. Implement state machine (see [openraft/consensus.md](openraft/consensus.md))
2. Define command types (enum)
3. Implement `apply()` method
4. Route write operations through leader (LBRP)
5. Test with chaos injection

## Error Handling Checklist

- [ ] Define custom error type with thiserror
- [ ] Use `#[error(...)]` attributes for messages
- [ ] Implement `From<ExternalError>` for each dependency
- [ ] Add context at each layer with anyhow
- [ ] Convert to gRPC Status at service boundary
- [ ] Log errors with structured logging
- [ ] Never `.unwrap()` in production code
- [ ] Test error paths with ≥90% coverage

## Security Checklist

- [ ] All inter-service communication uses mTLS (rustls)
- [ ] Certificates validated against CA
- [ ] gRPC channels encrypted with TLS 1.3
- [ ] Sensitive data encrypted with Kyber (application layer)
- [ ] Private keys stored securely (not in code)
- [ ] All operations logged and auditable
- [ ] Timeouts set on all network operations

## Testing Requirements

- [ ] Unit tests for all business logic
- [ ] Integration tests with real services
- [ ] Distributed tests for Raft consensus
- [ ] Error path tests
- [ ] Coverage report ≥90%
- [ ] Run `cargo test && cargo tarpaulin`

## Performance Considerations

- Tokio: Multiple worker threads by default
- Sled: Lock-free reads are very fast
- gRPC: HTTP/2 multiplexing, streaming support
- Serialization: bincode is compact, fast
- Raft: Log replication overhead on followers

## Next Steps

1. Pick a dependency from above
2. Read its README.md for overview
3. Follow links to detailed section files
4. Review NGI-specific patterns
5. Check example code in the services
6. Implement with tests
7. Verify coverage with `cargo tarpaulin`

## Questions?

- Check the relevant dependency README first
- Look at existing code in similar services
- Review ARCHITECTURE.md for service responsibilities
- Ask clarifying questions rather than guessing
