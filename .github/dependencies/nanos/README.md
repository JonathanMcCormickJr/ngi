# Nanos - Unikernel Operating System

> A single-process operating system designed for cloud applications with minimal overhead and maximum security.

**Official Docs:** https://nanos.org/thebook

**Current Version:** Latest

## Overview

Nanos is the lightweight operating system kernel that powers NGI's optional unikernel deployment model. It replaces the Linux kernel for cloud applications while providing only the syscalls and features your application actually needs.

## Architecture

### Nanos vs Traditional Linux
```
Traditional Linux:
- Multi-process OS
- ~20 million lines of code
- Designed for general purpose computing
- High overhead for simple applications

Nanos Unikernel:
- Single-process OS
- ~50,000 lines of optimized C
- Designed specifically for cloud applications
- Minimal overhead, maximum security
```

### Deployment Model
```
Cloud Hypervisor (KVM, Xen, Bhyve)
    ↓
Nanos Kernel (lightweight OS)
    ↓
Your Application (single threaded or multi-threaded)
    ↓
Direct hardware access (no context switching overhead)
```

## Core Components

### 1. Filesystem (TFS)

**Tuple File System (TFS):**
- Optimized for unikernel workloads
- Immutable kernel partition (separate from app data)
- Minimal overhead for file metadata
- Efficient serialization format

**Supported Storage Drivers:**
- virtio_blk (QEMU/KVM cloud)
- virtio_scsi (QEMU/KVM cloud)
- nvme (modern cloud instances)
- ata_pci (QEMU/KVM)
- pvscsi (VMware ESX)
- xenblk (Xen)
- storvsc (Hyper-V/Azure)

**Mounting Model:**
```
/               → Application code & data
/etc/certs      → TLS certificates
/data           → Persistent storage (from host mount)
```

### 2. Networking

**IP Support:**
- IPv4 (full support)
- IPv6 (full support)

**Network Drivers:**
- virtio_net (standard cloud)
- e1000 (compatibility)
- xen (Xen clouds)
- AWS/GCP/Azure specific optimizations

**Features:**
- TCP/UDP sockets
- DNS resolution (via gRPC or configured servers)
- Firewall rules (managed by cloud provider VPC)
- Optimized for single application networking

### 3. Memory Management

**Memory Allocators (Nanos Internal):**
- Bitmap - Efficient bit-level allocation
- ID Heap - For ID/handle allocation
- Page-Backed Heap - Per-page allocation with fine-grained protections
- Linear-Backed Heap - Continuous mapping for fast access
- Priority Queue - Resource scheduling

**NGI Relevant Characteristics:**
- Minimum 24-26MB for C applications
- Minimum 37-40MB for Go applications
- No memory overprovisioning (every byte counted)
- Hierarchical heaps for different allocation patterns

**Data Structures:**
- Bitmap - Bit vectors for resource tracking
- Red/Black Trees - Efficient tree operations
- ID Heap - Handle allocation
- Scatter/Gather Lists - Buffer management
- Tables - Key-value storage
- Tuples - Efficient serialization

### 4. Syscall Support

#### Networking (NGI Critical)
```
socket, bind, listen, accept, accept4, connect
sendto, sendmsg, sendmmsg, recvfrom, recvmsg
setsockopt, getsockname, getpeername, getsockopt
shutdown
```

#### I/O Operations
```
read, pread64, write, pwrite64, open, openat
dup, dup2, dup3, fstat, fcntl, lseek
fallocate, fadvise64, sendfile
readv, writev, truncate, ftruncate
sync, fsync, fdatasync, syncfs
```

#### Async I/O (Tokio Compatible)
```
epoll_create, epoll_create1, epoll_ctl, epoll_wait, epoll_pwait
poll, ppoll, select, pselect6
io_uring_setup, io_uring_enter, io_uring_register
timerfd_create, timerfd_gettime, timerfd_settime
eventfd, eventfd2
```

#### File Management
```
stat, lstat, newfstatat, mkdir, mkdirat
getdents, getdents64, symlink, readlink
unlink, unlinkat, rmdir, rename, renameat
chmod, fchmod (ignored), chdir, fchdir
```

#### Process & Signals
```
getpid, gettid, getrusage, exit, exit_group
clone (limited), set_tid_address
rt_sigaction, rt_sigpending, rt_sigprocmask
rt_sigtimedwait, rt_sigsuspend, signalfd
kill, tgkill, pause, alarm
```

#### Time Operations
```
clock_gettime, clock_nanosleep, gettimeofday
nanosleep, time, times
getitimer, setitimer
timer_create, timer_settime, timer_gettime
```

#### Memory Management
```
mmap, mremap, munmap, mprotect, msync
brk, mincore, mlock/munlock (ignored)
prctl (limited)
```

#### Other
```
futex (critical for Tokio)
arch_prctl (for TLS, x86_64)
capget, access, umask, getrlimit, setrlimit
getrandom, pipe, pipe2, socketpair
```

#### **Not Supported (Nanos Philosophy)**
```
fork, vfork, execve → Single process only
shmget, shmat, shmctl → No shared memory
semget, semop, semctl → No semaphores
msgget, msgsnd, msgrcv → No message queues
ptrace → No debugging syscalls
seteuid, setegid → Single user only
mount, umount → Single partition only
SSH, shells → Not supported
```

## Performance Characteristics

### Boot Time
- **Stripped Kernel:** 72ms
- **Unstripped Kernel:** 195ms
- **Typical (with app):** 100-150ms

### Memory Usage (Minimum)
| Runtime | QEMU | Firecracker | VirtualBox |
|---------|------|-------------|-----------|
| C | 26MB | 24MB | 24MB |
| Go | 40MB | 37MB | 36MB |

### Network Performance
- 2-3x request/second improvement vs traditional containers
- Reduced latency due to direct kernel pairing
- Efficient async I/O (epoll/io_uring)

## Security Model

### Threat Model
1. **Attack Surface Reduction**
   - No multi-process model (no fork)
   - No SSH/shells
   - No user/permission syscalls
   - Kernel in separate partition
   - Only necessary libraries included

2. **Privilege Isolation**
   - All code runs with same privileges
   - No UID/GID separation
   - Network isolation via VPC/firewall
   - Process isolation via hypervisor

### Security Features

#### ASLR (Address Space Layout Randomization)
- Stack randomization
- Heap randomization
- Library randomization (if multiple)
- Binary randomization

#### Page Protections
- Stack execution disabled by default
- Heap execution disabled
- Code pages non-writable
- Rodata non-executable
- Null page not mapped

#### Architecture-Level
- SMEP (Supervisor Mode Execution Protection)
- UMIP (User Mode Instruction Prevention)
- Read-only globals after init
- Stack cookies/canaries

#### NGI Optional: Exec Protection
```
ManifestPassthrough: {
  "exec_protection": "t"
}
```
When enabled:
- Application cannot exec() new programs
- Cannot create new executable files
- Cannot modify existing executable files
- Prevents code injection attacks

### Code Examples

**C Security Features:**
```c
// Stack canaries (automatic)
void function() {
    char buffer[100];
    // Stack overflow detection enabled
}

// ASLR protections (automatic)
void* heap_alloc = malloc(1000);
// Address randomized at each boot
```

**NGI Service Deployment:**
```json
{
  "ManifestPassthrough": {
    "exec_protection": "t"
  }
}
```

## Syscall Implementation

### Futex (Critical for Tokio)
```c
// Nanos supports futex for tokio's synchronization
futex(addr, FUTEX_WAIT, val, timeout)  // Wait on value
futex(addr, FUTEX_WAKE, wake_count)    // Wake waiters
futex(addr, FUTEX_CMP_REQUEUE, ...)    // Requeue operation
```

### Epoll (Critical for Async I/O)
```c
// Efficient event polling for thousands of connections
int epfd = epoll_create1(EPOLL_CLOEXEC);
epoll_ctl(epfd, EPOLL_CTL_ADD, fd, &event);
epoll_wait(epfd, events, max_events, timeout);
```

### IO_URING (Modern Async I/O)
```c
// Ring buffer for batch async operations
struct io_uring ring;
io_uring_queue_init(queue_depth, &ring, flags);
io_uring_enter(fd, to_submit, to_wait, flags);
```

## NGI Integration Pattern

### Auth Service on Nanos
```
ops/auth-service/
├── ops.json (Manifest)
├── src/main.rs (Rust service)
└── Dockerfile (For container comparison)

ops.json:
{
  "Runtime": "go",
  "Args": ["--port", "8082"],
  "Env": {
    "DB_SERVICE": "db.internal:8080"
  },
  "ManifestPassthrough": {
    "exec_protection": "t"
  }
}
```

### Build Process
```bash
# 1. Build static binary
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 \
  go build -o auth-service ./cmd/auth

# 2. Create ops image with Nanos
ops image create auth-service

# 3. Deploy to cloud (AWS)
ops instance create auth-service-image -c aws
```

## Debugging & Troubleshooting

### Syscall Tracing
```bash
ops run -d ./service
# Shows all syscalls (strace equivalent)
```

### Function Tracing
```bash
ops run ./service -trace
# ftrace equivalent
```

### HTTP Debugging
```bash
ops run ./service --http-dump
# Show all HTTP requests/responses
```

### Manifest Features
```json
{
  "ManifestPassthrough": {
    "futex_trace": "t",     // Trace futex operations
    "debugsyscalls": "t",   // Show all syscalls
    "fault": "t",           // Enable fault injection
    "exec_protect": "t"     // Prevent exec
  }
}
```

## Data Structures (Internal)

Nanos uses optimized internal data structures:

### Bitmap
- Efficient bit-level resource tracking
- Allocated in 64-bit chunks
- Used for memory/resource allocation
- Supports atomic operations

### Red/Black Trees
- Balanced binary search trees
- Ordered traversal (inorder/preorder/postorder)
- Used for kernel resource management
- O(log n) operations

### ID Heap
- Address space allocator
- Allocates from ranges of IDs/addresses
- Hierarchical (can allocate from parent heap)
- Thread-safe with optional locking

### Page-Backed Heap
- Page-granularity allocation
- Fine-grained protection flags
- Separate virtual/physical mapping
- Used for demand-paged resources

### Linear-Backed Heap
- Continuous physical memory mapping
- Largest page size optimization
- Fast lookup without page table traversal
- Used for bulk allocations

## Comparison: Nanos vs Linux

| Aspect | Nanos | Linux |
|--------|-------|-------|
| Lines of Code | ~50,000 | ~20,000,000 |
| Process Model | Single | Multi |
| Boot Time | 72-195ms | 2-5s |
| Memory (Go) | 37-40MB | 150-200MB |
| Security Model | Minimal attack surface | Complex permissions |
| Deployment | Bare metal or hypervisor | Containerized |
| Debugging | Limited (design constraint) | Full SSH access |

## NGI Use Cases

### ✅ Good Fit
- Stateless services (Auth, Admin, LBRP)
- Single-threaded async workloads (Tokio services)
- Fixed dependencies
- High-security requirements
- Cost-sensitive deployments

### ⚠️ Challenging
- Multi-process coordination
- Complex debugging requirements
- Services needing shell access
- Variable dependencies

### ❌ Not Suitable
- Legacy multi-process applications
- Services requiring exec()
- Complex orchestration needs

## References

- **Official Book:** https://nanos.org/thebook
- **GitHub Repository:** https://github.com/nanovms/nanos
- **OPS Tool:** https://docs.ops.city/ops/
- **Architecture Details:** https://nanos.org/static/img/vms-vs-unikernels.png
- **Security Details:** https://github.com/nanovms/nanos/blob/master/SECURITY.md

---

**Last Updated:** December 2025
**Documentation Version:** Nanos Latest
**Status:** Optional deployment platform for NGI services
**Architecture:** Optimized single-process unikernel
