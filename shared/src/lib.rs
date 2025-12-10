#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

//! Shared types and utilities for the NGI ticketing system.
//!
//! This crate provides common data structures, enums, and traits used across
//! all NGI microservices. It ensures type consistency and enables seamless
//! serialization/deserialization for both gRPC and REST APIs.

pub mod ticket;
pub mod user;
pub mod error;

// Re-export commonly used types
pub use ticket::{Ticket, TicketStatus, Symptom, Resolution, NextAction};
pub use user::{User, Role, AuthMethod};
pub use error::NgiError;
