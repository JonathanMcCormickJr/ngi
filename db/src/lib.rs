//! DB Service Library
//!
//! This library provides the core components of the distributed key-value database service:
//! - Raft consensus implementation
//! - Storage backend (Sled)
//! - gRPC server implementation
//! - Network layer for inter-node communication

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

pub mod network;
pub mod raft;
pub mod raft_service;
pub mod server;
pub mod storage;
