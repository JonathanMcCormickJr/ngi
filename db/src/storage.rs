//! Storage backend using Sled embedded database
//!
//! This module provides a persistent key-value store wrapper around Sled,
//! a high-performance embedded database written in Rust.
//!
//! # Operations
//!
//! - Single-key: `put`, `get`, `delete`, `exists`
//! - Multi-key: `list` (with prefix filtering), `batch_put`
//! - Testing: `new_temp` (creates temporary in-memory store)
//!
//! # Persistence
//!
//! All write operations are automatically flushed to disk, ensuring durability.
//! Data persists across process restarts.
//!
//! # Raft Integration
//!
//! The `LogEntry` enum represents all database operations that can be replicated
//! through Raft consensus. Each entry type can be applied to storage independently.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::path::Path;

// Tree Names
pub const TREE_RAFT_METADATA: &str = "raft_metadata";
pub const TREE_RAFT_STATE: &str = "raft_state";
pub const TREE_RAFT_LOG: &str = "raft_log";
pub const TREE_TICKETS: &str = "tickets";
pub const TREE_USERS: &str = "users";
pub const TREE_SESSIONS: &str = "sessions";
pub const TREE_AUDIT: &str = "audit";

// Index Trees
pub const IDX_TICKET_STATUS: &str = "idx_ticket_status";
pub const IDX_TICKET_ASSIGNEE: &str = "idx_ticket_assignee";
pub const IDX_TICKET_PROJECT: &str = "idx_ticket_project";
pub const IDX_TICKET_ACCOUNT: &str = "idx_ticket_account";
pub const IDX_TICKET_CREATED: &str = "idx_ticket_created";
pub const IDX_TICKET_UPDATED: &str = "idx_ticket_updated";
pub const IDX_TICKET_TRACKING: &str = "idx_ticket_tracking";
pub const IDX_USER_NAME: &str = "idx_user_name";
pub const IDX_USER_EMAIL: &str = "idx_user_email";
pub const IDX_USER_ROLE: &str = "idx_user_role";

/// Storage layer wrapping Sled with Namespaced Trees
#[derive(Clone)]
pub struct Storage {
    db: Db,
}

impl Storage {
    /// Create a new storage instance
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened at the specified path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    /// Create an in-memory storage for testing
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary database cannot be created.
    pub fn new_temp() -> Result<Self> {
        let db = sled::Config::new().temporary(true).open()?;
        Ok(Self { db })
    }

    /// Get the underlying Sled database for metadata access
    #[must_use]
    pub fn inner(&self) -> &Db {
        &self.db
    }

    /// Open or get a handle to a specific tree (namespace)
    ///
    /// # Errors
    ///
    /// Returns an error if the tree cannot be opened.
    pub fn get_tree(&self, name: &str) -> Result<Tree> {
        self.db.open_tree(name).context("failed to open tree")
    }

    /// Store a key-value pair in a specific collection (tree)
    ///
    /// # Errors
    ///
    /// Returns an error if the key-value pair cannot be stored.
    pub fn put(&self, collection: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let tree = self.get_tree(collection)?;
        tree.insert(key, value)?;
        // We might want to flush periodically rather than every put for performance,
        // but for safety in this critical DB, explicit flush is safer.
        // However, Sled flushes asynchronously by default.
        // For Raft, we usually rely on the Raft log flush.
        // Let's keep it simple for now.
        tree.flush()?;
        Ok(())
    }

    /// Retrieve a value by key from a specific collection
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be retrieved.
    pub fn get(&self, collection: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tree = self.get_tree(collection)?;
        Ok(tree.get(key)?.map(|v| v.to_vec()))
    }

    /// Delete a key from a specific collection
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be deleted.
    pub fn delete(&self, collection: &str, key: &[u8]) -> Result<()> {
        let tree = self.get_tree(collection)?;
        tree.remove(key)?;
        tree.flush()?;
        Ok(())
    }

    /// List key-value pairs with optional prefix from a specific collection
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be listed.
    pub fn list(
        &self,
        collection: &str,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let tree = self.get_tree(collection)?;
        let iter = if prefix.is_empty() {
            tree.iter()
        } else {
            tree.scan_prefix(prefix)
        };

        let pairs: Result<Vec<_>, _> = iter
            .take(limit.unwrap_or(usize::MAX))
            .map(|r| r.map(|(k, v)| (k.to_vec(), v.to_vec())))
            .collect();

        Ok(pairs?)
    }

    /// Check if key exists in a collection
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    pub fn exists(&self, collection: &str, key: &[u8]) -> Result<bool> {
        let tree = self.get_tree(collection)?;
        Ok(tree.contains_key(key)?)
    }

    /// Batch put operation into a specific collection
    ///
    /// # Errors
    ///
    /// Returns an error if the batch operation fails.
    pub fn batch_put(&self, collection: &str, pairs: &Vec<(Vec<u8>, Vec<u8>)>) -> Result<usize> {
        let tree = self.get_tree(collection)?;
        let mut batch = sled::Batch::default();
        for (key, value) in pairs {
            batch.insert(key.as_slice(), value.as_slice());
        }
        tree.apply_batch(batch)?;
        tree.flush()?;
        Ok(pairs.len())
    }
}

/// Raft log entry representing a database operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntry {
    Put {
        collection: String,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Get {
        collection: String,
        key: Vec<u8>,
    }, // Reads usually don't go through Raft log, but kept for consistency if needed
    Delete {
        collection: String,
        key: Vec<u8>,
    },
    BatchPut {
        collection: String,
        pairs: Vec<(Vec<u8>, Vec<u8>)>,
    },
}

impl LogEntry {
    /// Apply this log entry to storage
    ///
    /// # Errors
    ///
    /// Returns an error if the log entry cannot be applied.
    pub fn apply(&self, storage: &Storage) -> Result<()> {
        match self {
            Self::Put {
                collection,
                key,
                value,
            } => storage.put(collection, key, value),
            Self::Get { .. } => Ok(()), // Reads don't modify state
            Self::Delete { collection, key } => storage.delete(collection, key),
            Self::BatchPut { collection, pairs } => {
                storage.batch_put(collection, pairs)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_put_get() {
        let storage = Storage::new_temp().unwrap();
        let collection = "test_coll";

        storage.put(collection, b"test_key", b"test_value").unwrap();
        let value = storage.get(collection, b"test_key").unwrap();

        assert_eq!(value, Some(b"test_value".to_vec()));
    }

    #[test]
    fn test_storage_delete() {
        let storage = Storage::new_temp().unwrap();
        let collection = "test_coll";

        storage.put(collection, b"key1", b"value1").unwrap();
        assert!(storage.exists(collection, b"key1").unwrap());

        storage.delete(collection, b"key1").unwrap();
        assert!(!storage.exists(collection, b"key1").unwrap());
    }

    #[test]
    fn test_storage_list() {
        let storage = Storage::new_temp().unwrap();
        let collection = "test_coll";

        storage.put(collection, b"user:1", b"alice").unwrap();
        storage.put(collection, b"user:2", b"bob").unwrap();
        storage.put(collection, b"post:1", b"hello").unwrap();

        let pairs = storage.list(collection, b"user:", None).unwrap();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.iter().any(|(k, _)| k == b"user:1"));
        assert!(pairs.iter().any(|(k, _)| k == b"user:2"));
    }

    #[test]
    fn test_storage_batch_put() {
        let storage = Storage::new_temp().unwrap();
        let collection = "test_coll";

        let pairs = vec![
            (b"a".to_vec(), b"1".to_vec()),
            (b"b".to_vec(), b"2".to_vec()),
            (b"c".to_vec(), b"3".to_vec()),
        ];

        let count = storage.batch_put(collection, &pairs).unwrap();
        assert_eq!(count, 3);

        assert_eq!(storage.get(collection, b"a").unwrap(), Some(b"1".to_vec()));
        assert_eq!(storage.get(collection, b"b").unwrap(), Some(b"2".to_vec()));
        assert_eq!(storage.get(collection, b"c").unwrap(), Some(b"3".to_vec()));
    }

    #[test]
    fn test_log_entry_apply() {
        let storage = Storage::new_temp().unwrap();
        let collection = "test_coll";

        let entry = LogEntry::Put {
            collection: collection.to_string(),
            key: b"test".to_vec(),
            value: b"data".to_vec(),
        };

        entry.apply(&storage).unwrap();
        assert_eq!(
            storage.get(collection, b"test").unwrap(),
            Some(b"data".to_vec())
        );
    }
}
