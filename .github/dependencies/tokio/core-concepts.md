# Tokio 1.43

**Repository:** https://github.com/tokio-rs/tokio  
**Documentation:** https://docs.rs/tokio/  
**Crates.io:** https://crates.io/crates/tokio

## Version in NGI
```toml
tokio = { version = "1.43", features = ["full"] }
```

## Overview
Tokio is an asynchronous runtime for Rust built with Rust. It provides the foundations needed to build I/O-bound asynchronous applications. It's used for task scheduling, multi-threaded execution, and synchronization primitives.

## Key Features Used in NGI

### Multi-threaded Async Runtime
```rust
#[tokio::main]
async fn main() {
    // Automatically uses multi-threaded runtime
}
```

### Task Spawning
```rust
tokio::spawn(async {
    // Task runs concurrently
    do_something().await;
});
```

### Synchronization Primitives
- `tokio::sync::RwLock` - Reader-writer lock for shared state
- `tokio::sync::Mutex` - Mutual exclusion lock
- `tokio::sync::Semaphore` - Counting semaphore
- `tokio::sync::Barrier` - Synchronization barrier

### Channels for Inter-Task Communication
```rust
// Multi-producer, single-consumer channel
let (tx, mut rx) = tokio::sync::mpsc::channel(100);

// Broadcast channel (one-to-many)
let (tx, rx) = tokio::sync::broadcast::channel(100);

// Watch channel (always get latest value)
let (tx, rx) = tokio::sync::watch::channel(initial_value);
```

### Utilities
- `tokio::time::sleep()` - Async sleep
- `tokio::time::timeout()` - Timeout wrapper
- `tokio::time::interval()` - Recurring timer
- `tokio::join!()` - Join multiple futures
- `tokio::select!()` - Racing multiple futures

## NGI Usage Patterns

### Task Coordination
Used in DB service for Raft log replication and state machine application.

### Channel-based Message Passing
Admin service uses mpsc channels to communicate status updates to monitoring subsystem.

### Synchronization
Custodian service uses RwLock for concurrent access to ticket lock state.

## Best Practices
1. Use `#[tokio::main]` for application entry point
2. Prefer async/await over manual Future handling
3. Use channels for inter-task communication, not shared memory
4. Avoid blocking operations inside async code (use `task::block_in_place` sparingly)
5. Always timeout operations that could hang

## Common Issues
- **Deadlocks:** Tokio locks are not reentrant; avoid holding multiple locks
- **Blocking:** CPU-bound work should be spawned on separate thread pool
- **Task Cancellation:** Tokio tasks are cancelled when dropped; ensure resources are cleaned up

---

## Official API Documentation

### Modules

- **[tokio::task](https://docs.rs/tokio/latest/tokio/task/index.html)** - Asynchronous green-threads
  - [spawn](https://docs.rs/tokio/latest/tokio/task/fn.spawn.html) - Spawn a task
  - [JoinHandle](https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html) - Await the output of spawned task
  - [spawn_blocking](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html) - Run blocking operations

- **[tokio::sync](https://docs.rs/tokio/latest/tokio/sync/index.html)** - Synchronization primitives
  - [mpsc](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html) - Multi-producer, single-consumer channel
  - [broadcast](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html) - Broadcast channel
  - [watch](https://docs.rs/tokio/latest/tokio/sync/watch/index.html) - Watch channel
  - [oneshot](https://docs.rs/tokio/latest/tokio/sync/oneshot/index.html) - One-time channel
  - [Mutex](https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html) - Async mutex
  - [RwLock](https://docs.rs/tokio/latest/tokio/sync/struct.RwLock.html) - Reader-writer lock
  - [Barrier](https://docs.rs/tokio/latest/tokio/sync/struct.Barrier.html) - Synchronization barrier
  - [Semaphore](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html) - Counting semaphore

- **[tokio::time](https://docs.rs/tokio/latest/tokio/time/index.html)** - Time utilities
  - [sleep](https://docs.rs/tokio/latest/tokio/time/fn.sleep.html) - Async sleep
  - [timeout](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html) - Timeout wrapper
  - [interval](https://docs.rs/tokio/latest/tokio/time/fn.interval.html) - Repeating operation

- **[tokio::net](https://docs.rs/tokio/latest/tokio/net/index.html)** - Networking
  - [TcpListener](https://docs.rs/tokio/latest/tokio/net/struct.TcpListener.html)
  - [TcpStream](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html)
  - [UdpSocket](https://docs.rs/tokio/latest/tokio/net/struct.UdpSocket.html)

- **[tokio::runtime](https://docs.rs/tokio/latest/tokio/runtime/index.html)** - Runtime configuration
  - [Runtime](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html)
  - [Builder](https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html)

### Macros

- **[#[tokio::main]](https://docs.rs/tokio/latest/tokio/attr.main.html)** - Mark async function to run on Tokio runtime
- **[#[tokio::test]](https://docs.rs/tokio/latest/tokio/attr.test.html)** - Mark test to run on Tokio runtime
- **[tokio::join!](https://docs.rs/tokio/latest/tokio/macro.join.html)** - Join multiple concurrent branches
- **[tokio::select!](https://docs.rs/tokio/latest/tokio/macro.select.html)** - Race multiple concurrent branches
