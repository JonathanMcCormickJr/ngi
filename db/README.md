# DB Service

Distributed key-value database service with Raft consensus for the NGI ticketing system.

## Overview

The DB service provides a consistent, distributed key-value store using:
- **Sled** for local storage backend
- **OpenRaft** for distributed consensus
- **gRPC** (tonic) for inter-node communication
- **Protobuf** for efficient serialization

## Architecture

```
┌─────────────────┐
│  gRPC Server    │  ← Database service (8 RPCs)
├─────────────────┤
│  Raft Layer     │  ← Consensus (leader election, log replication)
├─────────────────┤
│  State Machine  │  ← Applies committed log entries
├─────────────────┤
│  Storage (Sled) │  ← Persistent key-value store
└─────────────────┘
```

## Features

### gRPC API (8 Methods)

1. **Put** - Store key-value pair (consensus write)
2. **Get** - Retrieve value by key (local read)
3. **Delete** - Remove key (consensus write)
4. **List** - List key-value pairs with prefix filter
5. **Exists** - Check if key exists
6. **BatchPut** - Bulk write operation
7. **Health** - Node health check (returns role: leader/follower/candidate)
8. **ClusterStatus** - Cluster state (leader, members, term, commit index)

### Storage Layer

- **Backend**: Sled embedded database
- **Operations**: put, get, delete, list, exists, batch_put
- **Durability**: All writes flushed to disk
- **Testing**: 5 comprehensive unit tests

### Raft Integration

- **Type Configuration**: Using `declare_raft_types!` macro
- **State Machine**: Applies log entries (Put, Get, Delete, BatchPut) to storage
- **Network Layer**: Skeleton for gRPC-based node communication
- **Consensus**: Ready for multi-node cluster deployment

## Configuration

Environment variables:

- `NODE_ID` - Unique node identifier (default: 1)
- `LISTEN_ADDR` - gRPC server address (default: [::1]:50051)
- `STORAGE_PATH` - Data directory path (default: /tmp/ngi-db-{NODE_ID})

## Testing

```bash
# Run all tests
cargo test -p db

# Run with coverage
cargo tarpaulin -p db

# Build
cargo build -p db

# Test
cargo test -p db                          # Unit tests
cargo test -p db --test integration_test  # Integration tests
```

**Test Coverage:** 21 tests passing (10 unit + 11 integration)
- **Unit Tests:**
  - Storage: 5 tests (put/get, delete, list, batch, log entries)
  - Raft: 3 tests (state machine put/delete, store creation)
  - Network: 1 test (factory creation)
  - Server: 1 test (service creation)
- **Integration Tests:**
  - Single-node Raft initialization
  - State machine operations through consensus
  - Storage persistence across restarts
  - Batch operations
  - List/delete operations
  - Concurrent operations
  - Snapshot building
  - Error handling
  - Raft metrics reporting
- **Code Coverage:** 53.83% (232/431 lines), Raft module: 57.9%

## Deployment

### Single Node (Development)

```bash
NODE_ID=1 LISTEN_ADDR="127.0.0.1:50051" STORAGE_PATH="./data/node1" ./db
```

### Cluster (3 Nodes - Production)

```bash
# Node 1 (Leader candidate)
NODE_ID=1 LISTEN_ADDR="10.0.0.1:50051" STORAGE_PATH="/var/lib/ngi-db/node1" ./db

# Node 2
NODE_ID=2 LISTEN_ADDR="10.0.0.2:50051" STORAGE_PATH="/var/lib/ngi-db/node2" ./db

# Node 3
NODE_ID=3 LISTEN_ADDR="10.0.0.3:50051" STORAGE_PATH="/var/lib/ngi-db/node3" ./db
```

## Implementation Status

✅ **Completed:**
- Storage layer with Sled backend
- Raft type configuration (DbTypeConfig with openraft macro)
- gRPC service definition (protobuf)
- gRPC server implementation (all 8 RPC methods)
- State machine for log application
- Network factory skeleton
- **RaftStorage trait implementation** (combines log storage and state machine)
- **RaftLogReader trait implementation**
- **RaftSnapshotBuilder trait implementation**
- **Cluster initialization** (single-node bootstrap)
- **Raft-gRPC integration** (server starts with Raft consensus)

🚧 **In Progress:**
- Full network layer (inter-node gRPC calls need real RPC implementation)
- Multi-node cluster deployment and testing

📋 **TODO:**
- Complete inter-node gRPC communication
- Leader election testing across multiple nodes
- Log replication verification
- Snapshot transfer between nodes
- Integration tests with 3+ node cluster
- Performance benchmarking

## Dependencies

- `tokio` 1.43 - Async runtime
- `tonic` 0.14 / `prost` 0.14 - gRPC framework
- `openraft` 0.9 - Raft consensus library
- `sled` 0.34 - Embedded database
- `bincode` 2 - Binary serialization
- `anyhow` / `thiserror` - Error handling
- `tracing` - Structured logging

## References

- [OpenRaft Documentation](https://docs.rs/openraft/latest/openraft/)
- [Sled Documentation](https://docs.rs/sled/latest/sled/)
- [Tonic gRPC](https://docs.rs/tonic/latest/tonic/)
- [Raft Paper](https://raft.github.io/raft.pdf)
