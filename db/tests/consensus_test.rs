//! Consensus enhancement tests
//!
//! These tests verify Raft consensus behavior and fault tolerance.
//! Note: Full multi-node testing requires network implementation.

use db::network::DbNetworkFactory;
use db::raft::{DbRaft, DbStore};
use db::storage::LogEntry;
use openraft::{storage::Adaptor, Config};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

/// Helper to create a test Raft node
async fn create_test_raft_node(node_id: u64) -> anyhow::Result<DbRaft> {
    let store = DbStore::new_temp()?;
    let config = Arc::new(Config {
        heartbeat_interval: 100,
        election_timeout_min: 300,
        election_timeout_max: 500,
        ..Default::default()
    }.validate()?);

    let network_factory = DbNetworkFactory::new();
    let (log_store, state_machine) = Adaptor::new(store);

    Ok(DbRaft::new(node_id, config, network_factory, log_store, state_machine).await?)
}

/// Test single-node cluster behavior (foundation for multi-node)
#[tokio::test]
async fn test_single_node_cluster_operations() {
    let raft = create_test_raft_node(1).await.unwrap();

    // Initialize single-node cluster
    let mut members = std::collections::BTreeSet::new();
    members.insert(1u64);
    raft.initialize(members).await.unwrap();

    // Wait for leader election
    timeout(Duration::from_secs(5), async {
        loop {
            let metrics = raft.metrics().borrow().clone();
            if metrics.current_leader.is_some() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("Leader election timeout");

    let metrics = raft.metrics().borrow().clone();
    assert_eq!(metrics.id, 1);
    assert_eq!(metrics.current_leader, Some(1));
    assert!(metrics.current_term >= 1);
}

/// Test consensus write operations
#[tokio::test]
async fn test_consensus_write_operations() {
    let raft = create_test_raft_node(1).await.unwrap();

    // Initialize cluster
    let mut members = std::collections::BTreeSet::new();
    members.insert(1u64);
    raft.initialize(members).await.unwrap();

    // Wait for leader
    timeout(Duration::from_secs(5), async {
        loop {
            let metrics = raft.metrics().borrow().clone();
            if metrics.current_leader.is_some() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("Leader election timeout");

    // Test various write operations
    let test_cases = vec![
        LogEntry::Put {
            collection: "test".to_string(),
            key: b"key1".to_vec(),
            value: b"value1".to_vec(),
        },
        LogEntry::Delete {
            collection: "test".to_string(),
            key: b"key1".to_vec(),
        },
        LogEntry::BatchPut {
            collection: "test".to_string(),
            pairs: vec![
                (b"batch_key1".to_vec(), b"batch_value1".to_vec()),
                (b"batch_key2".to_vec(), b"batch_value2".to_vec()),
            ],
        },
    ];

    for entry in test_cases {
        raft.client_write(entry).await.unwrap();
    }

    // Verify operations were recorded
    let metrics = raft.metrics().borrow().clone();
    assert!(metrics.last_log_index.is_some());
    assert!(metrics.last_log_index.unwrap() >= 3); // At least 3 log entries
}

/// Test Raft state machine application
#[tokio::test]
async fn test_state_machine_application() {
    let store = DbStore::new_temp().unwrap();
    let state_machine = store.state_machine().clone();

    // Apply various log entries
    let entries = vec![
        LogEntry::Put {
            collection: "test".to_string(),
            key: b"sm_key1".to_vec(),
            value: b"sm_value1".to_vec(),
        },
        LogEntry::Put {
            collection: "test".to_string(),
            key: b"sm_key2".to_vec(),
            value: b"sm_value2".to_vec(),
        },
        LogEntry::Delete {
            collection: "test".to_string(),
            key: b"sm_key1".to_vec(),
        },
    ];

    for entry in entries {
        let mut sm = state_machine.write().await;
        let response = sm.apply(&entry).unwrap();
        assert!(response.success);
    }

    // Verify final state
    let sm = state_machine.read().await;
    let storage = &sm.storage;

    // Key1 should be deleted
    assert_eq!(storage.get("test", b"sm_key1").unwrap(), None);

    // Key2 should exist
    assert_eq!(
        storage.get("test", b"sm_key2").unwrap(),
        Some(b"sm_value2".to_vec())
    );
}

/// Test log persistence and recovery (simplified)
#[tokio::test]
async fn test_log_persistence_and_recovery() {
    let store = DbStore::new_temp().unwrap();
    let config = Arc::new(Config {
        heartbeat_interval: 100,
        election_timeout_min: 300,
        election_timeout_max: 500,
        ..Default::default()
    }.validate().unwrap());

    let network_factory = DbNetworkFactory::new();
    let (log_store, state_machine) = Adaptor::new(store.clone());

    let raft = DbRaft::new(1, config, network_factory, log_store, state_machine).await.unwrap();

    // Initialize cluster
    let mut members = std::collections::BTreeSet::new();
    members.insert(1u64);
    raft.initialize(members).await.unwrap();

    // Wait for leader
    timeout(Duration::from_secs(5), async {
        loop {
            let metrics = raft.metrics().borrow().clone();
            if metrics.current_leader.is_some() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("Leader election timeout");

    // Write some data
    let entry = LogEntry::Put {
        collection: "persistent".to_string(),
        key: b"persist_key".to_vec(),
        value: b"persist_value".to_vec(),
    };
    raft.client_write(entry).await.unwrap();

    // Verify data is in state machine
    let state_machine = store.state_machine();
    let sm = state_machine.read().await;
    assert_eq!(
        sm.storage.get("persistent", b"persist_key").unwrap(),
        Some(b"persist_value".to_vec())
    );

    // Verify log was recorded
    let metrics = raft.metrics().borrow().clone();
    assert!(metrics.last_log_index.is_some());
    assert!(metrics.last_log_index.unwrap() >= 1);
}