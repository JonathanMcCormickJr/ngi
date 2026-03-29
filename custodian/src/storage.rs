//! Storage backend for custodian service
//!
//! This module provides persistent storage for ticket locks using Sled,
//! with Raft consensus for distributed coordination.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

// Tree Names
pub const TREE_LOCKS: &str = "locks";
pub const TREE_RAFT_METADATA: &str = "raft_metadata";
pub const TREE_RAFT_STATE: &str = "raft_state";
pub const TREE_RAFT_LOG: &str = "raft_log";

/// Lock information stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub ticket_id: u64,
    pub user_id: Uuid,
    pub acquired_at: chrono::DateTime<chrono::Utc>,
}

/// Storage layer wrapping Sled
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

    /// Get the underlying Sled database
    #[must_use]
    pub fn inner(&self) -> &Db {
        &self.db
    }

    /// Open or get a handle to a specific tree
    ///
    /// # Errors
    ///
    /// Returns an error if the tree cannot be opened.
    pub fn get_tree(&self, name: &str) -> Result<Tree> {
        self.db.open_tree(name).context("failed to open tree")
    }

    /// Store a key-value pair
    ///
    /// # Errors
    ///
    /// Returns an error if the key-value pair cannot be stored.
    pub fn put(&self, tree: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let tree = self.get_tree(tree)?;
        tree.insert(key, value)?;
        Ok(())
    }

    /// Retrieve a value by key
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be retrieved.
    pub fn get(&self, tree: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tree = self.get_tree(tree)?;
        Ok(tree.get(key)?.map(|v| v.to_vec()))
    }

    /// Delete a key
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be deleted.
    pub fn delete(&self, tree: &str, key: &[u8]) -> Result<()> {
        let tree = self.get_tree(tree)?;
        tree.remove(key)?;
        Ok(())
    }

    /// List all key-value pairs in a tree with optional prefix
    ///
    /// # Errors
    ///
    /// Returns an error if the tree cannot be accessed.
    pub fn list(
        &self,
        tree: &str,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let tree = self.get_tree(tree)?;
        let mut results = Vec::new();

        let iter = if prefix.is_empty() {
            tree.iter()
        } else {
            tree.scan_prefix(prefix)
        };

        for item in iter {
            let (key, value) = item?;
            results.push((key.to_vec(), value.to_vec()));

            if let Some(limit) = limit
                && results.len() >= limit
            {
                break;
            }
        }

        Ok(results)
    }

    /// Check if a key exists
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    pub fn exists(&self, tree: &str, key: &[u8]) -> Result<bool> {
        let tree = self.get_tree(tree)?;
        Ok(tree.contains_key(key)?)
    }

    /// Acquire a lock on a ticket
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    pub fn acquire_lock(&self, ticket_id: u64, user_id: Uuid) -> Result<()> {
        let key = ticket_id.to_be_bytes();
        let lock_info = LockInfo {
            ticket_id,
            user_id,
            acquired_at: chrono::Utc::now(),
        };
        let value = serde_json::to_vec(&lock_info)?;
        self.put(TREE_LOCKS, &key, &value)
    }

    /// Release a lock on a ticket
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be released.
    pub fn release_lock(&self, ticket_id: u64) -> Result<()> {
        let key = ticket_id.to_be_bytes();
        self.delete(TREE_LOCKS, &key)
    }

    /// Check if a ticket is locked
    ///
    /// # Errors
    ///
    /// Returns an error if the lock status cannot be checked.
    pub fn is_locked(&self, ticket_id: u64) -> Result<bool> {
        let key = ticket_id.to_be_bytes();
        self.exists(TREE_LOCKS, &key)
    }

    /// Get lock information for a ticket
    ///
    /// # Errors
    ///
    /// Returns an error if the lock information cannot be retrieved.
    pub fn get_lock_info(&self, ticket_id: u64) -> Result<Option<LockInfo>> {
        let key = ticket_id.to_be_bytes();
        match self.get(TREE_LOCKS, &key)? {
            Some(value) => {
                let lock_info = serde_json::from_slice(&value)?;
                Ok(Some(lock_info))
            }
            None => Ok(None),
        }
    }

    /// Get all current locks
    ///
    /// # Errors
    ///
    /// Returns an error if the locks cannot be retrieved.
    pub fn get_all_locks(&self) -> Result<HashMap<u64, LockInfo>> {
        let tree = self.get_tree(TREE_LOCKS)?;
        let mut locks = HashMap::new();

        for item in &tree {
            let (key, value) = item?;
            let ticket_id = u64::from_be_bytes(
                key.as_ref()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid key length"))?,
            );
            let lock_info: LockInfo = serde_json::from_slice(&value)?;
            locks.insert(ticket_id, lock_info);
        }

        Ok(locks)
    }
}

/// Raft log entry representing a lock operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LockCommand {
    AcquireLock { ticket_id: u64, user_id: Uuid },
    ReleaseLock { ticket_id: u64, user_id: Uuid },
}

impl LockCommand {
    /// Apply this command to storage
    ///
    /// # Errors
    ///
    /// Returns an error if the command cannot be applied.
    pub fn apply(&self, storage: &Storage) -> Result<()> {
        match self {
            Self::AcquireLock { ticket_id, user_id } => storage.acquire_lock(*ticket_id, *user_id),
            Self::ReleaseLock { ticket_id, .. } => storage.release_lock(*ticket_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_operations() {
        let storage = Storage::new_temp().unwrap();
        let ticket_id = 42;
        let user_id = Uuid::new_v4();

        // Initially not locked
        assert!(!storage.is_locked(ticket_id).unwrap());

        // Acquire lock
        storage.acquire_lock(ticket_id, user_id).unwrap();
        assert!(storage.is_locked(ticket_id).unwrap());

        // Get lock info
        let lock_info = storage.get_lock_info(ticket_id).unwrap().unwrap();
        assert_eq!(lock_info.ticket_id, ticket_id);
        assert_eq!(lock_info.user_id, user_id);

        // Release lock
        storage.release_lock(ticket_id).unwrap();
        assert!(!storage.is_locked(ticket_id).unwrap());
    }

    #[test]
    fn test_get_all_locks() {
        let storage = Storage::new_temp().unwrap();
        let user_id = Uuid::new_v4();

        // Add multiple locks
        storage.acquire_lock(1, user_id).unwrap();
        storage.acquire_lock(2, user_id).unwrap();

        let locks = storage.get_all_locks().unwrap();
        assert_eq!(locks.len(), 2);
        assert!(locks.contains_key(&1));
        assert!(locks.contains_key(&2));
    }
}
