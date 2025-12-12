# Sled - Embedded ACID Database for NGI

> A high-performance embedded database with ACID transactions, written in Rust.

**Official Docs:** https://docs.rs/sled/latest/sled/

**Current Version:** 0.34.7+

## Overview

Sled is the persistent storage foundation for NGI's consensus and data layers. It provides ACID-compliant transactions with a key-value interface suitable for state machines and log storage.

## Key Features for NGI

### ACID Transactions
```rust
db.transaction(|txn| {
    txn.insert(b"ticket:42", ticket_data)?;
    txn.insert(b"lock:42", lock_data)?;
    Ok(())  // Atomic commit
})?;
```

### Multiple Trees (Distributed Indexes)
```rust
let tickets = db.open_tree("tickets")?;
let locks = db.open_tree("locks")?;
let index = db.open_tree("ticket:index:status:open")?;
```

### Persistent State Machine
- Survives process crashes
- No data loss with `flush()` operations
- Perfect for Raft log storage and ticket persistence

## Documentation Index

- **[architecture.md](architecture.md)** - Complete Sled implementation guide
  - ACID transactions
  - Storage schema patterns
  - NGI storage layer implementation
  - Best practices and performance tuning
  - Official API documentation

## NGI Core Patterns

### Table-Like Structure with Prefixes
```
ticket:{id} -> serialized Ticket
ticket:index:status:{status}:{id} -> (empty)
lock:{id} -> LockInfo
user:{id} -> User struct
```

### Atomic Batch Operations
```rust
let mut batch = sled::Batch::default();
batch.insert(b"ticket:1", ticket1);
batch.insert(b"lock:1", lock_info);
db.apply_batch(batch)?;  // All-or-nothing
```

### Secondary Indexing
```rust
// Create index entry during write
let status_key = format!("ticket:index:status:{}:{}", status, id);
tickets.insert(status_key.as_bytes(), &[])?;

// Query by index
for item in tickets.scan_prefix(b"ticket:index:status:0:") {
    let (key, _) = item?;
    // Extract ticket ID and fetch
}
```

## API Breakdown

### `Db` Struct (Database Connection)
- `open_tree(name)` - Open/create named tree
- `drop_tree(name)` - Drop tree (soft delete)
- `tree_names()` - List all trees
- `generate_id()` - Thread-safe ID generation
- `import/export` - Data migration
- `flush()` - Ensure durability
- `size_on_disk()` - Database file size

### `Tree` Struct (Named Key-Value Store)
- `transaction(closure)` - ACID multi-operation
- `apply_batch(batch)` - Atomic batch insert
- `set_merge_operator()` - Read-modify-write ops
- `scan_prefix(prefix)` - Prefix iteration
- `range(start..end)` - Range queries
- `first(), last()` - Endpoint queries
- `watch_prefix()` - Subscribe to changes
- `insert, get, remove, contains_key` - Basic ops

### `Transactional` Trait
- Enables transactions on Trees and tuples of Trees
- `transaction(|txn| { ... })` syntax
- Automatic rollback on error
- Explicit abort with `Err()` returns

## Configuration

### Basic Setup
```rust
let db = sled::open("./data")?;
```

### Advanced Configuration
```rust
let db = sled::Config::default()
    .path("./data")
    .cache_capacity(1_000_000_000)  // 1GB cache
    .flush_every_ms(Some(500))       // Flush frequency
    .open()?;
```

### Performance Tuning for NGI
- **Large datasets:** Increase `cache_capacity`
- **High write volume:** Adjust `flush_every_ms`
- **High concurrency:** Sled handles 1000s of concurrent readers/writers safely

## Error Handling

```rust
use sled::{Db, Tree};

match db.open_tree("tickets") {
    Ok(tree) => process(tree),
    Err(e) => eprintln!("Failed to open tree: {}", e),
}

match db.apply_batch(batch) {
    Ok(_) => println!("Committed"),
    Err(e) => eprintln!("Batch failed: {}", e),
}
```

## Transaction Semantics

### Conflict Detection
```rust
let result = db.transaction(|txn| {
    let current = txn.get(b"counter")?;
    // If another transaction modified "counter", this returns Err
    // and transaction is retried automatically
    let next = update_counter(&current)?;
    txn.insert(b"counter", next)?;
    Ok(())
})?;
```

### Multi-Tree Transactions
```rust
(tickets_tree, locks_tree).transaction(|(t_txn, l_txn)| {
    t_txn.insert(ticket_id, ticket_data)?;
    l_txn.insert(ticket_id, lock_info)?;
    Ok(())
})?;
```

## Serialization Integration

### With Bincode (NGI Standard)
```rust
let ticket = Ticket { id: 1, status: Open, ... };
let encoded = bincode::serialize(&ticket)?;
tree.insert(b"ticket:1", encoded)?;

// Retrieve
let encoded = tree.get(b"ticket:1")?;
let ticket: Ticket = bincode::deserialize(&encoded)?;
```

## Durability Guarantees

| Operation | Durability |
|-----------|-----------|
| `insert()` | In memory until flush |
| `flush()` | Persisted to disk |
| `transaction().await` | Atomic, persisted after successful return |
| OS crash | Data up to last flush preserved |

## Testing Patterns

```rust
#[test]
fn test_transaction_rollback() {
    let db = sled::Config::default().temporary(true).open().unwrap();
    
    let result = db.transaction(|txn| {
        txn.insert(b"key", b"value")?;
        Err(sled::transaction::ConflictableTransactionError::Abort(()))
    });
    
    assert!(result.is_err());
    assert!(db.get(b"key").unwrap().is_none());
}
```

## Common NGI Use Cases

| Use Case | Sled Pattern |
|----------|--------------|
| Raft Log Storage | Multiple trees by term + index, transaction for atomic appends |
| Ticket Database | `ticket:{id}` main + `ticket:index:status:{s}:{id}` for queries |
| Lock State | `lock:{id}` with transaction for atomic acquire/release |
| User Cache | `user:{id}` with watch_prefix for invalidation |

## Limitations & Workarounds

1. **No distributed consensus** - NGI adds Raft on top
2. **Single machine** - Design for local persistence; cluster coordination via gRPC
3. **No SQL** - Use prefix indexing and range queries instead

## References

- **Official Modules:**
  - [Db](https://docs.rs/sled/latest/sled/struct.Db.html) - Database connection
  - [Tree](https://docs.rs/sled/latest/sled/struct.Tree.html) - Key-value store
  - [transaction](https://docs.rs/sled/latest/sled/transaction/) - ACID transactions

- **NGI Documentation:**
  - [db service](../../../db/) - NGI's database microservice
  - [ARCHITECTURE.md](../../../ARCHITECTURE.md) - Storage layer design

---

**Last Updated:** December 2025  
**Documentation Version:** Sled 0.34.7
