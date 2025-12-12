# NGI Dependency Documentation - Concepts Index

Cross-cutting concerns shared across multiple dependencies in NGI:

## Available Guides

- **[Error Handling](error-handling.md)** - Patterns for thiserror & anyhow
  - Custom error types with thiserror
  - Error context with anyhow
  - API boundary conversions
  - Error chains and recovery

- **[Security](security.md)** - TLS 1.3 and post-quantum cryptography
  - rustls for transport security
  - pqc_kyber for long-term confidentiality
  - Certificate management
  - Secure key handling

- **[Serialization](serialization.md)** - Binary encoding strategies
  - bincode for efficient binary format
  - serde for serialization
  - Efficiency optimization

## Related Dependency Documentation

See the root-level dependency READMEs for detailed documentation on each crate.
