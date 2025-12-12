# OpenRaft 0.9

**Repository:** https://github.com/datafuselabs/openraft  
**Documentation:** https://docs.rs/openraft/  
**Crates.io:** https://crates.io/crates/openraft

## Version in NGI
```toml
openraft = { version = "0.9", features = ["serde"] }
```

## Overview
OpenRaft is a Rust implementation of the Raft consensus algorithm. It provides a pluggable state machine interface to build strongly consistent, fault-tolerant distributed systems. NGI uses it for the DB and Custodian services.

## Key Concepts

### Services Using Raft
- **DB Service** (Port 8080): 3+ instances for data persistence
- **Custodian Service** (Port 8081): 3+ instances for distributed locking

### Quorum
- Minimum 3 instances (1 leader + 2 followers)
- Tolerates 1 failure
- Requires 2+ votes for leader election

## Core Components

### 1. State Machine
The state machine applies log entries and produces results.

```rust
use openraft::RaftStateMachine;

pub struct TicketStateMachine {
    data: HashMap<u64, Ticket>,
}

#[async_trait]
impl RaftStateMachine for TicketStateMachine {
    type D = LogEntry;           // Log entry type
    type R = CommandResponse;    // Response type
    type E = Error;              // Error type

    async fn apply(&mut self, entries: Vec<Self::D>) -> Result<Vec<Self::R>> {
        let mut responses = Vec::new();
        
        for entry in entries {
            match entry.command {
                Command::InsertTicket(ticket) => {
                    self.data.insert(ticket.id, ticket);
                    responses.push(CommandResponse::Ok);
                }
                Command::UpdateTicket(ticket) => {
                    self.data.insert(ticket.id, ticket);
                    responses.push(CommandResponse::Ok);
                }
            }
        }
        
        Ok(responses)
    }
}
```

### 2. Log Entry
Represents a command to apply to the state machine.

```rust
#[derive(Serialize, Deserialize)]
pub struct LogEntry {
    pub command: Command,
}

#[derive(Serialize, Deserialize)]
pub enum Command {
    InsertTicket(Ticket),
    UpdateTicket(Ticket),
    AcquireLock(u64, UserId),
    ReleaseLock(u64),
}
```

### 3. RaftNode
Wraps the Raft state machine and handles consensus.

```rust
use openraft::{Raft, RaftTypeConfig};

pub struct RaftNode {
    raft: Raft<TypeConfig>,
    sm: Arc<RwLock<TicketStateMachine>>,
}

impl RaftNode {
    pub async fn append_entry(&self, command: Command) -> Result<()> {
        let entry = LogEntry { command };
        self.raft.append_entries(vec![entry]).await?;
        Ok(())
    }

    pub async fn is_leader(&self) -> bool {
        self.raft.is_leader().await
    }

    pub async fn metrics(&self) -> Result<RaftMetrics> {
        Ok(self.raft.metrics().borrow().clone())
    }
}
```

## gRPC Service Integration

```rust
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl db_server::Db for DbServiceImpl {
    async fn update_ticket(
        &self,
        request: Request<UpdateTicketRequest>,
    ) -> Result<Response<Ticket>, Status> {
        let req = request.into_inner();
        
        // Append to Raft log
        let command = Command::UpdateTicket(req.ticket);
        self.raft_node.append_entry(command)
            .await
            .map_err(|e| Status::internal(format!("raft error: {}", e)))?;
        
        Ok(Response::new(updated_ticket))
    }
}
```

## Leader-Aware Routing (LBRP)

The Load Balancer must route write operations to the current leader:

```rust
async fn find_leader(instances: &[String]) -> Result<String> {
    for instance in instances {
        if is_leader(instance).await? {
            return Ok(instance.clone());
        }
    }
    Err(Error::NoLeaderElected)
}

async fn route_write(instances: &[String], req: WriteRequest) -> Result<Response> {
    let leader = find_leader(instances).await?;
    let channel = Channel::from_static(&leader).connect().await?;
    let mut client = DbClient::new(channel);
    client.write(req).await
}
```

## Log Replication

All log entries are replicated to followers before being applied to the state machine:

1. Leader receives entry
2. Sends entry to all followers
3. Waits for quorum acknowledgment
4. Applies to state machine once committed
5. Followers apply to their state machines

## Log Compaction & Snapshots

To prevent unbounded log growth:

```rust
pub async fn snapshot(&self) -> Result<Snapshot> {
    let state = self.sm.read().await.clone();
    let data = bincode::serialize(&state)?;
    Ok(Snapshot {
        last_included_term: current_term,
        last_included_index: current_index,
        data,
    })
}
```

## Best Practices
1. Always append through Raft, never directly to state machine
2. Followers serve reads from their state machine (eventual consistency)
3. Leaders serve strongly consistent reads
4. Handle leader election failures gracefully (return errors, don't panic)
5. Monitor Raft metrics for health
6. Implement proper timeouts for RPC calls

## Failure Scenarios
- **Leader Failure:** Followers detect via heartbeat timeout and hold new election
- **Network Partition:** Split brain prevented by quorum requirement
- **Follower Crash:** Rejoins cluster and catches up via log replication
- **Log Divergence:** Followers overwrite divergent entries from leader

## Testing Considerations
- Simulate leader failures in integration tests
- Test network partitions with chaos service
- Verify eventual consistency across cluster
- Measure recovery time and cluster convergence

---

## Official API Documentation

### Core Types

- **[Raft](https://docs.rs/openraft/latest/openraft/raft/struct.Raft.html)** - Main Raft API
  - Methods:
    - [append_entries](https://docs.rs/openraft/latest/openraft/raft/struct.Raft.html#method.append_entries) - Append log entries
    - [is_leader](https://docs.rs/openraft/latest/openraft/raft/struct.Raft.html#method.is_leader) - Check if node is leader
    - [metrics](https://docs.rs/openraft/latest/openraft/raft/struct.Raft.html#method.metrics) - Get Raft metrics
    - [change_membership](https://docs.rs/openraft/latest/openraft/raft/struct.Raft.html#method.change_membership) - Change cluster membership
    - [wait_for_metrics](https://docs.rs/openraft/latest/openraft/raft/struct.Raft.html#method.wait_for_metrics) - Wait for specific metrics

- **[RaftMetrics](https://docs.rs/openraft/latest/openraft/metrics/struct.RaftMetrics.html)** - Observability metrics
  - Fields: state, current_leader, last_log_index, last_log_term, etc.

- **[Entry](https://docs.rs/openraft/latest/openraft/entry/struct.Entry.html)** - Log entry
  - Contains log_id, payload, etc.

- **[LogId](https://docs.rs/openraft/latest/openraft/log_id/struct.LogId.html)** - Log entry identifier
  - term and index

- **[Vote](https://docs.rs/openraft/latest/openraft/struct.Vote.html)** - Node voting privilege

- **[Membership](https://docs.rs/openraft/latest/openraft/struct.Membership.html)** - Cluster membership config

- **[RaftState](https://docs.rs/openraft/latest/openraft/struct.RaftState.html)** - Node state for persistence

### Traits (Must Implement)

- **[RaftTypeConfig](https://docs.rs/openraft/latest/openraft/type_config/trait.RaftTypeConfig.html)** - Type configuration
  - Associated types: NodeId, Node, Entry, SnapshotData, etc.

- **[RaftStateMachine](https://docs.rs/openraft/latest/openraft/docs/components/state_machine/index.html)** - Application state machine
  - Methods:
    - [apply](https://docs.rs/openraft/latest/openraft/docs/components/state_machine/index.html) - Apply log entries
    - [snapshot](https://docs.rs/openraft/latest/openraft/docs/components/state_machine/index.html) - Create snapshot
    - [restore](https://docs.rs/openraft/latest/openraft/docs/components/state_machine/index.html) - Restore from snapshot

- **[RaftLogReader](https://docs.rs/openraft/latest/openraft/storage/trait.RaftLogReader.html)** - Read-only log access
  - Methods: try_get_log_entries, etc.

- **[RaftNetwork](https://docs.rs/openraft/latest/openraft/network/trait.RaftNetwork.html)** - Network communication
  - Methods: append_entries, install_snapshot, vote, etc.

### Enums

- **[ServerState](https://docs.rs/openraft/latest/openraft/enum.ServerState.html)** - Node role
  - Follower, Candidate, Leader values

- **[StorageError](https://docs.rs/openraft/latest/openraft/enum.StorageError.html)** - Storage operation errors

### Documentation Modules

- **[getting_started](https://docs.rs/openraft/latest/openraft/docs/getting_started/index.html)** - Tutorial for new users
- **[cluster_control](https://docs.rs/openraft/latest/openraft/docs/cluster_control/index.html)** - Manage cluster membership
- **[protocol](https://docs.rs/openraft/latest/openraft/docs/protocol/index.html)** - Protocol details
- **[upgrade_guide](https://docs.rs/openraft/latest/openraft/docs/upgrade_guide/index.html)** - Version upgrades
