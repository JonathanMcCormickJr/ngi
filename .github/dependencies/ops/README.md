# OPS - Unikernel Build & Deployment Tool

> A compilation and orchestration tool for building and deploying Nanos unikernels to any cloud provider.

**Official Docs:** https://docs.ops.city/ops/

**Current Version:** 0.1.27+

## Overview

OPS is the tooling that enables NGI to (optionally) compile and deploy services as unikernels instead of traditional containers. A unikernel is a specialized single-process operating system that dramatically reduces attack surface, resource footprint, and operational complexity.

## Unikernel Architecture

### Traditional Container vs Unikernel
```
Container (Linux-based):
  ├─ Application
  ├─ Language Runtime
  ├─ System Libraries (libc, libssl, etc.)
  └─ Linux Kernel (millions of lines of code)
  
Unikernel (Nanos):
  ├─ Application
  └─ Nanos Kernel (optimized for single process)
  
Result:
- 10-100x smaller image size
- Faster boot (72-195ms)
- Reduced attack surface
- Better security isolation
```

### Benefits for NGI

1. **Security:** Single-process model eliminates process privilege separation vulnerabilities
2. **Performance:** Direct kernel pairing with application, optimized scheduling
3. **Cost:** Tiny memory footprint (Go: 37-40MB, C: 24-26MB)
4. **Boot Speed:** 72-195ms from boot to application ready
5. **Simplicity:** No SSH, shells, or orchestration needed

## NGI Integration

### When to Use Unikernels

**Good fit for NGI services:**
- Stateless services (Auth, Admin, LBRP)
- I/O-bound services (DB, Custodian with careful design)
- Services with fixed dependencies

**Not ideal:**
- Services requiring dynamic libraries
- Services needing shell access during runtime
- Development/debugging scenarios

### Deployment Models

```
Option 1: Traditional Containers (Current)
Client → Cloud Load Balancer → Kubernetes/Docker
         ↓
    Multiple OS instances

Option 2: OPS Unikernels (Alternative)
Client → Cloud Load Balancer → Native VM Instances
         ↓
    Each service is native unikernel image
```

## Key Features

### Cloud Support
OPS can deploy to all major cloud providers:
- Amazon Web Services (AWS)
- Google Cloud Platform (GCP)
- Microsoft Azure
- Digital Ocean
- Vultr
- UpCloud
- Oracle Cloud
- OpenStack
- ProxMox

### Hypervisor Support
- KVM
- Xen
- Bhyve
- ESX

### Language Support
- Go (static binaries, pre-tested)
- C (static binaries, pre-tested)
- Node.js (via packages)
- Python (via packages)
- Ruby, Lua, Perl (via packages)

## Basic Workflow

### 1. Install OPS
```bash
curl https://ops.city/get.sh -sSfL | sh
```

### 2. Create ops.json Configuration
```json
{
  "Args": ["-port", "8080"],
  "Env": {
    "LOG_LEVEL": "debug"
  },
  "Mounts": {
    "/data": "data"
  },
  "ManifestPassthrough": {
    "exec_protection": "t"
  }
}
```

### 3. Build and Create Image
```bash
ops build ./my_app
ops image create ./my_app
```

### 4. Deploy to Cloud
```bash
# AWS
ops instance create my-app-image -c aws

# Google Cloud
ops instance create my-app-image -c gcp

# Azure
ops instance create my-app-image -c azure
```

## Configuration (ops.json)

### Application Configuration
```json
{
  "Runtime": "go",              // go, node, c, etc.
  "Args": ["--config", "prod"], // CLI arguments
  "Env": {                       // Environment variables
    "LOG_LEVEL": "info",
    "DATABASE_URL": "postgres://...",
    "TLS_CERT_PATH": "/etc/certs/cert.pem"
  },
  "Mounts": {                    // Volume mounts
    "/data": "local_data_dir",   // Guest path: Host path
    "/etc/config": "config.json"
  },
  "Ports": {
    "8080": "8080"               // Guest:Host port mapping
  }
}
```

### Security Configuration
```json
{
  "ManifestPassthrough": {
    "exec_protection": "t"       // Prevent executing new binaries
  },
  "Security": {
    "ASLR": true,                // Address Space Layout Randomization
    "StackCanaries": true,       // Stack overflow protection
    "DEP": true                  // Data Execution Prevention
  }
}
```

### Performance Tuning
```json
{
  "MemorySize": "256",           // MB of RAM
  "CPUs": 2,                     // CPU cores
  "QemuOpts": "-smp 2",          // Raw QEMU options
  "InstanceType": "t3.small"     // Cloud-specific type
}
```

## NGI Service Deployment Example

### Auth Service as Unikernel
```json
{
  "Runtime": "go",
  "Args": ["--port", "8082"],
  "Env": {
    "LOG_LEVEL": "info",
    "DB_SERVICE": "db.internal",
    "RUST_LOG": "auth=debug"
  },
  "ManifestPassthrough": {
    "exec_protection": "t",
    "ASLR": true
  },
  "MemorySize": "128",
  "CPUs": 1
}
```

### Build Process
```bash
# 1. Compile Go service (static binary)
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -o auth-service ./cmd/auth

# 2. Build ops image
ops image create auth-service -t nanos

# 3. Deploy to AWS
ops instance create auth-service-image -c aws -i t3.micro
```

## Syscall Support

### Supported (NGI Relevant)
- Socket operations (TCP/UDP)
- File I/O (read/write/open/close)
- Timers and async I/O
- Networking (epoll, select, poll)
- Process signals
- Memory management (mmap, mprotect)

### Unsupported
- fork/exec/vfork
- User/group permissions
- SSH/shells
- IPC mechanisms (semget, msgget, shmget)
- Process management syscalls

## Performance Characteristics

| Metric | Value | vs Container |
|--------|-------|-------------|
| Boot Time | 72-195ms | 10-100x faster |
| Memory (Go) | 37-40MB | 90%+ reduction |
| Memory (C) | 24-26MB | 95%+ reduction |
| Requests/sec | 2-3x baseline | 2-3x faster |
| Image Size | <100MB | 10-50x smaller |

## Security Features

### Reduced Attack Surface
- Single process (no fork)
- No SSH access
- No shell
- Exec protection (optional)
- 10,000s fewer lines of kernel code vs Linux

### Built-in Protections
- ASLR (Address Space Layout Randomization)
- DEP (Data Execution Prevention)
- Stack Canaries
- No null page mapping
- Read-only rodata
- Non-writable code sections

### NGI-Specific Security
```json
{
  "ManifestPassthrough": {
    "exec_protection": "t"       // Cannot execute new binaries
  }
}
```

With exec_protection enabled:
- Application cannot exec() new programs
- Application cannot modify executable files
- Prevents code injection attacks
- Cannot be disabled at runtime

## Development Workflow

### Local Testing
```bash
# Build and run locally with qemu
ops run ./my_app

# With strace debugging
ops run -d ./my_app

# Monitor with ftrace
ops run ./my_app -trace
```

### Cloud Deployment
```bash
# Create cloud-specific image
ops image create ./auth-service -c aws

# List available images
ops image list

# Inspect image contents
ops image tree ./auth-service.img

# Extract filesystem from image
ops image dump ./auth-service.img -d ./extracted
```

## NGI Deployment Scenarios

### Scenario 1: Hybrid Deployment
```
LBRP (Load Balancer) - Container (complex routing)
  ├─ Auth Service - Unikernel (stateless, simple)
  ├─ Admin Service - Unikernel (stateless, simple)
  └─ Custodian - Container (complex state management)
```

### Scenario 2: Full Unikernel Stack
All NGI services as unikernels with:
- Custom DNS for service discovery
- Shared security boundary (still more secure than containers)
- Unified deployment via OPS
- Reduced operational overhead

## Limitations & Workarounds

| Limitation | Impact on NGI | Workaround |
|-----------|---------------|-----------|
| No fork/exec | Cannot spawn child processes | Design as single-threaded process |
| No shell/SSH | Cannot debug interactively | Use OPS strace/ftrace tools |
| Limited IPC | Cannot use semaphores | Use application-level IPC (gRPC) |
| No users/perms | Must run as single user | Rely on network isolation |
| Single process | Multiple cores need threads | Use async/await patterns |

## Manifest Configuration

The ops.json manifest is the core configuration for building unikernels:

```json
{
  "Runtime": "go",
  "Args": ["--config", "prod"],
  "Env": {
    "LOG_LEVEL": "debug"
  },
  "Mounts": {
    "/etc/config": "config.json"
  },
  "ManifestPassthrough": {
    "exec_protection": "t",
    "fault": "t",          // Enable fault injection for testing
    "futex_trace": "t",    // Trace futex operations
    "debugsyscalls": "t"   // Debug syscall invocations
  },
  "MemorySize": "256",
  "CPUs": 2
}
```

## Rust Examples from OPS Project

The OPS project provides several Rust examples demonstrating different deployment patterns. All examples are available at: https://github.com/nanovms/ops-examples/tree/master/rust

### 01-Hello-World (Basic CLI Application)
**Use Case:** Simple command-line utility deployment

**Building on Linux:**
```bash
rustc main.rs -o main
ops run main
```

**Building on macOS (Cross-compile):**
Two approaches available:

**Option A: Static with MUSL (Recommended)**
```bash
# Setup once
rustup target add x86_64-unknown-linux-musl

# Add to .cargo/config
[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"

# Build
TARGET_CC=x86_64-linux-musl-gcc \
RUSTFLAGS="-C linker=x86_64-linux-musl-gcc" \
cargo build --target=x86_64-unknown-linux-musl
```

**Option B: Dynamic with Full ASLR (Faster, more secure)**
```bash
# Setup cross-compilation toolchain
TARGET_CC=x86_64-unknown-linux-gnu-gcc \
rustc --target=x86_64-unknown-linux-gnu \
-C linker=x86_64-unknown-linux-gnu-gcc main.rs

# Create config.json with library directories
{
  "Dirs": ["lib64"],
  "ManifestName": "bob.manifest"
}

# Copy required libraries
mkdir -p lib64
cp /path/to/ld-linux-x86-64.so.2 lib64/
cp /path/to/libc.so.6 lib64/
# ... copy other required libs

# Find what's needed
ldd main  # Shows all dependencies

# Run with OPS
LD_LIBRARY_PATH=lib64 ops run -c config.json main
```

**NGI Integration:** Use this pattern for stateless services that require minimal dependencies. The static MUSL approach is preferred for unikernel deployment as it eliminates the need to manage library dependencies.

### 02-HTTP-Hello-World (HTTP Server)
**Use Case:** REST API and network services (core for NGI services)

**Simple Build and Run:**
```bash
rustc http_server.rs -o http
ops run -p 8080 http
```

**NGI Integration Pattern:**
This demonstrates how to deploy NGI services (Auth, Admin, LBRP) as HTTP servers:

```bash
# Build NGI service
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -o auth-service ./cmd/auth

# Build Rust equivalent (if porting to Rust)
cargo build --release --target x86_64-unknown-linux-musl

# Run with OPS
ops run -p 8082 ./auth-service

# Or deploy to cloud
ops image create auth-service
ops instance create auth-service-image -c aws -i t3.micro
```

**Key Differences from Container Deployment:**
- No orchestration needed
- Direct port binding (no port mapping)
- Unikernel boots in 72-195ms (vs 1-3s for containers)
- Fixed memory footprint (24-40MB for Rust services)
- No shell access or debug utilities

### 03-Rust-SQLx (Database Connectivity)
**Use Case:** Services that need database access (Custodian, Admin services)

**Setup:**
```bash
# Create project
cargo new sqlx-example
cd sqlx-example

# Build
cargo build --release

# Create filesystem layout with required libraries
mkdir -p lib/x86_64-linux-gnu
mkdir -p etc

# Copy NSS libraries (required for DNS/hostname resolution)
cp /lib/x86_64-linux-gnu/libnss_compat.so.2 lib/x86_64-linux-gnu/.
cp /lib/x86_64-linux-gnu/libnss_files.so.2 lib/x86_64-linux-gnu/.

# Copy nsswitch.conf (controls name service resolution)
cp /etc/nsswitch.conf etc/.
```

**config.json:**
```json
{
  "Args": ["-c", "postgres://user:pass@db.internal/ngi"],
  "Dirs": ["lib/x86_64-linux-gnu", "etc"],
  "Env": {
    "DATABASE_URL": "postgres://user:pass@db.internal/ngi"
  }
}
```

**Running:**
```bash
ops run -c config.json target/release/sqlx-example

# For production deployment
ops image create sqlx-app -c config.json
ops instance create sqlx-app-image -c aws -i t3.small
```

**NGI Integration:** This pattern is essential for:
- Admin service (user management queries)
- Custodian service (distributed locking state)
- Any service that needs persistent state

**Important:** When using database connections in unikernels:
1. NSS libraries must be included for DNS resolution
2. Connection pooling is critical (no process pooling available)
3. Ensure database is accessible from hypervisor network
4. Use connection timeouts to prevent hanging on network failure

### 04-RoAPI (Advanced: REST API from Data)
**Use Case:** REST API servers with complex dependency management

**Easiest Approach: Pre-built Static Binary**
```bash
# Download pre-built MUSL static binary
wget https://github.com/roapi/roapi/releases/download/roapi-http-v0.1.3/roapi-http-x86_64-unknown-linux-musl.tar.gz
tar xzf roapi-http-x86_64-unknown-linux-musl.tar.gz

# config.json with S3 data source
{
  "Args": ["-a", "0.0.0.0:8080", "-t", "spacex:s3://bucket/data.json"],
  "Dirs": ["root"]
}

# Place AWS credentials
mkdir -p root/.aws
cp ~/.aws/credentials root/.aws/
cp ~/.aws/config root/.aws/

# Deploy
ops run -c config.json -p 8080 roapi-http
```

**Building from Source:**
```bash
# Create directory structure
mkdir -p lib/x86_64-linux-gnu etc usr/lib/ssl usr/lib/x86_64-linux-gnu

# Copy dynamic libraries (larger, but enables ASLR)
cp /lib/x86_64-linux-gnu/libnss_compat.so.2 lib/x86_64-linux-gnu/.
cp /lib/x86_64-linux-gnu/libnss_files.so.2 lib/x86_64-linux-gnu/.
cp /lib/x86_64-linux-gnu/libnss_systemd.so.2 lib/x86_64-linux-gnu/.
cp /usr/lib/ssl/openssl.cnf usr/lib/ssl/.
cp /lib/x86_64-linux-gnu/libcrypto.so.1.1 lib/x86_64-linux-gnu/.
cp /lib/x86_64-linux-gnu/libssl.so.1.1 lib/x86_64-linux-gnu/.
cp /etc/nsswitch.conf etc/.

# Debug: Find missing libraries
ops run --trace -c config.json -p 8080 roapi-http 2>&1 | grep "not found"
```

**config.json (from source):**
```json
{
  "Args": ["-a", "0.0.0.0:8080", "-t", "spacex:s3://bucket/data.json"],
  "Dirs": ["root", "usr", "etc", "lib"]
}
```

**NGI Integration:** This pattern applies to LBRP reverse proxy which needs:
- SSL/TLS support (OpenSSL libraries)
- Environment variables for upstream service discovery
- Data source configuration (routing rules)
- Credential management (TLS certificates)

**Libraries Required for SSL-based Services:**
```
libnss_compat.so.2   # Name service support
libnss_files.so.2    # File-based name services
libcrypto.so.1.1     # Cryptography (TLS)
libssl.so.1.1        # SSL/TLS library
libgcc_s.so.1        # GCC runtime
libpthread.so.0      # Threading support
libc.so.6            # C standard library
```

## Testing & Debugging

### Strace Equivalent
```bash
ops run -d ./service
# Shows all syscalls invoked
```

### Ftrace
```bash
ops run ./service -trace
# Function-level tracing
```

### Http Server Dump
```bash
ops run ./service --http-dump
# Show all HTTP traffic
```

## NGI Best Practices

1. **Use Static Binaries:** Compile with `-fPIC` and static linking
2. **Minimize Mounts:** Only mount what's necessary
3. **Async I/O:** Use tokio async patterns for concurrency
4. **No Shell:** Never exec shell commands in service
5. **Security:** Enable exec_protection for production
6. **Testing:** Test locally with `ops run` before deployment

## References

- **Official Documentation:** https://docs.ops.city/ops/
- **GitHub Repository:** https://github.com/nanovms/ops
- **Nanos Unikernel:** https://nanos.org/thebook
- **Community Forums:** https://forums.nanovms.com/

---

**Last Updated:** December 2025
**Documentation Version:** OPS 0.1.27+
**Status:** Optional alternative deployment platform for NGI
