# Error Handling: thiserror & anyhow

**thiserror:** https://docs.rs/thiserror/  
**anyhow:** https://docs.rs/anyhow/

## Versions in NGI
```toml
thiserror = "2.0"
anyhow = "1.0"
```

## Architecture

NGI uses a two-layer error handling strategy:
1. **thiserror** for typed, domain-specific errors (production code)
2. **anyhow** for contextual debugging and error chains (error handling)

## Layer 1: Custom Error Types (thiserror)

### Overview
Each service defines its own error types using `thiserror`. This provides:
- Type safety (compiler catches error variants)
- Documentation (error variants are self-documenting)
- Conversions (automatic `From` implementations)

### Pattern

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TicketError {
    // Simple error with message
    #[error("ticket not found")]
    NotFound,

    // Error with context
    #[error("ticket not found: {0}")]
    NotFoundWithId(u64),

    // Error with structured data
    #[error("ticket already locked by {user}")]
    AlreadyLocked { user: String },

    // Error wrapping another error (automatic From impl)
    #[error("database error")]
    Database(#[from] sled::Error),

    #[error("serialization error")]
    Serialization(#[from] bincode::Error),

    // Delegating to anyhow for complex errors
    #[error("service error: {0}")]
    Service(#[from] anyhow::Error),
}

// Define Result type alias
pub type Result<T> = std::result::Result<T, TicketError>;
```

### Shared Error Type (shared/src/error.rs)

All services use a common error type defined in `shared`:

```rust
// shared/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SharedError {
    #[error("ticket not found")]
    TicketNotFound,

    #[error("user not found")]
    UserNotFound,

    #[error("permission denied")]
    PermissionDenied,

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("internal error")]
    Internal,
}

pub type Result<T> = std::result::Result<T, SharedError>;
```

## Layer 2: Error Context (anyhow)

### When to Use anyhow

Use `anyhow` for:
- Operational errors where you don't control the error type
- Building error context through the stack
- Internal error handling that won't be exposed to clients

### Pattern

```rust
use anyhow::Context;

// Adding context
pub async fn get_ticket(&self, id: u64) -> Result<Ticket> {
    let ticket = self.db
        .get_ticket(id)
        .await
        .context("failed to fetch ticket from database")?;

    self.validate_ticket(&ticket)
        .context("ticket validation failed")?;

    Ok(ticket)
}

// Building error chains
pub async fn update_with_retry(&self, ticket: Ticket) -> Result<Ticket> {
    for attempt in 0..3 {
        match self.update(&ticket).await {
            Ok(updated) => return Ok(updated),
            Err(e) if attempt < 2 => {
                tracing::warn!("update failed, retrying: {}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                return Err(e).context(format!("update failed after {} attempts", attempt + 1))?;
            }
        }
    }
    unreachable!()
}
```

## API Boundary Conversion

Convert domain errors to gRPC Status codes:

```rust
use tonic::Status;

impl From<TicketError> for Status {
    fn from(err: TicketError) -> Self {
        match err {
            TicketError::NotFound => Status::not_found("ticket not found"),
            TicketError::AlreadyLocked { user } => {
                Status::already_exists(format!("locked by {}", user))
            }
            TicketError::Database(e) => {
                tracing::error!("database error: {}", e);
                Status::internal("database error")
            }
            TicketError::Service(e) => {
                tracing::error!("service error: {}", e);
                Status::internal("service error")
            }
        }
    }
}

// Usage in handler
#[tonic::async_trait]
impl custodian_server::Custodian for CustodianServiceImpl {
    async fn get_ticket(
        &self,
        request: Request<GetTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let ticket = self.service
            .get_ticket(request.into_inner().id)
            .await
            .map_err(Status::from)?;  // Automatic conversion
        Ok(Response::new(ticket))
    }
}
```

## Logging Errors

Always log errors with full context:

```rust
use tracing::{error, warn};

// Critical errors
if let Err(e) = critical_operation().await {
    error!(error = ?e, "critical operation failed");
    return Err(TicketError::Internal);
}

// Non-fatal errors
if let Err(e) = non_critical_operation().await {
    warn!(error = %e, "operation failed, continuing");
    // Continue or handle gracefully
}
```

## Best Practices

### ✓ Good Patterns

**Define clear error variants:**
```rust
#[derive(Error)]
pub enum TicketError {
    #[error("ticket not found")]
    NotFound,
    
    #[error("invalid ticket status: {0}")]
    InvalidStatus(String),
}
```

**Use error context:**
```rust
let ticket = db.get(id)
    .context("failed to fetch ticket")?;
```

**Convert at boundaries:**
```rust
impl From<TicketError> for Status {
    fn from(err: TicketError) -> Self { /* ... */ }
}
```

**Log and convert:**
```rust
match operation().await {
    Ok(result) => Ok(result),
    Err(e) => {
        error!("operation failed: {:#}", e);  // {:#} = pretty print
        Err(TicketError::Internal)
    }
}
```

### ✗ Anti-Patterns

**Never silently ignore errors:**
```rust
// BAD
let _ = operation().await;

// GOOD
if let Err(e) = operation().await {
    warn!("operation failed: {}", e);
}
```

**Don't lose error context:**
```rust
// BAD
result.map_err(|_| MyError::Generic)?

// GOOD
result.context("specific operation context")?
```

**Don't use `.unwrap()` in production:**
```rust
// BAD
let ticket = db.get(id).await.unwrap();

// GOOD
let ticket = db.get(id).await?;
```

**Don't expose internal errors to clients:**
```rust
// BAD - leaks database details
Status::internal(format!("sled error: {}", e))

// GOOD - generic message
Status::internal("database error")
```

## Error Chain Example

```rust
// Layer 1: Database error
sled::Error::Io("disk full")

// Layer 2: Add context
.context("failed to insert ticket")?
// -> Error: failed to insert ticket
//    Caused by: disk full

// Layer 3: Add more context
.context("ticket creation failed")?
// -> Error: ticket creation failed
//    Caused by: failed to insert ticket
//    Caused by: disk full

// Layer 4: Convert to Status
.map_err(Status::from)?
// -> Status::Internal("service error")
```

Print full error chain with:
```rust
error!("error chain: {:#}", err);
```

---

## Official API Documentation

### thiserror

- **[Error](https://docs.rs/thiserror/latest/thiserror/derive.Error.html)** - Derive macro for error types
  - Automatically implements `std::error::Error`
  - Attributes:
    - `#[error("message")]` - Display message
    - `#[from]` - Automatic From impl
    - `#[source]` - Source error field
    - `#[backtrace]` - Backtrace field
    - `#[transparent]` - Forward to inner error

### anyhow

- **[Error](https://docs.rs/anyhow/latest/anyhow/struct.Error.html)** - Flexible error wrapper
  - Trait object based: can hold any error type
  - Methods:
    - [context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html#tymethod.context) - Add context
    - [with_context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html#tymethod.with_context) - Add dynamic context
    - [downcast_ref](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#method.downcast_ref) - Downcast to inner type
    - [downcast](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#method.downcast) - Downcast by value
    - [chain](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#method.chain) - Iterate error chain

- **[Result](https://docs.rs/anyhow/latest/anyhow/type.Result.html)** - Type alias
  - `type Result<T> = std::result::Result<T, Error>`

- **[Context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html)** - Trait for adding context
  - Methods:
    - [context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html#tymethod.context) - Static context
    - [with_context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html#tymethod.with_context) - Lazy context

- **[anyhow!](https://docs.rs/anyhow/latest/anyhow/macro.anyhow.html)** - Construct ad-hoc error
  - Usage: `anyhow!("error: {}", value)`

- **[bail!](https://docs.rs/anyhow/latest/anyhow/macro.bail.html)** - Early return with error
  - Usage: `bail!("error: {}", value)`

- **[ensure!](https://docs.rs/anyhow/latest/anyhow/macro.ensure.html)** - Assert with error
  - Usage: `ensure!(condition, "error message")`

- **[Chain](https://docs.rs/anyhow/latest/anyhow/struct.Chain.html)** - Iterator over error sources
  - Get via `error.chain()`

## Error Recovery

```rust
pub async fn get_ticket_with_fallback(&self, id: u64) -> Result<Ticket> {
    match self.db.get_ticket(id).await {
        Ok(ticket) => Ok(ticket),
        Err(TicketError::Database(e)) => {
            warn!("database unavailable, trying cache: {}", e);
            self.cache.get_ticket(id)
                .context("cache lookup also failed")?
        }
        Err(e) => Err(e),
    }
}
```

---

**See Also:**
- [thiserror Crate](../thiserror/README.md)
- [anyhow Crate](../anyhow/README.md)
