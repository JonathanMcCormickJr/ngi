//! Integration tests for the DB service
//!
//! These tests verify the full service functionality including:
//! - Single-node Raft initialization
//! - gRPC API operations through Raft consensus
//! - Storage persistence
//! - Cluster status reporting

use db::raft::{DbRaft, DbStore};
use openraft::{Config, storage::Adaptor};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// Re-export network module access
mod helpers {
    use db::network::DbNetworkFactory;
    use db::raft::{DbRaft, DbStore};
    use openraft::{Config, storage::Adaptor};
    use std::sync::Arc;

    pub async fn create_test_raft_node(node_id: u64) -> anyhow::Result<DbRaft> {
        let store = DbStore::new_temp()?;
        let config = Arc::new(
            Config {
                heartbeat_interval: 100,
                election_timeout_min: 300,
                election_timeout_max: 500,
                ..Default::default()
            }
            .validate()?,
        );

        let network = DbNetworkFactory::new();
        let (log_store, state_machine) = Adaptor::new(store);

        Ok(DbRaft::new(node_id, config, network, log_store, state_machine).await?)
    }
}

#[tokio::test]
async fn test_single_node_initialization() {
    let raft = helpers::create_test_raft_node(1).await.unwrap();

    // Initialize single-node cluster
    let mut nodes = std::collections::BTreeSet::new();
    nodes.insert(1);
    raft.initialize(nodes).await.unwrap();

    // Give it a moment to become leader
    sleep(Duration::from_millis(200)).await;

    // Check metrics
    let metrics = raft.metrics().borrow().clone();
    assert_eq!(metrics.id, 1);
    assert!(metrics.current_leader.is_some());
}

#[tokio::test]
async fn test_raft_state_machine_operations() {
    let store = DbStore::new_temp().unwrap();
    let config = Arc::new(
        Config {
            heartbeat_interval: 100,
            election_timeout_min: 300,
            election_timeout_max: 500,
            ..Default::default()
        }
        .validate()
        .unwrap(),
    );

    let network = db::network::DbNetworkFactory::new();
    let (log_store, state_machine) = Adaptor::new(store.clone());

    let raft = DbRaft::new(1, config, network, log_store, state_machine)
        .await
        .unwrap();

    // Initialize cluster
    let mut nodes = std::collections::BTreeSet::new();
    nodes.insert(1);
    raft.initialize(nodes).await.unwrap();

    sleep(Duration::from_millis(200)).await;

    // Test write operation through Raft
    let entry = db::storage::LogEntry::Put {
        collection: "test".to_string(),
        key: b"test_key".to_vec(),
        value: b"test_value".to_vec(),
    };

    let response = raft.client_write(entry).await;
    assert!(response.is_ok());

    // Verify data was stored
    let storage = store.state_machine().read().await.storage.clone();
    let value = storage.get("test", b"test_key").unwrap();
    assert_eq!(value, Some(b"test_value".to_vec()));
}

#[tokio::test]
async fn test_storage_persistence() {
    // Create temporary storage
    let temp_dir = tempfile::tempdir().unwrap();
    let storage_path = temp_dir.path().join("test_db");

    {
        // Create store and write data
        let store = DbStore::new(storage_path.to_str().unwrap()).unwrap();
        let storage = store.state_machine().read().await.storage.clone();

        storage.put("test", b"key1", b"value1").unwrap();
        storage.put("test", b"key2", b"value2").unwrap();
    }

    {
        // Reopen store and verify data persisted
        let store = DbStore::new(storage_path.to_str().unwrap()).unwrap();
        let storage = store.state_machine().read().await.storage.clone();

        assert_eq!(
            storage.get("test", b"key1").unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            storage.get("test", b"key2").unwrap(),
            Some(b"value2".to_vec())
        );
    }
}

#[tokio::test]
async fn test_batch_operations() {
    let store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Test batch put
    let pairs = vec![
        (b"batch1".to_vec(), b"value1".to_vec()),
        (b"batch2".to_vec(), b"value2".to_vec()),
        (b"batch3".to_vec(), b"value3".to_vec()),
    ];

    storage.batch_put("test", &pairs).unwrap();

    // Verify all were stored
    assert_eq!(
        storage.get("test", b"batch1").unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        storage.get("test", b"batch2").unwrap(),
        Some(b"value2".to_vec())
    );
    assert_eq!(
        storage.get("test", b"batch3").unwrap(),
        Some(b"value3".to_vec())
    );
}

#[tokio::test]
async fn test_list_operations() {
    let store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Store data with common prefix
    storage.put("test", b"prefix:key1", b"value1").unwrap();
    storage.put("test", b"prefix:key2", b"value2").unwrap();
    storage.put("test", b"prefix:key3", b"value3").unwrap();
    storage.put("test", b"other:key", b"other").unwrap();

    // List with prefix
    let results = storage.list("test", b"prefix:", Some(10)).unwrap();
    assert_eq!(results.len(), 3);

    // List all
    let all_results = storage.list("test", b"", Some(10)).unwrap();
    assert_eq!(all_results.len(), 4);

    // List with limit
    let limited = storage.list("test", b"", Some(2)).unwrap();
    assert_eq!(limited.len(), 2);
}

#[tokio::test]
async fn test_delete_operations() {
    let store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Store and delete
    storage.put("test", b"to_delete", b"value").unwrap();
    assert!(storage.exists("test", b"to_delete").unwrap());

    storage.delete("test", b"to_delete").unwrap();
    assert!(!storage.exists("test", b"to_delete").unwrap());
    assert_eq!(storage.get("test", b"to_delete").unwrap(), None);
}

#[tokio::test]
async fn test_log_entry_application() {
    use db::storage::LogEntry;

    let store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Test Put
    let put_entry = LogEntry::Put {
        collection: "test".to_string(),
        key: b"log_key".to_vec(),
        value: b"log_value".to_vec(),
    };
    put_entry.apply(&storage).unwrap();
    assert_eq!(
        storage.get("test", b"log_key").unwrap(),
        Some(b"log_value".to_vec())
    );

    // Test Delete
    let delete_entry = LogEntry::Delete {
        collection: "test".to_string(),
        key: b"log_key".to_vec(),
    };
    delete_entry.apply(&storage).unwrap();
    assert_eq!(storage.get("test", b"log_key").unwrap(), None);

    // Test BatchPut
    let batch_entry = LogEntry::BatchPut {
        collection: "test".to_string(),
        pairs: vec![
            (b"batch_a".to_vec(), b"value_a".to_vec()),
            (b"batch_b".to_vec(), b"value_b".to_vec()),
        ],
    };
    batch_entry.apply(&storage).unwrap();
    assert_eq!(
        storage.get("test", b"batch_a").unwrap(),
        Some(b"value_a".to_vec())
    );
    assert_eq!(
        storage.get("test", b"batch_b").unwrap(),
        Some(b"value_b".to_vec())
    );
}

#[tokio::test]
async fn test_raft_metrics() {
    let raft = helpers::create_test_raft_node(42).await.unwrap();

    let mut nodes = std::collections::BTreeSet::new();
    nodes.insert(42);
    raft.initialize(nodes).await.unwrap();

    sleep(Duration::from_millis(200)).await;

    let metrics = raft.metrics().borrow().clone();

    // Verify node ID
    assert_eq!(metrics.id, 42);

    // Should be leader of single-node cluster
    assert!(metrics.current_leader.is_some());

    // Check that term has progressed
    assert!(metrics.current_term > 0);
}

#[tokio::test]
async fn test_concurrent_operations() {
    let store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Spawn multiple concurrent writes
    let mut handles = vec![];
    for i in 0..10 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            let key = format!("concurrent_key_{}", i);
            let value = format!("concurrent_value_{}", i);
            storage_clone
                .put("test", key.as_bytes(), value.as_bytes())
                .unwrap();
        });
        handles.push(handle);
    }

    // Wait for all writes
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all writes succeeded
    for i in 0..10 {
        let key = format!("concurrent_key_{}", i);
        let value = format!("concurrent_value_{}", i);
        assert_eq!(
            storage.get("test", key.as_bytes()).unwrap(),
            Some(value.as_bytes().to_vec())
        );
    }
}

#[tokio::test]
async fn test_snapshot_builder() {
    use openraft::{RaftSnapshotBuilder, RaftStorage};

    let mut store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Add some data
    storage.put("test", b"snap_key1", b"snap_value1").unwrap();
    storage.put("test", b"snap_key2", b"snap_value2").unwrap();

    // Get snapshot builder
    let mut builder = store.get_snapshot_builder().await;

    // Build snapshot
    let snapshot = builder.build_snapshot().await.unwrap();

    // Verify snapshot has data
    let snapshot_data = snapshot.snapshot.into_inner();
    assert!(!snapshot_data.is_empty());
}

#[tokio::test]
async fn test_error_handling() {
    let store = DbStore::new_temp().unwrap();
    let storage = store.state_machine().read().await.storage.clone();

    // Test getting non-existent key
    let result = storage.get("test", b"nonexistent").unwrap();
    assert_eq!(result, None);

    // Test exists on non-existent key
    let exists = storage.exists("test", b"nonexistent").unwrap();
    assert!(!exists);

    // Test deleting non-existent key (should not error)
    let result = storage.delete("test", b"nonexistent");
    assert!(result.is_ok());
}
