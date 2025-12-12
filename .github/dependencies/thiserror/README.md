# thiserror - Derive Macros for Error Types

> Derive macro for implementing `std::error::Error` with less boilerplate.

**Official Docs:** https://docs.rs/thiserror/latest/thiserror/

**Current Version:** 2.0.0+

## Overview

thiserror provides `#[derive(Error)]` macro to reduce boilerplate when defining custom error types. NGI uses it for domain-specific errors (ticket, lock, storage, etc.).

## Usage in NGI

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket not found: {0}")]
    NotFound(u64),
    
    #[error("ticket already locked by {user}")]
    AlreadyLocked { user: String },
    
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("invalid status transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },
    
    #[error(transparent)]
    StorageError(#[from] sled::Error),
}

// Automatically implements Display, Debug, Error traits
// + From<sled::Error> via #[from]

fn main() -> Result<(), TicketError> {
    let ticket = fetch_ticket(42)?;  // Returns TicketError::NotFound
    Ok(())
}
```

## Attribute Reference

### Error Messages
```rust
#[derive(Error, Debug)]
enum MyError {
    // Simple message
    #[error("something went wrong")]
    Simple,
    
    // Message with field interpolation
    #[error("value {} is invalid", .0)]
    InvalidValue(String),
    
    // Message with named fields
    #[error("{field} field required: {reason}")]
    MissingField { field: String, reason: String },
}
```

### Error Chaining (#[from])
```rust
#[derive(Error, Debug)]
enum ApiError {
    // Automatically converts sled::Error → ApiError
    #[from]
    Database(sled::Error),
    
    // Automatically converts std::io::Error → ApiError
    #[from]
    Io(std::io::Error),
    
    // Custom conversion
    #[from]
    ParseInt(#[from] std::num::ParseIntError),
}

// Usage:
async fn read_config() -> Result<Config, ApiError> {
    let bytes = std::fs::read("config.json")?;  // Converts std::io::Error
    sled::open("./db")?;  // Converts sled::Error
    Ok(...)
}
```

### Transparent Errors (Pass-Through)
```rust
#[derive(Error, Debug)]
enum AppError {
    // Unwrap underlying error completely
    #[error(transparent)]
    Inner(Box<dyn std::error::Error>),
}
```

## NGI Error Hierarchy

```rust
// shared/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NgError {
    #[error("ticket error: {0}")]
    Ticket(#[from] TicketError),
    
    #[error("lock error: {0}")]
    Lock(#[from] LockError),
    
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error>),
}

#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket {id} not found")]
    NotFound { id: u64 },
    
    #[error("ticket already locked")]
    AlreadyLocked,
    
    #[error(transparent)]
    Database(#[from] sled::Error),
}

#[derive(Error, Debug)]
pub enum LockError {
    #[error("cannot acquire lock: {reason}")]
    AcquisitionFailed { reason: String },
    
    #[error("lock held by {user_id}")]
    AlreadyLocked { user_id: u64 },
}

// Usage
fn get_ticket(id: u64) -> Result<Ticket, TicketError> {
    db.get(&format!("ticket:{}", id))?  // sled::Error → TicketError
        .ok_or(TicketError::NotFound { id })
}
```

## Integration with anyhow/Result

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket not found")]
    NotFound,
}

// Combine with anyhow for context
fn main() -> anyhow::Result<()> {
    get_ticket(42)
        .map_err(|e| anyhow::anyhow!(e))  // Convert to anyhow::Error
        .context("failed to fetch ticket")?;
    Ok(())
}
```

## Testing Error Handling

```rust
#[test]
fn test_error_messages() {
    let err = TicketError::NotFound { id: 42 };
    assert_eq!(err.to_string(), "ticket 42 not found");
}

#[tokio::test]
async fn test_error_propagation() {
    let result: Result<_, TicketError> = Err(TicketError::NotFound { id: 1 });
    assert!(matches!(result, Err(TicketError::NotFound { id: 1 })));
}
```

## vs Manual Implementation

### Without thiserror
```rust
pub enum TicketError {
    NotFound(u64),
}

impl std::fmt::Display for TicketError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TicketError::NotFound(id) => write!(f, "ticket {} not found", id),
        }
    }
}

impl std::fmt::Debug for TicketError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // ...
    }
}

impl std::error::Error for TicketError {}
```

### With thiserror
```rust
#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket {0} not found")]
    NotFound(u64),
}
```

---

**Last Updated:** December 2025  
**Documentation Version:** thiserror 2.0.0
