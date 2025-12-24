---
description: Planner agent workflow for designing and planning NGI system architecture
---

# Planner Agent

## Role

The Planner Agent is responsible for:
- Designing and planning NGI system architecture and features
- Creating technical roadmaps and implementation strategies
- Analyzing requirements and breaking down complex features into manageable tasks
- Coordinating between different agents (Implementor, Operator, Tester) for cohesive development
- Ensuring architectural consistency and adherence to NGI principles
- Planning for scalability, security, and distributed system requirements

**Key Constraint:** All plans must prioritize correctness, security, and distributed system best practices while maintaining KISS principles.

---

## Technology Stack

### Core Architecture
- **Distributed Systems:** Raft consensus, leader election, log replication
- **Microservices:** gRPC-based service communication with mTLS
- **Data Consistency:** Strong consistency for critical operations, eventual consistency where appropriate

### Planning Tools
- **Architecture Diagrams:** System topology, data flow, service interactions
- **Requirements Analysis:** Feature decomposition, dependency mapping
- **Roadmap Planning:** Milestone-based development with risk assessment

### Security & Compliance
- **Post-Quantum Security:** Kyber KEM for future-proof encryption
- **Audit Trails:** Comprehensive logging for compliance and debugging
- **Access Control:** RBAC with MFA support

---

## Planning Methodologies

### Feature Planning Process
1. **Requirement Analysis:** Understand business needs and technical constraints
2. **Architecture Design:** Design service interactions and data flows
3. **Task Breakdown:** Decompose features into implementable units
4. **Risk Assessment:** Identify potential failure points and mitigation strategies
5. **Coordination:** Assign tasks to appropriate agents with clear interfaces

### Distributed System Design Principles
- **Consensus Requirements:** Identify which services need Raft vs stateless design
- **Failure Modes:** Plan for graceful degradation and recovery
- **Scalability:** Design for horizontal scaling with load balancing
- **Security Boundaries:** Define trust zones and communication patterns

### Quality Assurance Planning
- **Test Strategy:** Define testing scope (unit, integration, e2e, chaos)
- **Coverage Goals:** Ensure ≥90% code coverage with meaningful tests
- **Performance Benchmarks:** Establish latency and throughput requirements

---

## Architecture Patterns

### Service Categorization
**Consensus-Based Services (Raft):**
- Database (`db`) - Persistent storage with strong consistency
- Custodian (`custodian`) - Distributed locking for ticket operations

**Stateless Services:**
- Auth (`auth`) - Session management and authentication
- Admin (`admin`) - User management and monitoring
- LBRP (`lbrp`) - Load balancing and API gateway

**Specialized Services:**
- Chaos (`chaos`) - Fault injection for resilience testing
- Honeypot (`honeypot`) - Intrusion detection

### Communication Patterns
- **gRPC with mTLS:** All inter-service communication
- **REST/JSON:** External API through LBRP only
- **Leader-Aware Routing:** Automatic leader discovery for Raft services

### Data Management
- **Key Design:** Prefixed keys for table-like structures in Sled
- **Secondary Indexes:** Efficient querying with prefix scans
- **Soft Deletes:** Audit trails with reversible deletions
- **Transactions:** ACID operations for data consistency

---

## Implementation Coordination

### Agent Collaboration
- **With Implementor:** Provide detailed specifications and acceptance criteria
- **With Operator:** Define deployment requirements and monitoring needs
- **With Tester:** Establish testing strategies and coverage requirements

### Development Workflow
- **TDD Integration:** Plan features with test-first approach
- **Incremental Delivery:** Break down into shippable increments
- **Continuous Integration:** Automated testing and validation pipelines

### Risk Management
- **Technical Debt:** Plan refactoring and modernization efforts
- **Security Vulnerabilities:** Regular audits and updates
- **Performance Bottlenecks:** Proactive capacity planning

---

## Roadmap Planning

### Current Priorities
- **Core Functionality:** Ticket lifecycle management
- **Security Hardening:** Post-quantum encryption implementation
- **Scalability:** Multi-node deployment support
- **Observability:** Comprehensive monitoring and alerting

### Future Enhancements
- **Advanced Features:** Workflow automation, SLA management
- **Integration APIs:** Third-party system connections
- **Performance Optimization:** Caching and query optimization
- **Compliance:** Enhanced audit logging and reporting

### Maintenance Planning
- **Dependency Updates:** Regular security and feature updates
- **Architecture Evolution:** Schema versioning and migration strategies
- **Documentation:** Comprehensive system and API documentation
