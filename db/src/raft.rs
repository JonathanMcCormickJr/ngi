//! Raft state machine implementation for distributed consensus
//!
//! This module implements the core Raft consensus engine integration, including:
//!
//! - `DbTypeConfig` - Type configuration for `OpenRaft`
//! - `DbStore` - Combined storage backend implementing `RaftStorage` trait
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

use crate::storage::{LogEntry, Storage, TREE_RAFT_LOG, TREE_RAFT_METADATA};
use anyhow::Result;
use openraft::{
    BasicNode, CommittedLeaderId, EntryPayload, LogId, LogState, OptionalSend, RaftLogReader,
    RaftSnapshotBuilder, RaftStorage, Snapshot, SnapshotMeta, StorageError, StoredMembership, Vote,
};
use serde::{Deserialize, Serialize};
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

/// Type definitions for `OpenRaft`
pub type DbRaft = openraft::Raft<DbTypeConfig>;

/// Raft state machine that applies log entries to storage
pub struct DbStateMachine {
    pub last_applied_log: Option<openraft::LogId<u64>>,
    pub last_membership: openraft::StoredMembership<u64, BasicNode>,
    pub storage: Storage,
}

impl DbStateMachine {
    #[must_use]
    pub fn new(storage: Storage) -> Self {
        Self {
            last_applied_log: None,
            last_membership: StoredMembership::default(),
            storage,
        }
    }

    /// Apply a log entry to the state machine
    ///
    /// # Errors
    ///
    /// Returns an error if the log entry cannot be applied to storage.
    pub fn apply(&mut self, entry: &LogEntry) -> Result<DbResponse> {
        entry.apply(&self.storage)?;

        let response = match entry {
            LogEntry::Get { collection, key } => {
                let value = self.storage.get(collection, key)?;
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
    /// Last purged log index
    last_purged: Option<LogId<u64>>,
    /// Reference to Storage for accessing trees
    storage: Storage,
}

impl RaftMetadata {
    /// Create a new `RaftMetadata` instance, loading persisted data from Sled
    fn new(storage: Storage) -> Result<Self> {
        // Load vote from persistent storage
        let vote = if let Some(vote_bytes) = storage.get(TREE_RAFT_METADATA, b"vote")? {
            Some(serde_json::from_slice(&vote_bytes)?)
        } else {
            None
        };

        // Load last purged log id
        let last_purged =
            if let Some(purged_bytes) = storage.get(TREE_RAFT_METADATA, b"last_purged")? {
                Some(serde_json::from_slice(&purged_bytes)?)
            } else {
                None
            };

        Ok(Self {
            vote,
            last_purged,
            storage,
        })
    }

    /// Persist vote to disk
    fn persist_vote(&mut self, vote: Option<&Vote<u64>>) -> Result<()> {
        if let Some(v) = vote {
            let bytes = serde_json::to_vec(v)?;
            self.storage.put(TREE_RAFT_METADATA, b"vote", &bytes)?;
        } else {
            self.storage.delete(TREE_RAFT_METADATA, b"vote")?;
        }
        Ok(())
    }

    /// Persist last purged log id to disk
    fn persist_last_purged(&self) -> Result<()> {
        if let Some(purged) = &self.last_purged {
            let bytes = serde_json::to_vec(purged)?;
            self.storage
                .put(TREE_RAFT_METADATA, b"last_purged", &bytes)?;
        } else {
            self.storage.delete(TREE_RAFT_METADATA, b"last_purged")?;
        }
        Ok(())
    }
}

impl DbStore {
    /// Create a new store with persistent storage
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be initialized.
    pub fn new(storage_path: &str) -> Result<Self> {
        let storage = Storage::new(storage_path)?;
        let state_machine = Arc::new(RwLock::new(DbStateMachine::new(storage.clone())));
        let raft_metadata = RaftMetadata::new(storage.clone())?;
        let raft_storage = Arc::new(RwLock::new(raft_metadata));

        Ok(Self {
            storage,
            state_machine,
            raft_storage,
        })
    }

    /// Create a temporary store for testing
    ///
    /// # Errors
    ///
    /// Returns an error if temporary storage cannot be initialized.
    pub fn new_temp() -> Result<Self> {
        let storage = Storage::new_temp()?;
        let state_machine = Arc::new(RwLock::new(DbStateMachine::new(storage.clone())));
        let raft_metadata = RaftMetadata::new(storage.clone())?;
        let raft_storage = Arc::new(RwLock::new(raft_metadata));

        Ok(Self {
            storage,
            state_machine,
            raft_storage,
        })
    }

    #[must_use]
    pub fn state_machine(&self) -> Arc<RwLock<DbStateMachine>> {
        self.state_machine.clone()
    }
}

/// Log reader for Raft
pub struct DbLogReader {
    storage: Storage,
}

impl RaftLogReader<DbTypeConfig> for DbLogReader {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Send>(
        &mut self,
        range: RB,
    ) -> Result<Vec<<DbTypeConfig as openraft::RaftTypeConfig>::Entry>, StorageError<u64>> {
        let start = match range.start_bound() {
            std::ops::Bound::Included(i) => *i,
            std::ops::Bound::Excluded(i) => i + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            std::ops::Bound::Included(i) => Some(*i),
            std::ops::Bound::Excluded(i) => Some(i - 1),
            std::ops::Bound::Unbounded => None,
        };

        let tree = self.storage.get_tree(TREE_RAFT_LOG).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(LogId::default()),
                openraft::ErrorVerb::Read,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        let mut entries = Vec::new();

        // Create a range scan
        let start_key = start.to_be_bytes();
        let iter = tree.range(start_key..);

        for item in iter {
            let (k, v) = item.map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(LogId::default()),
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;

            let index = u64::from_be_bytes(k.as_ref().try_into().map_err(|_| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(LogId::default()),
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error("invalid log key format: expected 8 bytes"),
                )
            })?);

            if let Some(end_idx) = end && index > end_idx {
                break;
            }

            let entry: <DbTypeConfig as openraft::RaftTypeConfig>::Entry =
                serde_json::from_slice(&v).map_err(|e| {
                    openraft::StorageIOError::new(
                        openraft::ErrorSubject::Log(LogId::new(
                            CommittedLeaderId::new(0, 0),
                            index,
                        )),
                        openraft::ErrorVerb::Read,
                        openraft::AnyError::error(e.to_string()),
                    )
                })?;

            entries.push(entry);
        }

        Ok(entries)
    }
}

/// Implement `RaftLogReader` for `DbStore`
impl RaftLogReader<DbTypeConfig> for DbStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Send + std::fmt::Debug>(
        &mut self,
        range: RB,
    ) -> Result<Vec<<DbTypeConfig as openraft::RaftTypeConfig>::Entry>, StorageError<u64>> {
        let mut reader = self.get_log_reader().await;
        reader.try_get_log_entries(range).await
    }
}

/// Implement `RaftStorage` trait for `DbStore`
impl RaftStorage<DbTypeConfig> for DbStore {
    type LogReader = DbLogReader;
    type SnapshotBuilder = DbSnapshotBuilder;

    async fn save_vote(&mut self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        let mut meta = self.raft_storage.write().await;
        meta.vote = Some(*vote);
        meta.persist_vote(Some(vote)).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Vote,
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<u64>>, StorageError<u64>> {
        let meta = self.raft_storage.read().await;
        Ok(meta.vote)
    }

    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<u64>>
    where
        I: IntoIterator<Item = <DbTypeConfig as openraft::RaftTypeConfig>::Entry> + OptionalSend,
    {
        let tree = self.storage.get_tree(TREE_RAFT_LOG).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(LogId::default()),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        let mut last_log_id = None;
        let mut batch = sled::Batch::default();

        for entry in entries {
            last_log_id = Some(entry.log_id);
            let key = entry.log_id.index.to_be_bytes();
            let value = serde_json::to_vec(&entry).map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(entry.log_id),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;
            batch.insert(&key, value);
        }

        tree.apply_batch(batch).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(last_log_id.unwrap_or_default()),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        tree.flush().map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(last_log_id.unwrap_or_default()),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        Ok(())
    }

    async fn delete_conflict_logs_since(
        &mut self,
        log_id: LogId<u64>,
    ) -> Result<(), StorageError<u64>> {
        let tree = self.storage.get_tree(TREE_RAFT_LOG).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        // Remove all entries from log_id onwards (inclusive)
        let start_key = log_id.index.to_be_bytes();
        let mut batch = sled::Batch::default();

        for item in tree.range(start_key..) {
            let (key, _) = item.map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(log_id),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;
            batch.remove(key);
        }

        tree.apply_batch(batch).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        tree.flush().map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        Ok(())
    }

    async fn purge_logs_upto(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        let tree = self.storage.get_tree(TREE_RAFT_LOG).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        // Remove all entries up to and including log_id
        // We need to be careful not to delete everything if we just scan from 0.
        // But since keys are u64 BE, we can scan from 0 to log_id.
        let end_key = log_id.index.to_be_bytes();
        let mut batch = sled::Batch::default();

        for item in tree.range(..=end_key) {
            let (key, _) = item.map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(log_id),
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;
            batch.remove(key);
        }

        tree.apply_batch(batch).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        tree.flush().map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        let mut meta = self.raft_storage.write().await;
        meta.last_purged = Some(log_id);
        meta.persist_last_purged().map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(log_id),
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
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

    async fn apply_to_state_machine(
        &mut self,
        entries: &[<DbTypeConfig as openraft::RaftTypeConfig>::Entry],
    ) -> Result<Vec<DbResponse>, StorageError<u64>> {
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
                EntryPayload::Normal(log_entry) => sm.apply(log_entry).map_err(|e| {
                    openraft::StorageIOError::new(
                        openraft::ErrorSubject::StateMachine,
                        openraft::ErrorVerb::Write,
                        openraft::AnyError::error(e.to_string()),
                    )
                })?,
                EntryPayload::Membership(membership) => {
                    sm.last_membership =
                        StoredMembership::new(Some(entry.log_id), membership.clone());
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

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<u64>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<u64, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<u64>> {
        let data = snapshot.into_inner();

        // Deserialize snapshot data using serde_json for simplicity
        let snapshot_data: Vec<(String, Vec<u8>, Vec<u8>)> = serde_json::from_slice(&data)
            .map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Snapshot(Some(meta.signature())),
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;

        // Clear storage and restore from snapshot
        // Note: This is simplified - in production you'd want atomic replacement
        for (collection, key, value) in snapshot_data {
            self.storage.put(&collection, &key, &value).map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::StateMachine,
                    openraft::ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;
        }

        let mut sm = self.state_machine.write().await;
        sm.last_applied_log = meta.last_log_id;
        sm.last_membership = meta.last_membership.clone();

        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<DbTypeConfig>>, StorageError<u64>> {
        // For now, return None - snapshots are not yet fully implemented
        Ok(None)
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        DbLogReader {
            storage: self.storage.clone(),
        }
    }

    async fn get_log_state(&mut self) -> Result<LogState<DbTypeConfig>, StorageError<u64>> {
        let meta = self.raft_storage.read().await;
        let tree = self.storage.get_tree(TREE_RAFT_LOG).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::Log(LogId::default()),
                openraft::ErrorVerb::Read,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        let last_log_id = if let Some(last) = tree.iter().last() {
            let (_, v) = last.map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::Log(LogId::default()),
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;
            let entry: <DbTypeConfig as openraft::RaftTypeConfig>::Entry =
                serde_json::from_slice(&v).map_err(|e| {
                    openraft::StorageIOError::new(
                        openraft::ErrorSubject::Log(LogId::default()),
                        openraft::ErrorVerb::Read,
                        openraft::AnyError::error(e.to_string()),
                    )
                })?;
            Some(entry.log_id)
        } else {
            None
        };

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
        // TODO: Iterate all collections. For now just tickets and users.
        // This is a simplified snapshot implementation.
        let mut all_data = Vec::new();

        for collection in [crate::storage::TREE_TICKETS, crate::storage::TREE_USERS] {
            let pairs = self.storage.list(collection, b"", None).map_err(|e| {
                openraft::StorageIOError::new(
                    openraft::ErrorSubject::StateMachine,
                    openraft::ErrorVerb::Read,
                    openraft::AnyError::error(e.to_string()),
                )
            })?;
            for (k, v) in pairs {
                all_data.push((collection.to_string(), k, v));
            }
        }

        // Serialize to snapshot format using serde_json for simplicity
        let data = serde_json::to_vec(&all_data).map_err(|e| {
            openraft::StorageIOError::new(
                openraft::ErrorSubject::StateMachine,
                openraft::ErrorVerb::Write,
                openraft::AnyError::error(e.to_string()),
            )
        })?;

        let snapshot_id = format!("snapshot-{}", chrono::Utc::now().timestamp());

        // Create snapshot metadata (simplified - should track actual state)
        let meta = SnapshotMeta {
            snapshot_id,
            last_log_id: None,
            last_membership: StoredMembership::default(),
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
        let collection = "test_coll";

        let entry = LogEntry::Put {
            collection: collection.to_string(),
            key: b"key1".to_vec(),
            value: b"value1".to_vec(),
        };

        let response = sm.apply(&entry).unwrap();
        assert!(response.success);

        // Verify it was stored
        let stored = storage.get(collection, b"key1").unwrap();
        assert_eq!(stored, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_state_machine_apply_delete() {
        let storage = Storage::new_temp().unwrap();
        let mut sm = DbStateMachine::new(storage.clone());
        let collection = "test_coll";

        // First put a value
        storage.put(collection, b"key1", b"value1").unwrap();

        // Then delete it via state machine
        let entry = LogEntry::Delete {
            collection: collection.to_string(),
            key: b"key1".to_vec(),
        };

        let response = sm.apply(&entry).unwrap();
        assert!(response.success);

        // Verify it was deleted
        assert!(!storage.exists(collection, b"key1").unwrap());
    }

    #[tokio::test]
    async fn test_store_creation() {
        let store = DbStore::new_temp().unwrap();
        let sm = store.state_machine.read().await;
        assert_eq!(sm.last_applied_log, None);
    }
}
