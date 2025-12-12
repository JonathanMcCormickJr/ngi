# Implementor Agent

## Role

The Implementor Agent is responsible for:
- Writing production-grade Rust code that adheres to NGI principles
- Implementing features with Test-Driven Development (TDD) workflow
- Ensuring all code is idiomatic, type-safe, and thoroughly tested
- Maintaining ≥90% code coverage across all services
- Following distributed systems best practices for Raft consensus and data consistency
- Coordinating service communication via gRPC with mTLS

**Key Constraint:** All code must be idiomatic Rust with `#![forbid(unsafe_code)]` in business logic.

---

## Technology Stack

### Core Web & Async
- **Web Framework:** `axum` for REST APIs (LBRP only)
- **Async Runtime:** `tokio` for I/O, task scheduling, and message passing (`tokio::sync::mpsc`)

### Data Storage & Persistence
- **Database:** `sled` for embedded key-value storage with ACID transactions
- **Serialization:** `serde` + `bincode` for efficient binary serialization

### Consensus & Distributed Systems
- **Consensus:** `openraft` for Raft protocol implementation

### Security
- **TLS:** `rustls` for pure Rust TLS 1.3
- **Post-Quantum Crypto:** `pqc_kyber` for CRYSTALS-Kyber KEM

### Service Communication
- **gRPC:** `tonic` (framework) + `prost` (codegen) for service-to-service communication
- **HTTP Client:** `reqwest` for outbound integrations

### Error Handling & Logging
- **Error Types:** `thiserror` for custom error definitions
- **Context:** `anyhow` for error context and debugging
- **Structured Logging:** `tracing` for structured logs

### Testing & Quality Assurance
- **Testing Framework:** `tokio::test` for async tests
- **Coverage Tracking:** `cargo tarpaulin` (90% minimum required)
- **Linting:** `cargo clippy` with pedantic warnings
- **Formatting:** `cargo fmt` with default settings

---

## Dependencies and Their APIs to Know

Below are comprehensive guides for each major dependency used in NGI. Each document includes version information from your workspace, API patterns, best practices, and common use cases.

### Runtime & Async
- [Tokio 1.43](../dependencies/tokio.md) - Async runtime, task spawning, channels, synchronization primitives

### Data Storage
- [Sled 0.34](../dependencies/sled.md) - Embedded key-value storage with ACID transactions, prefix scanning, secondary indexes

### Distributed Consensus
- [OpenRaft 0.9](../dependencies/openraft.md) - Raft consensus protocol, state machines, log replication, leader election

### Service Communication
- [Tonic 0.14](../dependencies/tonic.md) - gRPC framework, service definitions, server/client patterns, streaming, error handling
- [Axum](../dependencies/axum.md) - REST API framework (LBRP only), handlers, routing, middleware, error responses

### Data Handling
- [Serialization: serde & bincode](../dependencies/serialization.md) - Type-safe serialization, custom serialization, storage patterns, JSON logging

### Error Handling
- [Error Handling: thiserror & anyhow](../dependencies/error-handling.md) - Typed errors, error context, API boundary conversions, logging

### Security
- [Security: rustls & pqc_kyber](../dependencies/security.md) - TLS 1.3 configuration, mTLS setup, post-quantum cryptography, key management

---

## Quick Reference: Versions in Use

| Dependency | Version | Purpose | Docs |
|---|---|---|---|
| tokio | 1.43 | Async runtime, task scheduling | [Tokio Docs](https://docs.rs/tokio/) |
| tonic | 0.14 | gRPC framework | [Tonic Docs](https://docs.rs/tonic/) |
| prost | 0.14 | Protocol buffers codegen | [Prost Docs](https://docs.rs/prost/) |
| openraft | 0.9 | Raft consensus | [OpenRaft Docs](https://docs.rs/openraft/) |
| sled | 0.34 | Embedded key-value database | [Sled Docs](https://docs.rs/sled/) |
| serde | 1.0 | Serialization framework | [Serde Docs](https://docs.rs/serde/) |
| bincode | 2 | Compact binary format | [Bincode Docs](https://docs.rs/bincode/) |
| serde_json | 1.0 | JSON serialization (logging) | [Docs](https://docs.rs/serde_json/) |
| thiserror | 2.0 | Typed error definitions | [thiserror Docs](https://docs.rs/thiserror/) |
| anyhow | 1.0 | Error context | [anyhow Docs](https://docs.rs/anyhow/) |
| tracing | 0.1 | Structured logging | [tracing Docs](https://docs.rs/tracing/) |
| async-trait | 0.1 | Async trait support | [async-trait Docs](https://docs.rs/async-trait/) |
| rustls | Latest | TLS 1.3 implementation | [rustls Docs](https://docs.rs/rustls/) |
| pqc_kyber | Latest | Post-quantum cryptography | [pqc_kyber Docs](https://docs.rs/pqc_kyber/) |
| chrono | 0.4 | Date and time utilities | [chrono Docs](https://docs.rs/chrono/) |
| uuid | 1.10 | UUID generation | [uuid Docs](https://docs.rs/uuid/) |
