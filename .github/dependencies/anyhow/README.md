# anyhow - Flexible Error Handling

> Flexible concrete error type built on std::error::Error with support for error messages and custom error types.

**Official Docs:** https://docs.rs/anyhow/latest/anyhow/

**Current Version:** 1.0.0+

## Overview

anyhow provides `Result<T>` (shorthand for `Result<T, anyhow::Error>`) for functions where the specific error type isn't important. NGI uses it for operations where errors are operational/contextual rather than domain-specific.

## Quick Start

```rust
use anyhow::{Result, Context, anyhow};

fn main() -> Result<()> {
    let ticket = fetch_ticket(42)
        .context("failed to fetch ticket")?;
    
    process_ticket(&ticket)?;
    
    Ok(())
}

fn fetch_ticket(id: u64) -> Result<Ticket> {
    let response = std::fs::read_to_string("tickets.json")
        .context("failed to read tickets file")?;
    
    let tickets: Vec<Ticket> = serde_json::from_str(&response)
        .context("failed to parse JSON")?;
    
    tickets.into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| anyhow!("ticket {id} not found"))
}
```

## NGI Usage Patterns

### Adding Context to Errors
```rust
use anyhow::{Context, Result};

// Without context: Error message alone
async fn init_database() -> Result<sled::Db> {
    sled::open("./data")?  // Generic "IO error"
}

// With context: Specific operation context
async fn init_database() -> Result<sled::Db> {
    sled::open("./data")
        .context("failed to open ticket database at ./data")?
}

// Usage:
// Error message becomes: "failed to open ticket database at ./data: ..."
```

### Chaining Error Context
```rust
fn process_workflow() -> Result<()> {
    acquire_lock()
        .context("failed to acquire ticket lock")?;
    
    update_state()
        .context("failed to update ticket state")?;
    
    release_lock()
        .context("failed to release lock")?;
    
    Ok(())
}

// If lock acquisition fails:
// Error chain: 
// - failed to acquire ticket lock
//   - database connection timeout
//     - connection refused
```

### Creating Errors
```rust
use anyhow::anyhow;

fn validate_ticket(ticket: &Ticket) -> Result<()> {
    if ticket.id == 0 {
        return Err(anyhow!("ticket id cannot be zero"));
    }
    
    if ticket.title.is_empty() {
        return Err(anyhow!("ticket title required"));
    }
    
    Ok(())
}

// With interpolation:
fn validate_id(id: u64, max: u64) -> Result<()> {
    if id > max {
        return Err(anyhow!("id {id} exceeds maximum {max}"));
    }
    Ok(())
}
```

### Converting Domain Errors to anyhow
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket not found: {0}")]
    NotFound(u64),
}

fn get_ticket(id: u64) -> anyhow::Result<Ticket> {
    let ticket = find_ticket(id)
        .map_err(|e| anyhow::anyhow!(e))
        .context("failed to fetch ticket")?;
    Ok(ticket)
}

// Or more idiomatically:
fn get_ticket(id: u64) -> anyhow::Result<Ticket> {
    find_ticket(id)
        .map_err(|e| anyhow::anyhow!(e).context("failed to fetch ticket"))
}
```

## Error Downcasting

```rust
use anyhow::anyhow;

fn handle_error(err: anyhow::Error) {
    // Check source of error
    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        eprintln!("IO error: {io_err}");
    } else if let Some(serde_err) = err.downcast_ref::<serde_json::Error>() {
        eprintln!("JSON error: {serde_err}");
    } else {
        eprintln!("Other error: {err}");
    }
}
```

## Result Operators

### Combining Multiple Operations
```rust
fn multi_step_operation() -> anyhow::Result<Output> {
    let a = step_a().context("step A failed")?;
    let b = step_b(&a).context("step B failed")?;
    let c = step_c(&b).context("step C failed")?;
    Ok(c)
}
```

### Collecting Errors
```rust
fn process_multiple() -> anyhow::Result<Vec<Result>> {
    let items = vec![1, 2, 3, 4, 5];
    
    items.into_iter()
        .map(|id| {
            process_item(id)
                .context(format!("failed to process item {}", id))
        })
        .collect::<anyhow::Result<Vec<_>>>()
}
```

## Bail Macro

```rust
use anyhow::bail;

fn check_preconditions(input: &str) -> anyhow::Result<()> {
    if input.is_empty() {
        bail!("input cannot be empty");
    }
    
    if input.len() > 1000 {
        bail!("input exceeds 1000 characters");
    }
    
    Ok(())
}
```

## Comparison: anyhow vs thiserror

| Use Case | Choose |
|----------|--------|
| Library with custom error types | thiserror |
| Application/bin where errors are contextual | anyhow |
| API boundary (HTTP errors) | thiserror (for status codes) |
| Internal functions | anyhow (simpler) |
| Parse/decode with context needed | anyhow |
| Business logic with specific failures | thiserror |

## NGI Best Practices

### Library Crates (shared, db, custodian)
```rust
// Use thiserror for domain-specific errors
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, TicketError>;
```

### Application Crates (main.rs)
```rust
// Use anyhow for operational errors
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    initialize_services()
        .context("failed to initialize services")?;
    
    run_server().await?;
}
```

### Mixed Strategy
```rust
// Boundary: Convert domain errors to anyhow
use anyhow::Result;

async fn get_ticket_api(id: u64) -> Result<Ticket> {
    let ticket = ticket_service.get(id)
        .map_err(|e| anyhow::anyhow!(e))
        .context("failed to fetch ticket")?;
    Ok(ticket)
}
```

## Testing Error Cases

```rust
#[test]
fn test_error_context() {
    let result: anyhow::Result<i32> = Err(anyhow::anyhow!("base error")
        .context("added context"));
    
    let err_str = format!("{:?}", result);
    assert!(err_str.contains("base error"));
    assert!(err_str.contains("added context"));
}

#[tokio::test]
async fn test_operation_fails() {
    let result = failing_operation()
        .context("operation context")
        .await;
    
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("operation context"));
}
```

## NGI Service Integration Example

```rust
// auth/src/main.rs - Application code
use anyhow::{Result, Context};

#[tokio::main]
async fn main() -> Result<()> {
    let db_client = create_db_client()
        .await
        .context("failed to connect to db service")?;
    
    let auth_service = AuthService::new(db_client);
    
    run_server(auth_service)
        .await
        .context("server failed")?;
    
    Ok(())
}

// shared/src/ticket.rs - Library code
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TicketError {
    #[error("ticket {id} not found")]
    NotFound { id: u64 },
}

pub async fn validate_ticket(id: u64) -> std::result::Result<Ticket, TicketError> {
    // ...
}
```

---

**Last Updated:** December 2025  
**Documentation Version:** anyhow 1.0.0
