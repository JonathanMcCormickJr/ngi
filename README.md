# NGI: *Next-Gen Infoman* 🎫

(c) 2025 **Jonathan A. McCormick, Jr.** All rights reserved. If you are interested in using this software, please contact the author for licensing information.

NGI aims to solve the pain points experienced by users of the current generation of DSR's internal ticketing system, Infoman3. NGI is designed to be modular, extensible, scalable, fault-tolerant, user-friendly, blazingly fast, ultra secure, and easily maintainable. This is a system built from the ground up to support DSR's tech support capabilities through the next 40 years and beyond.

DISCLAIMER: This project is neither endorsed nor sponsored by DSR, and is just a grassroots effort at innovation on the author's own personal time. 

## Features
- Ticket Creation & Management
- User Authentication & Authorization with MFA
- Role-Based Access Control
- Real-Time Notifications
- Advanced Search & Filtering
- Audit Logging & Reporting
- Internally uses gRPC between independent components
- RESTful API for Integration with Other Systems
- Fault Injection for Resilience Testing
- Intrusion Detection via Honeypot Service
- Auto-save Drafts to Browser Cookies (recovery from interruptions)
- Thorough Automated Test Coverage
- Dynamic Schema Evolution (add/remove fields and workflow steps without downtime)
- Zero-redundancy data entry. If a field is already filled in from another source, it will not be requested again.
- Only one user may possess a ticket's lock at a time, preventing edit conflicts. Multiple tickets may be simultaneously locked by the same user.

For a list of user-generated feature requests, please see [FEATURE_REQUESTS.md](./FEATURE_REQUESTS.md).

## Ticket Structure
Each ticket in NGI contains the following fields:
- Ticket Number (unique identifier): auto-incremented unsigned 64-bit integer
- Customer Ticket Number: optional field for the customer's own ticket reference
- ISP Ticket Number: optional field for the ISP's own ticket reference
- Other Ticket Number: optional field for the partner's own ticket reference
- Title: brief summary of the issue or request
- Project: associated project/customer organization
- Account UUID: a universally unique identifier (UUID) representing the account associated with the ticket
- Symptom: an enum representing the primary symptom of the issue. Values are represented as unsigned 8-bit integers for efficiency.
- Priority: an enum representing the severity of the issue as it fits within the overall context from the customer's perspective. Values are represented as unsigned 8-bit integers for efficiency.
- Status: an enum representing the current status of the ticket. Values are represented as unsigned 8-bit integers for efficiency. Depending on the status, certain fields may be required or optional. Examples:
    - Closed, Auto-close: require Resolution field to be not None.
    - Open, Awaiting Customer, Awaiting ISP, Awaiting Partner: require Next Action to be not None.
- Next Action: enum representing the next action to be taken on the ticket. Values are represented as unsigned 8-bit integers for efficiency. `None` is a valid value indicating that no immediate action is required. `FollowUp` indicates a normal follow-up action and the scheduled date & time of it. `Appointment` indicates that a critical appointment is scheduled that must be attended, and provides the scheduled date & time. `AutoClose` indicates that the ticket is scheduled to be automatically closed at the provided date & time if no further action is taken. `AutoClose` can be set by the user for any of the following timeframes: EOD (end of day), 24 hours, 48 hours, 72 hours. 
- Resolution: an enum representing the resolution of the ticket. Values are represented as unsigned 8-bit integers for efficiency. `None` is a valid value indicating that the ticket has not yet been resolved.
- Lock: optional field indicating which user has locked the ticket for editing.
- Assigned To: optional field indicating which user/team is assigned to the ticket.
- Created By: user who created the ticket
- Created At: timestamp of ticket creation
- Updated By: user who last updated the ticket
- Updated At: timestamp of last ticket update
- History: a log of all changes made to the ticket, including timestamps and user information
- Ebond: optional field for ebonding information
- Tracking URL: optional URL for DSR Broadband Provisioning portal tracking record (auto-updated with ticket summaries)
- Network Devices: collection of DSR-supported equipment at the site (modems, ONTs, routers, switches, firewalls, VPN gateways, antennas) with make, model, MAC address, and serial number

## Design Goals
- **Modular**: NGI is built using a collection of smaller components, allowing for easy addition and removal of features as needed.
- **Extensible**: The system is designed to accommodate future enhancements and integrations with other tools and platforms. Management can add custom fields, enum options, and workflow states without code changes—schema versioning and lazy migrations enable live evolution of data models.
- **Scalable**: NGI can handle increasing loads and user demands without compromising performance.
- **Fault-Tolerant**: The system is resilient to failures, ensuring continuous operation and minimal downtime. It even includes its own fault-injection system (inspired by Netflix's Chaos Monkey) to help identify and fix potential points of failure.
- **User-Friendly**: NGI features an intuitive interface that simplifies ticket management for users of all technical levels. The frontend automatically saves drafts to browser cookies, so users never lose their work due to interruptions, crashes, or accidental navigation.
- **Blazingly Fast**: Optimized for speed, NGI ensures quick response times and efficient ticket processing. It takes advantage of the Rust programming language's support for both parallelism and asynchronous programming in order to push performance to the limit. With DSR's ambitions for growth, this system is designed to handle thousands of concurrent users without breaking a sweat.
- **Ultra Secure**: NGI incorporates robust security measures to protect sensitive information and maintain user privacy. All network communications are doubly-encrypted: first using TLS 1.3 and secondly with a NIST-vetted postquantum algorithm named CRYSTALS-Kyber. User authentication is handled with mandatory MFA using several methods, including password-based authentication, WebAuthn, U2F, TOTP, and Active Directory (where the user's underlying OS login status counts toward authentication).
- **Easily Maintainable**: The system is designed for straightforward maintenance and updates, reducing the burden on IT and development teams. Strict **Test-Driven Development (TDD)** ensures high code quality from day one—tests are written before implementation, documenting intended behavior and catching regressions early. Automated checks for dependencies (`cargo audit`), code quality (`cargo clippy` & `cargo fmt`), test coverage, documentation, and security vulnerabilities are integrated into the development workflow to ensure the system remains robust and up-to-date.

## Architecture
NGI is built using a microservices architecture, with each service responsible for a specific function within the ticketing system. Services communicate internally using gRPC over HTTP/2 with mutual TLS for maximum performance and security, while the load balancer (LBRP) exposes a RESTful JSON API to browsers and external partners. Internal components within each service use Tokio channels for asynchronous message passing. This ensures loose coupling and high cohesion while squeezing every bit of practical performance from inter-service communication. Each service can be developed, deployed, and scaled independently, allowing for greater flexibility and agility in responding to changing requirements. Each service (including the load balancer itself) is also capable of running multiple instances for load balancing and high availability.

See [ARCHITECTURE.md](./ARCHITECTURE.md) for detailed diagrams and explanations of the system architecture.

### Key Components
- [**Admin:**](./admin/) Manages user accounts, roles, and permissions within the NGI system.
- [**Auth:**](./auth/) Handles user authentication and authorization, including support for MFA and various authentication methods.
- [**Chaos:**](./chaos/) Injects faults into the system to test resilience and fault-tolerance capabilities
- [**Custodian:**](./custodian/) Controls tickets, including creation, updates, assignments (including ticket locks), and status changes.
- [**DB: Database Service:**](./db/) Manages data storage and retrieval, ensuring data integrity and consistency across the system.
- [**Honeypot (CriticalBackups):**](./honeypot/) Deceptive high-value target service for intrusion detection. Captures attacker behavior and reports to admin for logging.
- [**LBRP: Load Balancer & Reverse Proxy:**](./lbrp/) Distributes incoming requests across multiple instances of each service to ensure optimal performance and reliability. Also serves static files for the web frontend.
- [**Tests:**](./tests/) Contains integration and end-to-end tests for the entire NGI system, ensuring that all components work together seamlessly.

### Inter-Service Communication
All inter-service communication uses gRPC (via `tonic`) over HTTP/2, secured with mutual TLS (mTLS) to ensure that only authorized services can communicate with each other. Each service has its own unique certificate and private key, which are used to establish secure connections. This approach helps to prevent unauthorized access and ensures the integrity of data exchanged between services. The LBRP service translates incoming REST/JSON requests from browsers into gRPC calls and returns JSON responses.

### Consistency Model
NGI employs a flexible consistency model. Operations that critically rely on data consistency, such as setting/clearing ticket locks, utilize strong consistency to ensure data integrity. Operations that have no such requirement, such as UI format, maintain flexibility. For example, the UI can have A/B testing enabled, allowing different users to experience different UI layouts without impacting the underlying data consistency.

## Setup & Deployment
NGI is deployed as a collection of Nanos unikernels created using [NanoVMs's OPS tool](https://github.com/nanovms/ops). Docker is banned from this project due to attack surface concerns. Example usage of OPS can be found at https://github.com/nanovms/ops-examples/tree/master/rust. 

## CI/CD
Continuous integration and deployment (CI/CD) pipelines are set up using GitHub Actions to automate the build, test, and deployment processes for NGI. This ensures that new features and bug fixes are delivered quickly and reliably to users.

### Requirements for successful deployment
- All tests must pass successfully with `cargo test`.
- Code coverage must meet or exceed the 90% as specified by `cargo tarpaulin`.
- No security vulnerabilities detected by `cargo audit`.
- Code adheres to style guidelines enforced by `cargo fmt` and `cargo clippy`.
- Documentation is up-to-date and complete.

## Development Environment

### Local Setup
```bash
# Clone the repository
git clone https://github.com/JonathanMcCormickJr/ngi.git
cd ngi

# Install Rust toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install development tools
cargo install cargo-watch
cargo install cargo-tarpaulin
cargo install cargo-audit
cargo install cargo-fuzz

# Build the project
cargo build

# Run tests
cargo test
```

### Running Services Locally
```bash
# Terminal 1: Start database service
cargo run --bin db -- --dev-mode --raft-node-id 1

# Terminal 2: Start auth service
cargo run --bin auth -- --dev-mode

# Terminal 3: Start LBRP service
cargo run --bin lbrp -- --dev-mode

# Terminal 4: Run tests against local services
cargo test --test integration_test
```

### Development Workflow
```bash
# Watch mode for continuous testing
cargo watch -x test

# Run specific service tests
cargo test -p db
cargo test -p auth

# Check coverage
cargo tarpaulin --out Html --output-dir coverage
open coverage/index.html

# Security audit
cargo audit

# Format and lint
cargo fmt
cargo clippy -- -D warnings
```

### Debugging
```bash
# Enable debug logging
RUST_LOG=debug cargo run --bin db

# Run with backtrace on panic
RUST_BACKTRACE=1 cargo test

# Debug specific test
cargo test test_name -- --nocapture
```

### Multi-Node Development Testing
```bash
# Terminal 1: First database node
cargo run --bin db -- --dev-mode --raft-node-id 1 --port 8080

# Terminal 2: Second database node
cargo run --bin db -- --dev-mode --raft-node-id 2 --port 8081

# Terminal 3: Third database node
cargo run --bin db -- --dev-mode --raft-node-id 3 --port 8082
```

### Code Quality Checks
```bash
# All quality checks (run before commit)
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo tarpaulin --fail-under 90
cargo audit
```

### IDE Setup
- **VS Code:** Install Rust Analyzer extension
- **IntelliJ/CLion:** Built-in Rust support
- **Vim/Neovim:** rust.vim or coc-rust-analyzer
- **GitHub Copilot:** Use `.github/copilot-instructions.md` for context

## Production Operations

### Service Health Checks
```bash
# Check service health
curl -k https://service.ngi.internal:808x/health

# Check cluster status
curl -k https://admin.ngi.internal:8083/cluster/status

# View metrics
curl -k https://admin.ngi.internal:8083/metrics
```

### Log Access
```bash
# View recent logs
curl -k https://admin.ngi.internal:8083/logs?service=db&lines=100

# Search logs
curl -k "https://admin.ngi.internal:8083/logs/search?query=error&since=1h"
```

### Certificate Management
```bash
# Check certificate expiry
curl -k https://admin.ngi.internal:8083/certs/expiry

# Rotate certificates (automated)
curl -X POST -k https://admin.ngi.internal:8083/certs/rotate
```

### Backup & Recovery
```bash
# Create database snapshot
curl -X POST -k https://db-leader.ngi.internal:8080/admin/snapshot

# List available backups
curl -k https://admin.ngi.internal:8083/backups

# Restore from backup
curl -X POST -k https://db-leader.ngi.internal:8080/admin/restore \
  -H "Content-Type: application/json" \
  -d '{"backup_id": "backup-2025-12-11"}'
```

### Troubleshooting
```bash
# Common issues and solutions
curl -k https://admin.ngi.internal:8083/docs/troubleshooting

# Generate diagnostic report
curl -k https://admin.ngi.internal:8083/diagnostics > diagnostics.json
```

