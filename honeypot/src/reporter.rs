//! Alert reporting module for sending intrusion events to admin service.

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use std::time::SystemTime;

/// Intrusion event captured by honeypot
#[derive(Debug, Clone)]
pub struct IntrusionEvent {
    pub timestamp: SystemTime,
    pub source_ip: String,
    pub user_agent: Option<String>,
    pub endpoint_accessed: String,
    pub request_method: String,
    pub request_body: Option<String>,
    pub tls_fingerprint: Option<String>,
}

impl IntrusionEvent {
    /// Create a new intrusion event
    #[must_use]
    pub fn new(source_ip: String, endpoint: String, method: String) -> Self {
        Self {
            timestamp: SystemTime::now(),
            source_ip,
            user_agent: None,
            endpoint_accessed: endpoint,
            request_method: method,
            request_body: None,
            tls_fingerprint: None,
        }
    }

    /// Report this event to the admin service via gRPC
    /// TODO: Implement actual gRPC client call to admin service
    pub fn report(&self) {
        eprintln!("🚨 INTRUSION DETECTED: {:?}", self);
    }
}
