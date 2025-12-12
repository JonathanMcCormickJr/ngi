# Bincode - Binary Serialization for NGI

> A fast, compact binary encoding for Rust types, designed for efficiency and safety.

**Official Docs:** https://docs.rs/bincode/latest/bincode/

**Current Version:** 2.0.1+

## Overview

Bincode provides NGI's standard serialization format for:
- Sled database values
- gRPC message encoding (via prost)
- Inter-service message passing
- Audit logs and state snapshots

## Key Features

### Type-Safe Encoding
- No schema language needed (Rust types are schema)
- Compile-time checked
- Zero unsafe code in business logic

### Efficiency
- Compact binary format (not text-based like JSON)
- Variable-length integer encoding available
- Customizable endianness

### Configuration
```rust
use bincode::config::{standard, legacy};

// NGI default - efficient, cross-platform compatible
let config = bincode::config::standard()
    .with_big_endian()
    .with_variable_int_encoding();

// Or use Bincode 1.x compatibility
let config = bincode::config::legacy();
```

## NGI Serialization Patterns

### Basic Ticket Serialization
```rust
use serde::{Serialize, Deserialize};
use bincode::config::standard;

#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
    pub status: TicketStatus,
}

// Encode to bytes
let ticket = Ticket { /* ... */ };
let bytes = bincode::encode_to_vec(&ticket, standard())?;

// Persist to Sled
db.insert(b"ticket:1", bytes)?;

// Decode from storage
let bytes = db.get(b"ticket:1")?.unwrap();
let ticket: Ticket = bincode::decode_from_slice(&bytes, standard())?.0;
```

### Enum Encoding (Ticket Status)
```rust
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]  // Store as single byte
pub enum TicketStatus {
    Open = 0,
    Assigned = 1,
    Resolved = 2,
    Closed = 3,
}

// Encoded as 1 byte per status
```

### Efficient Storage
```rust
// With variable-length integers
let config = standard().with_variable_int_encoding();

// Small numbers use fewer bytes
let small_id: u64 = 42;
let bytes = bincode::encode_to_vec(&small_id, config)?;  // 1-2 bytes

// Large numbers use more bytes
let large_id: u64 = 18_446_744_073_709_551_615;
let bytes = bincode::encode_to_vec(&large_id, config)?;  // 8 bytes
```

## API Reference

### Core Functions

| Function | Purpose | Use Case |
|----------|---------|----------|
| `encode_to_vec(T, config)` | Encode to `Vec<u8>` | General serialization |
| `decode_from_slice(&[u8], config)` | Decode from bytes | Deserialization |
| `encode_into_slice(T, &mut [u8], config)` | Encode to fixed buffer | Pre-allocated buffers |
| `encode_into_std_write(T, writer, config)` | Encode to `std::io::Write` | File/network output |
| `decode_from_std_read(reader, config)` | Decode from reader | File/network input |

### Traits

- `Encode` - Implemented by serde types for encoding
- `Decode` - Implemented by serde types for decoding
- `BorrowDecode` - Efficient decoding with borrowed data

### Configuration Options

```rust
use bincode::config::*;

// Endianness
let config = standard()
    .with_little_endian()  // Default for x86/ARM
    .with_big_endian();    // Network byte order

// Integer encoding
let config = standard()
    .with_fixint()      // Fixed 8 bytes per u64
    .with_variable_int_encoding();  // 1-9 bytes per u64

// Size limits
let config = standard()
    .with_limit::<1_000_000>()   // Max 1MB decoded
    .with_no_limit();            // Unlimited (caution!)
```

## Serde Integration

All standard Serde types work with Bincode:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct ComplexType {
    id: u64,
    name: String,
    tags: Vec<String>,
    metadata: std::collections::HashMap<String, String>,
}

let bytes = bincode::encode_to_vec(&value, standard())?;
```

## Performance Characteristics

| Aspect | Value |
|--------|-------|
| Encoding speed | 100-200 MB/s (typical) |
| Decoding speed | 150-300 MB/s (typical) |
| Overhead | Minimal (structure of data only) |
| Size | 10-30% of JSON equivalent |

## NGI Recommended Configuration

```rust
// Define in shared crate for consistency
pub const BINCODE_CONFIG: bincode::config::Configuration<
    bincode::config::BigEndian,
    bincode::config::Varint,
> = bincode::config::standard()
    .with_big_endian()
    .with_variable_int_encoding()
    .with_limit::<{ 10 * 1024 * 1024 }>();  // 10MB safety limit

// Usage everywhere in NGI
bincode::encode_to_vec(&ticket, BINCODE_CONFIG)?;
```

## Error Handling

```rust
use bincode::{EncodeError, DecodeError};

match bincode::encode_to_vec(&value, standard()) {
    Ok(bytes) => persist(bytes),
    Err(EncodeError::InvalidIntBitPattern(_)) => handle_invalid(),
    Err(e) => eprintln!("Encoding failed: {}", e),
}

match bincode::decode_from_slice(&bytes, standard()) {
    Ok((value, _)) => process(value),
    Err(DecodeError::UnexpectedEnd) => handle_truncated(),
    Err(e) => eprintln!("Decoding failed: {}", e),
}
```

## Comparison: Bincode vs Alternatives

| Format | Size | Speed | Schema | Use in NGI |
|--------|------|-------|--------|-----------|
| Bincode | Smallest | Fastest | Compile-time | ✅ Default |
| JSON | Medium | Moderate | Runtime | Admin API (via serde) |
| Protocol Buffers | Small | Fast | .proto file | gRPC (via prost) |
| MessagePack | Small | Fast | Optional | Not used |

## Common Pitfalls & Solutions

### 1. Mixed Endianness Between Services
**Problem:** Server encodes big-endian, client decodes little-endian
```rust
// WRONG: Each service uses default
let bytes = bincode::encode_to_vec(&data)?;

// CORRECT: Use consistent config
let bytes = bincode::encode_to_vec(&data, BINCODE_CONFIG)?;
```

### 2. Size Limit Exceeded
**Problem:** Decode fails on large messages
```rust
// WRONG: No limit, potential OOM
bincode::decode_from_slice(&bytes, standard())?;

// CORRECT: Safe limit with fallback
match bincode::decode_from_slice(&bytes, 
    standard().with_limit::<{ 100 * 1024 * 1024 }>()) {
    Ok((value, _)) => process(value),
    Err(_) => return Err("Message too large"),
}
```

### 3. Forward Compatibility
**Problem:** Adding new fields breaks old serialized data
```rust
// WRONG: Required new field
#[derive(Serialize, Deserialize)]
struct Ticket {
    id: u64,
    priority: u8,  // Added field breaks compatibility
}

// CORRECT: Optional new field
#[derive(Serialize, Deserialize)]
struct Ticket {
    id: u64,
    #[serde(default)]
    priority: Option<u8>,
}
```

## Testing Patterns

```rust
#[test]
fn test_roundtrip_serialization() {
    let original = Ticket {
        id: 42,
        title: "Test ticket".into(),
        status: TicketStatus::Open,
    };
    
    let bytes = bincode::encode_to_vec(&original, standard()).unwrap();
    let (restored, _): (Ticket, _) = 
        bincode::decode_from_slice(&bytes, standard()).unwrap();
    
    assert_eq!(original, restored);
}

#[test]
fn test_deterministic_encoding() {
    // Same data must encode identically every time
    let data = vec![1, 2, 3, 4];
    let bytes1 = bincode::encode_to_vec(&data, standard()).unwrap();
    let bytes2 = bincode::encode_to_vec(&data, standard()).unwrap();
    assert_eq!(bytes1, bytes2);
}
```

## NGI Service Integration

| Service | Usage |
|---------|-------|
| DB | Sled value encoding, state snapshots |
| Custodian | Lock state serialization |
| Auth | Session token encoding (if cached locally) |
| Admin | User struct persistence |
| LBRP | Not typically used (REST/JSON via serde) |

## References

- **Official Modules:**
  - [config](https://docs.rs/bincode/latest/bincode/config/) - Encoding configuration
  - [encode/decode](https://docs.rs/bincode/latest/bincode/) - Core functions
  
- **NGI Integration:**
  - [shared/src/ticket.rs](../../../shared/src/ticket.rs) - Serializable types
  - [db/src/storage.rs](../../../db/src/storage.rs) - Storage patterns

---

**Last Updated:** December 2025  
**Documentation Version:** Bincode 2.0.1
