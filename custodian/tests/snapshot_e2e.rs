use custodian::raft::{CustodianStore, CustodianTypeConfig};
use custodian::storage::{TREE_LOCKS};
use openraft::{RaftStorage, RaftSnapshotBuilder};

#[tokio::test]
async fn test_snapshot_build_and_install_e2e() {
    // Leader store with some data
    let leader = CustodianStore::new_temp().expect("create leader store");
    let leader_storage = leader.storage();
    // insert a key that should be captured by snapshot
    leader_storage.put(TREE_LOCKS, b"lock-42", b"owner-xyz").expect("put");

    // Build snapshot using the store's snapshot builder
    let mut leader_for_builder = leader.clone();
    let mut builder = <CustodianStore as RaftStorage<CustodianTypeConfig>>::
        get_snapshot_builder(&mut leader_for_builder).await;

    let snapshot = builder.build_snapshot().await.expect("build snapshot");

    // Create follower store and install the snapshot
    let mut follower = CustodianStore::new_temp().expect("create follower store");
    // Install snapshot directly into follower storage via RaftStorage impl
    <CustodianStore as RaftStorage<CustodianTypeConfig>>::
        install_snapshot(&mut follower, &snapshot.meta, snapshot.snapshot).await
        .expect("install snapshot");

    // Verify follower now has the key
    let follower_storage = follower.storage();
    let got = follower_storage.get(TREE_LOCKS, b"lock-42").expect("get");
    assert!(got.is_some(), "snapshot data not present on follower");
    assert_eq!(got.unwrap(), b"owner-xyz".to_vec());
}
