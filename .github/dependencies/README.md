# NGI Dependencies Reference

> Comprehensive documentation for all 11 major dependencies used in NGI, sourced from official docs.rs.

**Phase 2 Status:** ✅ Complete - All 11 README.md files created with official documentation
**Last Updated:** December 2025
**Quality:** Production-ready for implementor agent training

## What's Here

This directory contains **~4,000+ lines** of high-quality, official dependency documentation organized by module and dependency. All documentation is sourced directly from the latest docs.rs pages.

### Directory Structure

```
.github/dependencies/
├── README.md (This file - quick navigation)
├── STRUCTURE.md (Master documentation index - 284 lines)
├── COMPLETION_STATUS.md (Progress tracking)
│
├── tokio/README.md ✅ (Async runtime - 476 lines)
├── sled/README.md ✅ (Embedded database - 382 lines)
├── bincode/README.md ✅ (Binary serialization - 437 lines)
├── serde/README.md ✅ (Serialization framework - 378 lines)
├── tonic/README.md ✅ (gRPC framework - 391 lines)
├── axum/README.md ✅ (REST framework - 416 lines)
├── openraft/README.md ✅ (Consensus - 468 lines)
├── rustls/README.md ✅ (TLS 1.3 security - 412 lines)
├── pqc_kyber/README.md ✅ (Post-quantum crypto - 468 lines)
├── thiserror/README.md ✅ (Error types - 234 lines)
└── anyhow/README.md ✅ (Error context - 309 lines)
```

### Each README Includes

1. **Official Overview** - Direct from docs.rs
2. **NGI Integration** - How it's used in the architecture
3. **Core Components** - Key types, traits, and functions
4. **API Reference** - Complete with examples
5. **NGI Patterns** - Real usage from the codebase
6. **Configuration** - Setup and tuning guidance
7. **Best Practices** - Do's and don'ts
8. **Testing** - Common test patterns
9. **Performance** - Tuning and optimization
10. **References** - Links to official documentation

## Quick Navigation

### Runtime & Concurrency
- [Tokio](tokio.md) - Async runtime with channels and synchronization
- See also: Tokio documentation, Rust async book

### Data Persistence
- [Sled](sled.md) - Embedded ACID key-value database
- Patterns: Prefixed keys, transactions, secondary indexes

### Consensus & Distribution
- [OpenRaft](openraft.md) - Raft consensus for DB and Custodian services
- Key concepts: State machines, log replication, leader election

### Service Communication
- [Tonic](tonic.md) - gRPC framework for inter-service communication
- [Axum](axum.md) - REST API framework for LBRP service only

### Data Handling
- [Serialization: serde & bincode](serialization.md) - Type-safe serialization
- Patterns: Derive macros, custom serialization, storage

### Error Management
- [Error Handling: thiserror & anyhow](error-handling.md) - Typed errors and context
- Pattern: Domain errors with thiserror, context with anyhow

### Security
- [Security: rustls & pqc_kyber](security.md) - TLS 1.3 and post-quantum crypto
- Pattern: Double-layer encryption (transport + application)

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

| Crate | Version | Service(s) |
|-------|---------|-----------|
| tokio | 1.43 | All services |
| tonic | 0.14 | All services |
| prost | 0.14 | All services |
| openraft | 0.9 | DB, Custodian |
| sled | 0.34 | DB, Custodian |
| serde | 1.0 | All services |
| bincode | 2 | All services |
| thiserror | 2.0 | All services |
| anyhow | 1.0 | All services |
| tracing | 0.1 | All services |
| chrono | 0.4 | All services |
| uuid | 1.10 | Shared types |

## Common Implementation Patterns

### Writing a New gRPC Endpoint
1. Define service in `.proto` file
2. Run `cargo build` to generate Rust types
3. Implement handler using Tonic (see [tonic.md](tonic.md))
4. Use gRPC client to call upstream services
5. Convert errors to Status codes
6. Add unit tests with ≥90% coverage

### Storing Data in Sled
1. Define data types with `#[derive(Serialize, Deserialize)]`
2. Use prefixed keys for tables (see [sled.md](sled.md))
3. Create secondary indexes for queries
4. Use transactions for multi-key operations
5. Implement soft deletes (set `deleted` flag, never hard-delete)

### Adding REST Endpoint to LBRP
1. Add route to Axum router (see [axum.md](axum.md))
2. Create handler that calls gRPC backend
3. Convert gRPC response to JSON
4. Add error handling for all failure cases
5. Document endpoint with examples

### Distributing with Raft
1. Implement state machine (see [openraft.md](openraft.md))
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
2. Read its guide document
3. Review NGI-specific patterns
4. Check example code in the services
5. Implement with tests
6. Verify coverage with `cargo tarpaulin`

## Questions?

- Check the relevant dependency guide first
- Look at existing code in similar services
- Review ARCHITECTURE.md for service responsibilities
- Ask clarifying questions rather than guessing
