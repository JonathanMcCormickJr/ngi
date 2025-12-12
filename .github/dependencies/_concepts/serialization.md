# Serialization: serde & bincode

**serde:** https://docs.rs/serde/  
**bincode:** https://docs.rs/bincode/  

## Versions in NGI
```toml
serde = { version = "1.0", features = ["derive"] }
bincode = "2"
```

## Overview

NGI uses `serde` for serialization/deserialization and `bincode` for efficient binary encoding:
- **serde:** Serialization framework with derive macros
- **bincode:** Compact binary format for Sled storage and gRPC payloads

## serde Basics

### Derive Macro

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
    pub status: TicketStatus,
    pub assigned_to: Option<UserId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketStatus {
    Open,
    AwaitingCustomer,
    AwaitingISP,
    Closed,
    AutoClosed,
}
```

### Custom Serialization

```rust
use serde::{Serialize, Serializer, Deserialize, Deserializer};

#[derive(Serialize, Deserialize)]
pub struct CustomType {
    #[serde(serialize_with = "serialize_custom")]
    #[serde(deserialize_with = "deserialize_custom")]
    pub data: SomeType,
}

fn serialize_custom<S>(data: &SomeType, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Custom serialization logic
    serializer.serialize_str(&format!("{:?}", data))
}

fn deserialize_custom<'de, D>(deserializer: D) -> Result<SomeType, D::Error>
where
    D: Deserializer<'de>,
{
    // Custom deserialization logic
    let s = String::deserialize(deserializer)?;
    Ok(SomeType::from_str(&s).map_err(serde::de::Error::custom)?)
}
```

### Skipping Fields

```rust
#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    
    // Skip field during serialization
    #[serde(skip_serializing)]
    pub internal_cache: HashMap<String, String>,
    
    // Skip entire field (both directions)
    #[serde(skip)]
    pub temporary_data: Option<String>,
    
    // Use default if missing during deserialization
    #[serde(default)]
    pub optional_field: String,
}
```

## bincode Usage

### Serializing to Bytes

```rust
use bincode;

let ticket = Ticket { /* ... */ };

// Serialize to Vec<u8>
let bytes = bincode::serialize(&ticket)?;

// Store in Sled
db.insert(b"ticket:42", bytes)?;
```

### Deserializing from Bytes

```rust
// Retrieve from Sled
if let Some(bytes) = db.get(b"ticket:42")? {
    let ticket: Ticket = bincode::deserialize(&bytes)?;
    println!("Ticket: {:?}", ticket);
}
```

### Configuration

```rust
use bincode::config;

// Compact encoding (default)
let compact = bincode::serialize(&ticket)?;

// With custom config
let config = bincode::config::standard();
let bytes = bincode::encode_to_vec(&ticket, config)?;
```

## Common Patterns in NGI

### Sled Storage

```rust
// Store with bincode
pub fn store_ticket(&self, ticket: &Ticket) -> Result<()> {
    let key = format!("ticket:{}", ticket.id);
    let value = bincode::serialize(ticket)?;
    self.db.insert(key.as_bytes(), value)?;
    Ok(())
}

// Retrieve with bincode
pub fn get_ticket(&self, id: u64) -> Result<Option<Ticket>> {
    let key = format!("ticket:{}", id);
    match self.db.get(key.as_bytes())? {
        Some(bytes) => {
            let ticket = bincode::deserialize(&bytes)?;
            Ok(Some(ticket))
        }
        None => Ok(None),
    }
}
```

### gRPC Messages

Protocol Buffer messages automatically derive `Serialize`:

```protobuf
message Ticket {
    uint64 id = 1;
    string title = 2;
    TicketStatus status = 3;
}
```

Generated Rust code includes `impl Serialize, Deserialize` and can be:
- Sent over gRPC (built-in)
- Stored in Sled (via bincode)
- Logged (via serde_json)

## JSON Serialization (Logging Only)

```toml
serde_json = "1.0"
```

For debug logging:

```rust
use serde_json;

let json = serde_json::to_string_pretty(&ticket)?;
tracing::debug!("ticket: {}", json);
```

## Best Practices

### ✓ Good Patterns

**Always derive Serialize/Deserialize:**
```rust
#[derive(Serialize, Deserialize)]
pub struct MyType {
    // ...
}
```

**Use type-safe newtype wrappers:**
```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct UserId(u64);

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct TicketId(u64);
```

**Handle deserialization errors:**
```rust
let ticket: Ticket = bincode::deserialize(&bytes)
    .context("failed to deserialize ticket")?;
```

### ✗ Anti-Patterns

**Never store without type safety:**
```rust
// BAD: Storing untyped data
db.insert(b"data", raw_bytes)?;

// GOOD: Store strongly typed values
#[derive(Serialize, Deserialize)]
pub struct Data { /* ... */ }
db.insert(b"data", bincode::serialize(&data)?)?;
```

**Don't lose error context:**
```rust
// BAD
bincode::deserialize(&bytes).ok()?

// GOOD
bincode::deserialize(&bytes)
    .context("failed to deserialize ticket")?
```

## Compression (if needed)

For large payloads, consider compression:

```toml
flate2 = "1.0"  # Gzip compression
```

```rust
use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

let serialized = bincode::serialize(&ticket)?;
let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
encoder.write_all(&serialized)?;
let compressed = encoder.finish()?;
```

## Size Optimization

For efficient storage in Sled:

```rust
// Use compact types
#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,           // 8 bytes
    pub status: Status,    // 1 byte (enum as u8)
    pub assigned_to: Option<UserId>,  // 1 + 8 bytes
}

// Rather than
pub assigned_to: Option<String>,  // 1 + len bytes (less efficient)
```

---

## Official API Documentation

### serde

- **[Serialize](https://docs.rs/serde/latest/serde/trait.Serialize.html)** - Trait for serializable types
  - Use `#[derive(Serialize)]` for custom types

- **[Deserialize](https://docs.rs/serde/latest/serde/trait.Deserialize.html)** - Trait for deserializable types
  - Use `#[derive(Deserialize)]` for custom types

- **[Serializer](https://docs.rs/serde/latest/serde/trait.Serializer.html)** - Data format serializer
  - Implement for custom formats

- **[Deserializer](https://docs.rs/serde/latest/serde/trait.Deserializer.html)** - Data format deserializer
  - Implement for custom formats

### bincode

- **[encode_to_vec](https://docs.rs/bincode/latest/bincode/fn.encode_to_vec.html)** - Encode to Vec<u8>
  ```rust
  let bytes = bincode::encode_to_vec(&data, config::standard())?;
  ```

- **[decode_from_slice](https://docs.rs/bincode/latest/bincode/fn.decode_from_slice.html)** - Decode from bytes
  ```rust
  let (value, len): (T, usize) = bincode::decode_from_slice(&bytes, config::standard())?;
  ```

---

**See Also:**
- [serde Crate](../serde/README.md)
- [bincode Crate](../bincode/README.md)
