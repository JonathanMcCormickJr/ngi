# Tokio - Async Runtime for NGI

> An event-driven, non-blocking I/O platform for writing asynchronous applications in Rust.

**Official Docs:** https://docs.rs/tokio/latest/tokio/

**Current Version:** 1.48.0+

## Overview

Tokio is the core async runtime for NGI, providing all the essential infrastructure for concurrent task execution, I/O operations, and timing utilities.

## Documentation Index

- **[core-concepts.md](core-concepts.md)** - Complete Tokio implementation guide
  - Multi-threaded async runtime
  - Task spawning and management
  - Synchronization primitives (RwLock, Mutex, channels)
  - Channels for inter-task communication
  - Time utilities (sleep, timeout, interval)
  - NGI integration patterns
  - Best practices and common issues
  - Official API documentation

## Core Components for NGI

### Task Management (`tokio::task`)
- **Purpose:** Spawning and managing lightweight async tasks
- **Usage in NGI:** All microservices use `tokio::spawn()` for concurrent operations
- **Key APIs:**
  - `spawn()` - Create new async task
  - `JoinHandle` - Await task completion
  - `spawn_blocking()` - Run blocking code safely
  - `yield_now()` - Cooperative scheduling

### Synchronization (`tokio::sync`)
- **Purpose:** Async-safe communication between tasks
- **Channels:** oneshot, mpsc, broadcast, watch
- **Primitives:** Mutex, RwLock, Barrier, Semaphore, Notify
- **NGI Usage:** Inter-service message passing and state coordination

### Runtime (`tokio::runtime`)
- **Purpose:** Execution engine for async code
- **Schedulers:**
  - Multi-threaded (work-stealing, default)
  - Current-threaded (single thread)
- **Configuration:** `Builder` for custom runtime setup

### I/O Operations (`tokio::net`, `tokio::fs`, `tokio::io`)
- **TCP:** `TcpListener`, `TcpStream` for network services
- **UDP:** `UdpSocket` for datagram protocols
- **Async I/O Traits:** `AsyncRead`, `AsyncWrite`, `AsyncBufRead`

### Timing (`tokio::time`)
- **Utilities:** `sleep()`, `timeout()`, `interval()`
- **Types:** `Sleep`, `Timeout`, `Interval`
- **NGI Usage:** Request timeouts, periodic cleanup tasks

## Feature Flags for NGI

```toml
tokio = { version = "1", features = [
    "full",          # Enables all features (easiest for apps)
    # OR be explicit:
    "rt-multi-thread",  # Multi-threaded scheduler
    "macros",        # #[tokio::main], #[tokio::test]
    "net",           # TCP/UDP/Unix sockets
    "sync",          # Channels and locks
    "time",          # Timers and intervals
    "io-util",       # AsyncRead/Write utilities
    "fs",            # Async file operations
    "signal",        # OS signal handling
] }
```

## Common NGI Patterns

### Task Spawning
```rust
tokio::spawn(async {
    // Work runs concurrently
    perform_work().await;
});
```

### Communication with Channels
```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(100);

// Send from one task
tx.send(message).await?;

// Receive in another
while let Some(msg) = rx.recv().await {
    process(msg);
}
```

### Timeouts
```rust
match tokio::time::timeout(Duration::from_secs(5), operation()).await {
    Ok(result) => handle(result),
    Err(_) => handle_timeout(),
}
```

## Key Modules by NGI Service

| Service | Primary Tokio Modules |
|---------|----------------------|
| DB | `task`, `sync`, `net` (gRPC) |
| Custodian | `task`, `sync::Mutex`, `time::timeout` |
| Auth | `task`, `net::TcpListener` |
| Admin | `task`, `sync` |
| LBRP | `net`, `io`, `time` |

## Performance Considerations

1. **Work-Stealing:** Multi-threaded runtime distributes tasks across CPU cores
2. **Cooperative Scheduling:** Tasks yield at `.await` points (no preemption)
3. **Blocking Operations:** Use `spawn_blocking()` to avoid blocking runtime threads
4. **Memory:** Each task is ~64 bytes; NGI scales to hundreds of thousands of concurrent connections

## Testing with Tokio

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = my_async_function().await;
    assert_eq!(result, expected);
}
```

For tests with custom runtime:
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_with_multiple_threads() { }
```

## References

- **Module Documentation:**
  - [task](https://docs.rs/tokio/latest/tokio/task/) - Task spawning and management
  - [sync](https://docs.rs/tokio/latest/tokio/sync/) - Synchronization primitives
  - [runtime](https://docs.rs/tokio/latest/tokio/runtime/) - Runtime configuration
  - [net](https://docs.rs/tokio/latest/tokio/net/) - Networking (TCP/UDP/Unix)
  - [time](https://docs.rs/tokio/latest/tokio/time/) - Timing and delays

- **NGI Documentation:**
  - [ARCHITECTURE.md](../../../ARCHITECTURE.md) - System design with Tokio usage
  - [copilot-instructions.md](../copilot-instructions.md) - NGI coding standards

## Migration Notes

NGI uses Tokio 1.x stable. No breaking changes expected between patch versions.

---

**Last Updated:** December 2025  
**Documentation Version:** Tokio 1.48.0
