# Serde - Serialization Framework for NGI

> A generic serialization/deserialization framework for Rust data structures.

**Official Docs:** https://docs.rs/serde/latest/serde/

**Current Version:** 1.0.200+

## Overview

Serde is NGI's universal serialization abstraction layer. It enables seamless conversion between Rust types and various formats (Bincode, JSON, Protocol Buffers) without format-specific code.

## Core Concept

Serde provides two generic traits:
- `Serialize` - Convert Rust value → any format
- `Deserialize` - Convert any format → Rust value

Data formats (Bincode, serde_json, etc.) implement Serializer/Deserializer traits. User types only derive Serialize/Deserialize once and work with all formats.

## Basic Usage

```rust
use serde::{Serialize, Deserialize};

// Declare once, use anywhere
#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
    pub status: TicketStatus,
}

// Serialize with Bincode
let bytes = bincode::encode_to_vec(&ticket, standard())?;

// Serialize with JSON (if using serde_json)
let json = serde_json::to_string(&ticket)?;

// Deserialize either way
let ticket: Ticket = bincode::decode_from_slice(&bytes, standard())?.0;
let ticket: Ticket = serde_json::from_str(&json)?;
```

## NGI Serialization Stack

```
Rust Type (with #[derive(Serialize, Deserialize)])
        ↓
    Serde Trait
        ↓
    Format Implementation
        ├─ Bincode → bytes (Sled storage)
        ├─ JSON → text (REST API via axum)
        ├─ Protocol Buffers → bytes (gRPC via tonic)
        └─ MessagePack (if needed)
```

## Derive Macros

### Serialize Derive
```rust
#[derive(Serialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
}

// Generates impl Serialize { fn serialize(...) }
```

### Deserialize Derive
```rust
#[derive(Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
}

// Generates impl Deserialize { fn deserialize(...) }
```

### Combined
```rust
#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
}
```

## Attributes for Fine-Grained Control

### Field-Level
```rust
#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    
    // Rename field in serialized format
    #[serde(rename = "ticket_title")]
    pub title: String,
    
    // Skip this field
    #[serde(skip)]
    pub internal_state: Arc<RwLock<State>>,
    
    // Use default if missing during deserialization
    #[serde(default)]
    pub tags: Vec<String>,
    
    // Use custom serialization function
    #[serde(serialize_with = "serialize_status")]
    #[serde(deserialize_with = "deserialize_status")]
    pub status: TicketStatus,
}

fn serialize_status<S>(status: &TicketStatus, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u8(*status as u8)
}

fn deserialize_status<'de, D>(deserializer: D) -> Result<TicketStatus, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let byte = u8::deserialize(deserializer)?;
    TicketStatus::try_from(byte).map_err(serde::de::Error::custom)
}
```

### Container-Level
```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]  // all fields lowercase
#[serde(deny_unknown_fields)]       // error on extra fields
pub struct Config {
    pub host: String,
    pub port: u16,
}
```

## NGI Common Patterns

### Enums with Numeric Representation
```rust
#[derive(Serialize, Deserialize, Copy, Clone)]
#[repr(u8)]  // Store as byte
pub enum TicketStatus {
    Open = 0,
    Assigned = 1,
    Resolved = 2,
    Closed = 3,
}

// Or more explicitly with serde:
#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(u8)]  // Serialize as u8
pub enum TicketStatus {
    Open = 0,
    Assigned = 1,
    Resolved = 2,
    Closed = 3,
}
```

### Optional Fields with Defaults
```rust
#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
    
    // Present in JSON API responses, optional in deserialization
    #[serde(default)]
    pub assigned_to: Option<u64>,
}

// If JSON is {"id": 1, "title": "Ticket"}, assigned_to defaults to None
```

### Newtype Pattern for Type Safety
```rust
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TicketId(u64);

// These serialize/deserialize differently by type, preventing confusion
let user_id = UserId(1);
let ticket_id = TicketId(1);

// Won't compile: can't mix UserId and TicketId
// fn foo(id: UserId) { }
// foo(ticket_id);  // ERROR
```

## Trait Reference

### Serialize Trait
```rust
pub trait Serialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}
```

Used for:
- Implementing custom serialization
- Advanced control over format

### Deserialize Trait
```rust
pub trait Deserialize<'de>: Sized {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}
```

Used for:
- Implementing custom deserialization
- Custom type builders

### Serializer Trait
Implemented by format backends (Bincode, JSON, etc.):
```rust
pub trait Serializer: Sized {
    type Ok;
    type Error;
    
    fn serialize_unit_variant(...) -> Result<...>;
    fn serialize_newtype_struct(...) -> Result<...>;
    // ... other serialization methods
}
```

### Deserializer Trait
Implemented by format backends:
```rust
pub trait Deserializer<'de>: Sized {
    type Error;
    
    fn deserialize_unit_variant(...) -> Result<...>;
    fn deserialize_newtype_struct(...) -> Result<...>;
    // ... other deserialization methods
}
```

## Format Integration in NGI

### Bincode (Storage)
```rust
let ticket = Ticket { /* ... */ };

// Serialize to Sled
let bytes = bincode::encode_to_vec(&ticket, standard())?;
db.insert(b"ticket:1", bytes)?;

// Deserialize from Sled
let bytes = db.get(b"ticket:1")?.unwrap();
let ticket: Ticket = bincode::decode_from_slice(&bytes, standard())?.0;
```

### JSON (REST API via Axum)
```rust
use axum::Json;

#[derive(Serialize, Deserialize)]
pub struct TicketResponse {
    pub id: u64,
    pub title: String,
    pub status: String,
}

async fn get_ticket(id: u64) -> Json<TicketResponse> {
    // Automatically serializes struct to JSON
    Json(TicketResponse { /* ... */ })
}
```

### Protocol Buffers (gRPC via tonic/prost)
```rust
// prost handles proto3 message serialization
// Serde not used directly; bincode wraps proto bytes for storage
```

## Error Handling

```rust
use serde::de::Error;

#[derive(Deserialize)]
pub struct Ticket {
    #[serde(deserialize_with = "validate_id")]
    pub id: u64,
}

fn validate_id<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let id = u64::deserialize(deserializer)?;
    if id == 0 {
        Err(D::Error::custom("ID cannot be zero"))
    } else {
        Ok(id)
    }
}
```

## Testing Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_ticket() {
        let ticket = Ticket { id: 42, title: "Test".into(), status: Open };
        let json = serde_json::to_string(&ticket).unwrap();
        assert!(json.contains("\"id\":42"));
    }
    
    #[test]
    fn test_deserialize_ticket() {
        let json = r#"{"id":42,"title":"Test","status":"Open"}"#;
        let ticket: Ticket = serde_json::from_str(json).unwrap();
        assert_eq!(ticket.id, 42);
    }
}
```

## References

- **Official Modules:**
  - [ser](https://docs.rs/serde/latest/serde/ser/) - Serialization traits
  - [de](https://docs.rs/serde/latest/serde/de/) - Deserialization traits

- **NGI Types:**
  - [shared/src/ticket.rs](../../../shared/src/ticket.rs) - Serializable ticket types
  - [shared/src/user.rs](../../../shared/src/user.rs) - Serializable user types

---

**Last Updated:** December 2025  
**Documentation Version:** Serde 1.0.200
