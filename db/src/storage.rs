//! Storage backend using Sled embedded database

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::Path;

/// Storage layer wrapping Sled
#[derive(Clone)]
pub struct Storage {
    db: Db,
}

impl Storage {
    /// Create a new storage instance
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    /// Create an in-memory storage for testing
    pub fn new_temp() -> Result<Self> {
        let db = sled::Config::new().temporary(true).open()?;
        Ok(Self { db })
    }

    /// Store a key-value pair
    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.db.insert(key, value)?;
        self.db.flush()?;
        Ok(())
    }

    /// Retrieve a value by key
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.db.get(key)?.map(|v| v.to_vec()))
    }

    /// Delete a key
    pub fn delete(&self, key: &[u8]) -> Result<()> {
        self.db.remove(key)?;
        self.db.flush()?;
        Ok(())
    }

    /// List key-value pairs with optional prefix
    pub fn list(&self, prefix: &[u8], limit: Option<usize>) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let iter = if prefix.is_empty() {
            self.db.iter()
        } else {
            self.db.scan_prefix(prefix)
        };

        let pairs: Result<Vec<_>, _> = iter
            .take(limit.unwrap_or(usize::MAX))
            .map(|r| r.map(|(k, v)| (k.to_vec(), v.to_vec())))
            .collect();

        Ok(pairs?)
    }

    /// Check if key exists
    pub fn exists(&self, key: &[u8]) -> Result<bool> {
        Ok(self.db.contains_key(key)?)
    }

    /// Batch put operation
    pub fn batch_put(&self, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<usize> {
        let mut batch = sled::Batch::default();
        for (key, value) in &pairs {
            batch.insert(key.as_slice(), value.as_slice());
        }
        self.db.apply_batch(batch)?;
        self.db.flush()?;
        Ok(pairs.len())
    }
}

/// Raft log entry representing a database operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntry {
    Put { key: Vec<u8>, value: Vec<u8> },
    Get { key: Vec<u8> },
    Delete { key: Vec<u8> },
    BatchPut { pairs: Vec<(Vec<u8>, Vec<u8>)> },
}

impl LogEntry {
    /// Apply this log entry to storage
    pub fn apply(&self, storage: &Storage) -> Result<()> {
        match self {
            Self::Put { key, value } => storage.put(key, value),
            Self::Get { .. } => Ok(()), // Reads don't modify state
            Self::Delete { key } => storage.delete(key),
            Self::BatchPut { pairs } => {
                storage.batch_put(pairs.clone())?;
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
        
        storage.put(b"test_key", b"test_value").unwrap();
        let value = storage.get(b"test_key").unwrap();
        
        assert_eq!(value, Some(b"test_value".to_vec()));
    }

    #[test]
    fn test_storage_delete() {
        let storage = Storage::new_temp().unwrap();
        
        storage.put(b"key1", b"value1").unwrap();
        assert!(storage.exists(b"key1").unwrap());
        
        storage.delete(b"key1").unwrap();
        assert!(!storage.exists(b"key1").unwrap());
    }

    #[test]
    fn test_storage_list() {
        let storage = Storage::new_temp().unwrap();
        
        storage.put(b"user:1", b"alice").unwrap();
        storage.put(b"user:2", b"bob").unwrap();
        storage.put(b"post:1", b"hello").unwrap();
        
        let pairs = storage.list(b"user:", None).unwrap();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.iter().any(|(k, _)| k == b"user:1"));
        assert!(pairs.iter().any(|(k, _)| k == b"user:2"));
    }

    #[test]
    fn test_storage_batch_put() {
        let storage = Storage::new_temp().unwrap();
        
        let pairs = vec![
            (b"a".to_vec(), b"1".to_vec()),
            (b"b".to_vec(), b"2".to_vec()),
            (b"c".to_vec(), b"3".to_vec()),
        ];
        
        let count = storage.batch_put(pairs).unwrap();
        assert_eq!(count, 3);
        
        assert_eq!(storage.get(b"a").unwrap(), Some(b"1".to_vec()));
        assert_eq!(storage.get(b"b").unwrap(), Some(b"2".to_vec()));
        assert_eq!(storage.get(b"c").unwrap(), Some(b"3".to_vec()));
    }

    #[test]
    fn test_log_entry_apply() {
        let storage = Storage::new_temp().unwrap();
        
        let entry = LogEntry::Put {
            key: b"test".to_vec(),
            value: b"data".to_vec(),
        };
        
        entry.apply(&storage).unwrap();
        assert_eq!(storage.get(b"test").unwrap(), Some(b"data".to_vec()));
    }
}
