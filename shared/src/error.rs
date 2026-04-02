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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ngi_error_display_all_variants() {
        let errors: Vec<NgiError> = vec![
            NgiError::ValidationError("bad input".to_string()),
            NgiError::AuthenticationError("wrong password".to_string()),
            NgiError::AuthorizationError,
            NgiError::NotFound("ticket 42".to_string()),
            NgiError::Conflict("duplicate".to_string()),
            NgiError::DatabaseError("connection lost".to_string()),
            NgiError::NetworkError("timeout".to_string()),
            NgiError::InternalError("unexpected".to_string()),
            NgiError::RateLimitExceeded,
            NgiError::ServiceUnavailable("maintenance".to_string()),
        ];

        let expected_substrings = [
            "bad input",
            "wrong password",
            "insufficient permissions",
            "ticket 42",
            "duplicate",
            "connection lost",
            "timeout",
            "unexpected",
            "Rate limit",
            "maintenance",
        ];

        for (error, expected) in errors.iter().zip(expected_substrings.iter()) {
            let msg = error.to_string();
            assert!(
                msg.contains(expected),
                "Error message '{msg}' should contain '{expected}'"
            );
        }
    }

    #[test]
    fn test_ngi_result_type_alias() {
        let ok: NgiResult<u32> = Ok(42);
        assert!(ok.is_ok());

        let err: NgiResult<u32> = Err(NgiError::NotFound("x".to_string()));
        assert!(err.is_err());
    }
}
