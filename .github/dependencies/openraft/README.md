# OpenRaft - Distributed Consensus for NGI

> A Rust implementation of Raft consensus algorithm for building fault-tolerant distributed systems.

**Official Docs:** https://docs.rs/openraft/latest/openraft/

**Current Version:** 0.9.0+

## Overview

OpenRaft provides distributed consensus for NGI's critical services (DB and Custodian). It ensures strong consistency across replicated instances, tolerates machine failures, and provides automatic leader election.

## NGI Consensus Architecture

### Raft Topology
```
Client Request
    ↓
    ├─→ [Leader] (accepts writes, replicates log)
    │        ↓
    │    Replicate to followers
    │        ↓
    ├─→ [Follower] (accept reads, append log entries)
    │        ↓
    └─→ [Follower] (candidate during election)

Quorum: 3+ instances (tolerates 1 failure)
Election: Automatic on leader failure (<150ms)
```

### Services Using Raft
1. **DB Service** (Port 8080)
   - Replicates ticket database state
   - Ensures consistent reads/writes
   - 3+ instances for quorum

2. **Custodian Service** (Port 8081)
   - Replicates ticket lock state
   - Prevents race conditions
   - 3+ instances for quorum

## Documentation Index

- **[consensus.md](consensus.md)** - Complete OpenRaft consensus guide
  - State machine implementation
  - Log entries and commands
  - RaftNode wrapper patterns
  - gRPC service integration
  - Leader-aware routing (LBRP)
  - Log replication and snapshots
  - Failure scenarios and recovery
  - Testing considerations
  - Official API documentation

## NGI Consensus Architecture

### Raft Topology
```
Client Request
    ↓
    ├─→ [Leader] (accepts writes, replicates log)
    │        ↓
    │    Replicate to followers
    │        ↓
    ├─→ [Follower] (accept reads, append log entries)
    │        ↓
    └─→ [Follower] (candidate during election)

Quorum: 3+ instances (tolerates 1 failure)
Election: Automatic on leader failure (<150ms)
```

### Services Using Raft
1. **DB Service** (Port 8080)
   - Replicates ticket database state
   - Ensures consistent reads/writes
   - 3+ instances for quorum

2. **Custodian Service** (Port 8081)
   - Replicates ticket lock state
   - Prevents race conditions
   - 3+ instances for quorum

## Core Components

### State Machine (Application Logic)
```rust
pub struct TicketStateMachine {
    tickets: HashMap<u64, Ticket>,
    locks: HashMap<u64, LockInfo>,
}

impl RaftStateMachine for TicketStateMachine {
    type D = LogEntry;
    type R = CommandResponse;
    
    async fn apply(&mut self, entries: Vec<Self::D>) -> Result<Vec<Self::R>> {
        let mut responses = Vec::new();
        for entry in entries {
            match entry.command {
                Command::InsertTicket(ticket) => {
                    self.tickets.insert(ticket.id, ticket);
                    responses.push(CommandResponse::Ok);
                }
                Command::AcquireLock(ticket_id, user_id) => {
                    if self.locks.contains_key(&ticket_id) {
                        responses.push(CommandResponse::AlreadyLocked);
                    } else {
                        self.locks.insert(ticket_id, LockInfo { owner: user_id });
                        responses.push(CommandResponse::Ok);
                    }
                }
            }
        }
        Ok(responses)
    }
}
```

### Raft Node Configuration
```rust
use openraft::Config;

let config = Config {
    // Election timeout: 150-300ms
    election_timeout_min: 150,
    election_timeout_max: 300,
    
    // Heartbeat interval: 50-100ms
    heartbeat_interval: 50,
    
    // Log snapshot threshold
    snapshot_policy: SnapshotPolicy::LogsSinceLast(1000),
};

let raft = Raft::new(
    node_id,
    config,
    storage,
    state_machine,
)?;
```

### Log Storage (Persistent)
```rust
pub struct LogStorage {
    db: sled::Db,
    logs: sled::Tree,
}

impl RaftLogStorage for LogStorage {
    type LogEntry = LogEntry;
    
    async fn append_log_entries(&mut self, entries: Vec<LogEntry>) -> Result<()> {
        let mut batch = sled::Batch::default();
        for entry in entries {
            let key = format!("log:{}", entry.log_index);
            batch.insert(key.as_bytes(), bincode::encode_to_vec(&entry, BINCODE_CONFIG)?);
        }
        self.logs.apply_batch(batch)?;
        Ok(())
    }
}
```

### Snapshot Mechanism
```rust
impl RaftSnapshotStorage for SnapshotStorage {
    async fn create_snapshot(&self, index: u64, state: &StateMachine) -> Result<()> {
        let snapshot_id = format!("snapshot:{}", index);
        let encoded = bincode::encode_to_vec(&state, BINCODE_CONFIG)?;
        self.db.insert(snapshot_id.as_bytes(), encoded)?;
        Ok(())
    }
    
    async fn load_snapshot(&self, index: u64) -> Result<StateMachine> {
        let snapshot_id = format!("snapshot:{}", index);
        let encoded = self.db.get(snapshot_id.as_bytes())?;
        bincode::decode_from_slice(&encoded, BINCODE_CONFIG)?.0
    }
}
```

## Replication Flow

### Write Path (Client → Leader → Followers)
```
1. Client sends write request to leader
   ↓
2. Leader appends to local log
   ↓
3. Leader sends log entry to all followers
   ↓
4. Followers append to their logs and ACK
   ↓
5. Leader commits entry (quorum ACK'd)
   ↓
6. Leader applies to state machine
   ↓
7. Leader sends response to client
```

### Read Path (Leader Only for Consistency)
```
1. Client sends read request
   ↓
2. Leader checks if it's still leader (heartbeat confirmation)
   ↓
3. Leader reads from state machine
   ↓
4. Leader sends response to client
```

## Leadership & Election

### Automatic Leadership
```rust
// After network partition or crash
// Followers become candidates if no heartbeat for election_timeout
// Candidates request votes from other nodes
// First to get quorum (3/5 or 2/3) becomes leader

let now_leader = raft.is_leader().await;
if now_leader {
    println!("I'm the leader!");
}
```

### Leader Failure Recovery
```
1. Followers detect no heartbeat (150-300ms timeout)
   ↓
2. Random follower becomes candidate
   ↓
3. Candidate requests votes
   ↓
4. Followers vote for first candidate with higher term
   ↓
5. New leader elected
   ↓
6. New leader replicates logs to followers
   ↓
Total time: ~200-500ms (bounded by election_timeout_max + jitter)
```

## NGI Implementation Pattern

### gRPC Service Wrapper
```rust
use tonic::{Request, Response, Status};
use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse};

pub struct RaftNode {
    raft: Raft<TypeConfig>,
}

#[tonic::async_trait]
impl DbRaft for RaftNode {
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();
        let resp = self.raft.append_entries(req).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }
    
    async fn install_snapshot(
        &self,
        request: Request<InstallSnapshotRequest>,
    ) -> Result<Response<InstallSnapshotResponse>, Status> {
        // Handle snapshot replication
        Ok(Response::new(...))
    }
}
```

### Leader-Aware Client Routing
```rust
// LBRP identifies leader by polling instances
async fn find_raft_leader(instances: &[String]) -> Result<String> {
    for instance in instances {
        let channel = Channel::from_static(instance).connect().await?;
        let mut client = DbRaftClient::new(channel);
        
        match client.is_leader(Empty {}).await {
            Ok(response) if response.into_inner().is_leader => {
                return Ok(instance.to_string());
            }
            _ => continue,
        }
    }
    Err("No leader found")
}

// Route writes to leader
pub async fn write_ticket(ticket: Ticket) -> Result<()> {
    let leader = find_raft_leader(&DB_INSTANCES).await?;
    let channel = Channel::from_static(&leader).connect().await?;
    let mut client = DbRaftClient::new(channel);
    client.apply_command(ApplyRequest { command }).await?;
    Ok(())
}
```

## Failure Scenarios Handled

| Scenario | Behavior |
|----------|----------|
| Leader crashes | Followers elect new leader (~300ms) |
| Network partition | Isolated partition stops accepting writes |
| Follower lag | Leader retransmits log entries |
| Log divergence | Conflicting entries overwritten |
| All replicas down | System unavailable (no quorum) |

## Consistency Guarantees

1. **Strong Consistency** - All replicas see same data in same order
2. **Durability** - Committed entries survive node failures
3. **Liveness** - System makes progress if quorum available
4. **Safety** - No data loss or corruption

## Configuration for NGI

```rust
use openraft::Config;

pub struct NgiBuildConfig;

impl openraft::RaftTypeConfig for NgiBuildConfig {
    type D = LogEntry;  // Your log entry type
    type R = CommandResponse;  // Your response type
    type NodeId = NodeId;
    type Node = Node;
    
    type SnapshotData = Vec<u8>;  // Snapshot format
    type AsyncRuntime = TokioRuntime;
}

// Recommended Raft config for 3-node cluster
let config = Config {
    election_timeout_min: 150,
    election_timeout_max: 300,
    heartbeat_interval: 50,
    snapshot_policy: SnapshotPolicy::LogsSinceLast(500),
    max_payload_entries: 300,
    replication_lag_threshold: 128,
    enable_heartbeat: true,
};
```

## Testing Raft Behavior

```rust
#[tokio::test]
async fn test_leader_election() {
    // Start 3 Raft nodes
    let mut raft_nodes = vec![];
    for node_id in 0..3 {
        let node = create_raft_node(node_id).await;
        raft_nodes.push(node);
    }
    
    // Node 0 should become leader
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(raft_nodes[0].is_leader().await);
    
    // Kill leader
    drop(raft_nodes.remove(0));
    
    // New leader elected
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(raft_nodes[0].is_leader().await || raft_nodes[1].is_leader().await);
}

#[tokio::test]
async fn test_log_replication() {
    let mut cluster = create_raft_cluster(3).await;
    
    // Write to leader
    let entry = LogEntry { command: CreateTicket(42) };
    cluster.nodes[0].append_entries(vec![entry]).await?;
    
    // Wait for replication
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Verify all nodes have entry
    for node in &cluster.nodes {
        let has_entry = node.get_log_entry(0).await?;
        assert!(has_entry.is_some());
    }
}
```

## Snapshots for Performance

```rust
// Periodically snapshot state machine to avoid replaying entire log
pub async fn create_snapshot(&mut self, index: u64) -> Result<()> {
    // Serialize current state machine
    let snapshot_data = bincode::encode_to_vec(&self.state_machine, BINCODE_CONFIG)?;
    
    // Store with index
    self.snapshot_storage.create_snapshot(index, &snapshot_data).await?;
    
    // Truncate log up to snapshot index
    self.log_storage.trim_log(index).await?;
    
    Ok(())
}

// On startup, load snapshot then replay remaining log
pub async fn recover() -> Result<StateMachine> {
    if let Ok(snapshot) = self.snapshot_storage.latest_snapshot().await {
        let mut state = bincode::decode_from_slice(&snapshot, BINCODE_CONFIG)?.0;
        
        // Replay logs after snapshot
        let remaining_logs = self.log_storage.get_logs_after(snapshot.index).await?;
        state.apply(remaining_logs).await?;
        
        return Ok(state);
    }
    
    // No snapshot, replay entire log
    Ok(StateMachine::default())
}
```

## References

- **Official Documentation:**
  - [Raft protocol overview](https://docs.rs/openraft/latest/openraft/) - Core concepts
  - [TypeConfig](https://docs.rs/openraft/latest/openraft/type_config/) - Configuration trait
  - [Storage traits](https://docs.rs/openraft/latest/openraft/storage/) - Persistence layer

- **NGI Services:**
  - [db/src/raft.rs](../../../db/src/raft.rs) - DB service Raft implementation
  - [custodian/src/](../../../custodian/src/) - Custodian Raft service

---

**Last Updated:** December 2025  
**Documentation Version:** OpenRaft 0.9.0
