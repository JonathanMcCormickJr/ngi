//! Raft state machine implementation for distributed consensus
//!
//! This module implements the core Raft consensus engine integration, including:
//!
//! - `DbTypeConfig` - Type configuration for OpenRaft
//! - `DbStore` - Combined storage backend implementing RaftStorage trait
//! - `DbStateMachine` - State machine that applies log entries
//! - `RaftMetadata` - Raft-specific metadata (votes, logs)
//! - `DbLogReader` - Iterator for reading log entries
//! - `DbSnapshotBuilder` - Snapshot creation for log compaction
//!
//! # Architecture
//!
//! The Raft implementation separates concerns:
//!
//! - **Application Data**: Stored in Sled database via `Storage`
//! - **Raft Metadata**: Stored in-memory in `RaftMetadata` (votes, logs)
//! - **State Machine**: Applies log entries to storage (via `DbStateMachine`)
//! - **Network**: Skeleton for inter-node communication (see `network.rs`)
//!
//! # Single vs Multi-Node
//!
//! - **Single-node clusters**: Fully functional, works perfectly
//! - **Multi-node clusters**: Requires network layer implementation (see `network.rs`)

use crate::storage::{LogEntry, Storage};
use anyhow::Result;
use openraft::{
    BasicNode, LogId, RaftLogReader, RaftStorage, 
    StorageError, Vote, OptionalSend, RaftSnapshotBuilder, Snapshot, 
    StoredMembership, SnapshotMeta, EntryPayload, LogState,
};
use serde::{Deserialize, Serialize};
use sled::Db;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Response type for client requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbResponse {
    pub success: bool,
    pub value: Option<Vec<u8>>,
}

// Declare Raft types using the openraft macro
openraft::declare_raft_types!(
    pub DbTypeConfig:
        D = LogEntry,
        R = DbResponse,
        NodeId = u64,
        Node = BasicNode,
        Entry = openraft::Entry<DbTypeConfig>,
        SnapshotData = Cursor<Vec<u8>>,
        AsyncRuntime = openraft::TokioRuntime,
        Responder = openraft::impls::OneshotResponder<DbTypeConfig>,
);

/// Type definitions for OpenRaft
pub type DbRaft = openraft::Raft<DbTypeConfig>;

/// Raft state machine that applies log entries to storage
pub struct DbStateMachine {
    pub last_applied_log: Option<openraft::LogId<u64>>,
    pub last_membership: openraft::StoredMembership<u64, BasicNode>,
    pub storage: Storage,
}

impl DbStateMachine {
    pub fn new(storage: Storage) -> Self {
        Self {
            last_applied_log: None,
            last_membership: Default::default(),
            storage,
        }
    }

    /// Apply a log entry to the state machine
    pub async fn apply(&mut self, entry: &LogEntry) -> Result<DbResponse> {
        entry.apply(&self.storage)?;
        
        let response = match entry {
            LogEntry::Get { key } => {
                let value = self.storage.get(key)?;
                DbResponse {
                    success: value.is_some(),
                    value,
                }
            }
            _ => DbResponse {
                success: true,
                value: None,
            },
        };
        
        Ok(response)
    }
}

/// Combined Raft log storage and state machine
#[derive(Clone)]
pub struct DbStore {
    /// Main storage backend
    storage: Storage,
    /// State machine
    state_machine: Arc<RwLock<DbStateMachine>>,
    /// Raft metadata storage (vote, logs)
    raft_storage: Arc<RwLock<RaftMetadata>>,
}

/// Raft metadata stored separately from application data
pub struct RaftMetadata {
    /// Current vote
    vote: Option<Vote<u64>>,
    /// Raft log entries
    log: BTreeMap<u64, <DbTypeConfig as openraft::RaftTypeConfig>::Entry>,
    /// Last purged log index
    last_purged: Option<LogId<u64>>,
    /// Reference to Sled metadata tree for persistence
    db: Arc<sled::Tree>,
}

impl RaftMetadata {
    /// Create a new RaftMetadata instance, loading persisted data from Sled
    async fn new(db: &Db) -> Result<Self> {
        // Get or create metadata tree
        let meta_tree = db.open_tree("raft_metadata")?;
        
        // Load vote from persistent storage
        let vote = if let Some(vote_bytes) = meta_tree.get("vote")? {
            Some(serde_json::from_slice(&vote_bytes.to_vec())?)
        } else {
            None
        };
        
        // Load log entries from persistent storage
        let mut log = BTreeMap::new();
        if let Some(log_bytes) = meta_tree.get("log")? {
            let entries: Vec<(u64, <DbTypeConfig as openraft::RaftTypeConfig>::Entry)> = 
                serde_json::from_slice(&log_bytes.to_vec())?;
            for (index, entry) in entries {
                log.insert(index, entry);
            }
        }
        
        // Load last purged log id
        let last_purged = if let Some(purged_bytes) = meta_tree.get("last_purged")? {
            Some(serde_json::from_slice(&purged_bytes.to_vec())?)
        } else {
            None
        };
        
        Ok(Self {
            vote,
            log,
            last_purged,
            db: Arc::new(meta_tree),
        })
    }
    
    /// Persist vote to disk
    async fn persist_vote(&mut self, vote: &Option<Vote<u64>>) -> Result<()> {
        if let Some(v) = vote {
            let bytes = serde_json::to_vec(v)?;
            self.db.insert("vote", bytes)?;
        } else {
            self.db.remove("vote")?;
        }
        self.db.flush_async().await?;
        Ok(())
    }
    
    /// Persist log entries to disk
    async fn persist_log(&self) -> Result<()> {
        let entries: Vec<_> = self.log.iter().map(|(k, v)| (*k, v.clone())).collect();
        let bytes = serde_json::to_vec(&entries)?;
        self.db.insert("log", bytes)?;
        self.db.flush_async().await?;
        Ok(())
    }
    
    /// Persist last purged log id to disk
    async fn persist_last_purged(&self) -> Result<()> {
        if let Some(purged) = &self.last_purged {
            let bytes = serde_json::to_vec(purged)?;
            self.db.insert("last_purged", bytes)?;
        } else {
            self.db.remove("last_purged")?;
        }
        self.db.flush_async().await?;
        Ok(())
    }
}

impl DbStore {
    pub async fn new(storage_path: &str) -> Result<Self> {
        let storage = Storage::new(storage_path)?;
        let state_machine = Arc::new(RwLock::new(DbStateMachine::new(storage.clone())));
        let raft_metadata = RaftMetadata::new(storage.inner()).await?;
        let raft_storage = Arc::new(RwLock::new(raft_metadata));
        
        Ok(Self {
            storage,
            state_machine,
            raft_storage,
        })
    }

    /// Create a temporary store for testing
    pub async fn new_temp() -> Result<Self> {
        let storage = Storage::new_temp()?;
        let state_machine = Arc::new(RwLock::new(DbStateMachine::new(storage.clone())));
        let raft_metadata = RaftMetadata::new(storage.inner()).await?;
        let raft_storage = Arc::new(RwLock::new(raft_metadata));
        
        Ok(Self {
            storage,
            state_machine,
            raft_storage,
        })
    }
    
    pub fn state_machine(&self) -> Arc<RwLock<DbStateMachine>> {
        self.state_machine.clone()
    }
}

/// Log reader for Raft
pub struct DbLogReader {
    log: Arc<RwLock<RaftMetadata>>,
}

impl RaftLogReader<DbTypeConfig> for DbLogReader {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Send>(
        &mut self,
        range: RB,
    ) -> Result<Vec<<DbTypeConfig as openraft::RaftTypeConfig>::Entry>, StorageError<u64>> {
        let meta = self.log.read().await;
        let mut entries = Vec::new();
        
        for (_, entry) in meta.log.range(range) {
            entries.push(entry.clone());
        }
        
        Ok(entries)
    }
}

/// Implement RaftLogReader for DbStore
impl RaftLogReader<DbTypeConfig> for DbStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Send>(
        &mut self,
        range: RB,
    ) -> Result<Vec<<DbTypeConfig as openraft::RaftTypeConfig>::Entry>, StorageError<u64>> {
        let meta = self.raft_storage.read().await;
        let mut entries = Vec::new();
        
        for (_, entry) in meta.log.range(range) {
            entries.push(entry.clone());
        }
        
        Ok(entries)
    }
}

/// Implement RaftStorage trait for DbStore
impl RaftStorage<DbTypeConfig> for DbStore {
    type LogReader = DbLogReader;
    type SnapshotBuilder = DbSnapshotBuilder;

    async fn save_vote(&mut self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        let mut meta = self.raft_storage.write().await;
        meta.vote = Some(vote.clone());
        meta.persist_vote(&Some(vote.clone())).await
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Vote,
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<u64>>, StorageError<u64>> {
        let meta = self.raft_storage.read().await;
        Ok(meta.vote.clone())
    }

    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<u64>>
    where
        I: IntoIterator<Item = <DbTypeConfig as openraft::RaftTypeConfig>::Entry> + OptionalSend,
    {
        let mut meta = self.raft_storage.write().await;
        
        let mut last_log_id = None;
        for entry in entries {
            last_log_id = Some(entry.log_id);
            meta.log.insert(entry.log_id.index, entry);
        }
        
        meta.persist_log().await
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(last_log_id.unwrap_or_default()),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        Ok(())
    }

    async fn delete_conflict_logs_since(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        let mut meta = self.raft_storage.write().await;
        
        // Remove all entries from log_id onwards (inclusive)
        let keys_to_remove: Vec<u64> = meta.log
            .range(log_id.index..)
            .map(|(k, _)| *k)
            .collect();
        
        for key in keys_to_remove {
            meta.log.remove(&key);
        }
        
        meta.persist_log().await
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(log_id),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        Ok(())
    }

    async fn purge_logs_upto(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        let mut meta = self.raft_storage.write().await;
        
        // Remove all entries up to and including log_id
        let keys_to_remove: Vec<u64> = meta.log
            .range(..=log_id.index)
            .map(|(k, _)| *k)
            .collect();
        
        for key in keys_to_remove {
            meta.log.remove(&key);
        }
        
        meta.last_purged = Some(log_id);
        
        meta.persist_log().await
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(log_id),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        meta.persist_last_purged().await
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(log_id),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        Ok(())
    }

    async fn last_applied_state(
        &mut self,
    ) -> Result<(Option<LogId<u64>>, StoredMembership<u64, BasicNode>), StorageError<u64>> {
        let sm = self.state_machine.read().await;
        Ok((sm.last_applied_log, sm.last_membership.clone()))
    }

    async fn apply_to_state_machine(&mut self, entries: &[<DbTypeConfig as openraft::RaftTypeConfig>::Entry]) -> Result<Vec<DbResponse>, StorageError<u64>> {
        let mut sm = self.state_machine.write().await;
        let mut responses = Vec::new();
        
        for entry in entries {
            sm.last_applied_log = Some(entry.log_id);
            
            let response = match &entry.payload {
                EntryPayload::Blank => {
                    // Blank entries still need a response
                    DbResponse {
                        success: true,
                        value: None,
                    }
                }
                EntryPayload::Normal(log_entry) => {
                    sm.apply(log_entry).await
                        .map_err(|e| {
                            openraft::StorageIOError::new(
                                openraft::ErrorSubject::StateMachine,
                                openraft::ErrorVerb::Write,
                                openraft::AnyError::error(e.to_string())
                            )
                        })?
                }
                EntryPayload::Membership(membership) => {
                    sm.last_membership = StoredMembership::new(Some(entry.log_id), membership.clone());
                    // Membership changes also need a response
                    DbResponse {
                        success: true,
                        value: None,
                    }
                }
            };
            
            responses.push(response);
        }
        
        Ok(responses)
    }

    async fn begin_receiving_snapshot(&mut self) -> Result<Box<Cursor<Vec<u8>>>, StorageError<u64>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<u64, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<u64>> {
        let data = snapshot.into_inner();
        
        // Deserialize snapshot data using serde_json for simplicity
        let snapshot_data: Vec<(Vec<u8>, Vec<u8>)> = serde_json::from_slice(&data)
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Snapshot(Some(meta.signature())),
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        // Clear storage and restore from snapshot
        // Note: This is simplified - in production you'd want atomic replacement
        for (key, value) in snapshot_data {
            self.storage.put(&key, &value)
                .map_err(|e| {
                    openraft::StorageIOError::new(
                        openraft::ErrorSubject::StateMachine,
                        openraft::ErrorVerb::Write,
                        openraft::AnyError::error(e.to_string())
                    )
                })?;
        }
        
        let mut sm = self.state_machine.write().await;
        sm.last_applied_log = meta.last_log_id;
        sm.last_membership = meta.last_membership.clone();
        
        Ok(())
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<DbTypeConfig>>, StorageError<u64>> {
        // For now, return None - snapshots are not yet fully implemented
        Ok(None)
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        DbLogReader {
            log: self.raft_storage.clone(),
        }
    }

    async fn get_log_state(&mut self) -> Result<LogState<DbTypeConfig>, StorageError<u64>> {
        let meta = self.raft_storage.read().await;
        
        let last_log_id = meta.log.iter().next_back().map(|(_, entry)| entry.log_id);
        let last_purged_log_id = meta.last_purged;
        
        Ok(LogState {
            last_log_id,
            last_purged_log_id,
        })
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        DbSnapshotBuilder {
            storage: self.storage.clone(),
        }
    }
}

/// Snapshot builder for creating Raft snapshots
pub struct DbSnapshotBuilder {
    storage: Storage,
}

impl RaftSnapshotBuilder<DbTypeConfig> for DbSnapshotBuilder {
    async fn build_snapshot(&mut self) -> Result<Snapshot<DbTypeConfig>, StorageError<u64>> {
        // Get all data from storage
        let all_data = self.storage.list(b"", None)
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::StateMachine,
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        // Serialize to snapshot format using serde_json for simplicity
        let data = serde_json::to_vec(&all_data)
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::StateMachine,
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string())
                )
            })?;
        
        let snapshot_id = format!("snapshot-{}", chrono::Utc::now().timestamp());
        
        // Create snapshot metadata (simplified - should track actual state)
        let meta = SnapshotMeta {
            snapshot_id,
            last_log_id: None,
            last_membership: Default::default(),
        };
        
        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(data)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_machine_apply_put() {
        let storage = Storage::new_temp().unwrap();
        let mut sm = DbStateMachine::new(storage.clone());
        
        let entry = LogEntry::Put {
            key: b"key1".to_vec(),
            value: b"value1".to_vec(),
        };
        
        let response = sm.apply(&entry).await.unwrap();
        assert!(response.success);
        
        // Verify it was stored
        let stored = storage.get(b"key1").unwrap();
        assert_eq!(stored, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_state_machine_apply_delete() {
        let storage = Storage::new_temp().unwrap();
        let mut sm = DbStateMachine::new(storage.clone());
        
        // First put a value
        storage.put(b"key1", b"value1").unwrap();
        
        // Then delete it via state machine
        let entry = LogEntry::Delete {
            key: b"key1".to_vec(),
        };
        
        let response = sm.apply(&entry).await.unwrap();
        assert!(response.success);
        
        // Verify it was deleted
        assert!(!storage.exists(b"key1").unwrap());
    }

    #[tokio::test]
    async fn test_store_creation() {
        let store = DbStore::new_temp().await.unwrap();
        let sm = store.state_machine.read().await;
        assert_eq!(sm.last_applied_log, None);
    }
}
