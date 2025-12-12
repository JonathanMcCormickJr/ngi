# Sled 0.34

**Repository:** https://github.com/spacejam/sled  
**Documentation:** https://docs.rs/sled/  
**Crates.io:** https://crates.io/crates/sled

## Version in NGI
```toml
sled = "0.34"
```

## Overview
Sled is an embedded key-value database written in Rust with ACID transactions, lock-free reads, and high concurrent write throughput. It's the primary persistent storage backend for NGI services.

## Key Features Used in NGI

### ACID Transactions
```rust
let result = db.transaction(|txn| {
    txn.insert(b"key1", b"value1")?;
    txn.insert(b"key2", b"value2")?;
    Ok(())
})?;
```

### Key-Value Operations
```rust
// Insert/update
db.insert(b"key", b"value")?;

// Get
if let Some(val) = db.get(b"key")? {
    println!("value: {:?}", val);
}

// Delete
db.remove(b"key")?;
```

### Prefix Scanning (Secondary Indexes)
```rust
for result in db.scan_prefix(b"prefix:") {
    let (key, val) = result?;
    // Process matching keys
}
```

### Atomicity
Operations are fully ACID compliant - either all succeed or all fail.

## NGI Storage Schema

NGI uses prefixed keys to simulate table structures:

```
// Tickets
ticket:{ticket_id} -> Ticket struct (bincode)
ticket:index:status:{status}:{ticket_id} -> empty value
ticket:index:assigned:{user_id}:{ticket_id} -> empty value
ticket:deleted -> set of deleted ticket IDs

// Users
user:{user_id} -> User struct
user:index:email:{email} -> user_id

// Locks
lock:{ticket_id} -> LockInfo struct
lock:expiry:{timestamp}:{ticket_id} -> empty value

// Audit logs
audit:{ticket_id}:{timestamp} -> AuditEntry struct
```

## Implementation Pattern

```rust
use sled::{Db, Tree};

pub struct StorageLayer {
    db: Db,
    tickets: Tree,
    users: Tree,
}

impl StorageLayer {
    pub fn new(path: &str) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self {
            tickets: db.open_tree("tickets")?,
            users: db.open_tree("users")?,
            db,
        })
    }

    pub fn insert_ticket(&self, ticket: &Ticket) -> Result<()> {
        let key = format!("ticket:{}", ticket.id);
        let value = bincode::serialize(ticket)?;
        self.tickets.insert(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn tickets_by_status(&self, status: TicketStatus) -> Result<Vec<Ticket>> {
        let prefix = format!("ticket:index:status:{}:", status as u8);
        let mut tickets = Vec::new();

        for result in self.tickets.scan_prefix(prefix.as_bytes()) {
            let (key, _) = result?;
            let key_str = String::from_utf8(key.to_vec())?;
            let ticket_id: u64 = key_str.split(':').nth(3).unwrap().parse()?;

            if let Some(value) = self.tickets.get(format!("ticket:{}", ticket_id).as_bytes())? {
                let ticket = bincode::deserialize(&value)?;
                if !ticket.deleted {
                    tickets.push(ticket);
                }
            }
        }

        Ok(tickets)
    }
}
```

## Best Practices
1. Use transactions for multi-key operations
2. Create secondary indexes with prefix keys for efficient queries
3. Soft delete only - mark deleted flag rather than removing data
4. Use separate trees for different data types to improve locality
5. Serialize complex types with bincode for compact storage

## Performance Considerations
- Lock-free reads are very fast
- Writes are serialized through a log
- Prefix scans are efficient for range queries
- Configuration can be tuned in `sled::Config`

## Data Durability
- All writes are flushed to disk immediately by default
- `flush()` can be called explicitly to ensure durability
- Database is crash-safe

---

## Official API Documentation

### Main Structs

- **[Db](https://docs.rs/sled/latest/sled/struct.Db.html)** - Top-level database handle
  - Implements `Deref<Target = Tree>` to refer to default keyspace
  - Methods:
    - [insert](https://docs.rs/sled/latest/sled/struct.Db.html#method.insert) - Insert key-value pair
    - [get](https://docs.rs/sled/latest/sled/struct.Db.html#method.get) - Get value by key
    - [remove](https://docs.rs/sled/latest/sled/struct.Db.html#method.remove) - Remove key-value pair
    - [open_tree](https://docs.rs/sled/latest/sled/struct.Db.html#method.open_tree) - Open separate tree
    - [flush](https://docs.rs/sled/latest/sled/struct.Db.html#method.flush) - Flush to disk
    - [transaction](https://docs.rs/sled/latest/sled/struct.Db.html#method.transaction) - Atomic multi-key transaction
    - [range](https://docs.rs/sled/latest/sled/struct.Db.html#method.range) - Range iteration

- **[Tree](https://docs.rs/sled/latest/sled/struct.Tree.html)** - Isolated keyspace
  - Methods:
    - [insert](https://docs.rs/sled/latest/sled/struct.Tree.html#method.insert)
    - [get](https://docs.rs/sled/latest/sled/struct.Tree.html#method.get)
    - [remove](https://docs.rs/sled/latest/sled/struct.Tree.html#method.remove)
    - [scan_prefix](https://docs.rs/sled/latest/sled/struct.Tree.html#method.scan_prefix) - Prefix scanning for secondary indexes
    - [compare_and_swap](https://docs.rs/sled/latest/sled/struct.Tree.html#method.compare_and_swap) - Atomic CAS
    - [watch_prefix](https://docs.rs/sled/latest/sled/struct.Tree.html#method.watch_prefix) - Subscribe to updates
    - [merge](https://docs.rs/sled/latest/sled/struct.Tree.html#method.merge) - Merge operator
    - [batch](https://docs.rs/sled/latest/sled/struct.Tree.html#method.apply_batch) - Apply batch updates

- **[IVec](https://docs.rs/sled/latest/sled/struct.IVec.html)** - Inline or remote buffer
  - Can be inline for small values or remote (Arc-backed) for large values

- **[Iter](https://docs.rs/sled/latest/sled/struct.Iter.html)** - Iterator over keys and values

- **[Config](https://docs.rs/sled/latest/sled/struct.Config.html)** - Database configuration
  - Methods:
    - [path](https://docs.rs/sled/latest/sled/struct.Config.html#method.path) - Set database path
    - [cache_capacity](https://docs.rs/sled/latest/sled/struct.Config.html#method.cache_capacity) - Set cache size
    - [open](https://docs.rs/sled/latest/sled/struct.Config.html#method.open) - Open database

### Transactions

- **[transaction::Transactional](https://docs.rs/sled/latest/sled/transaction/trait.Transactional.html)** - Multi-tree transaction trait

### Error Types

- **[Error](https://docs.rs/sled/latest/sled/enum.Error.html)** - Database error type
- **[Result](https://docs.rs/sled/latest/sled/type.Result.html)** - Result type for database operations

### Functions

- **[open](https://docs.rs/sled/latest/sled/fn.open.html)** - Open database with default config
  - Takes path as argument
  - Creates directory if it doesn't exist
