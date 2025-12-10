//! Error types for NGI system operations.

use thiserror::Error;

/// NGI system errors
#[derive(Error, Debug)]
pub enum NgiError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Authorization failed: insufficient permissions")]
    AuthorizationError,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Resource conflict: {0}")]
    Conflict(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

/// Result type alias for NGI operations
pub type NgiResult<T> = Result<T, NgiError>;
