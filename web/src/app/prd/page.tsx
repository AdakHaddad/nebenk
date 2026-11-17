'use client';

import { DocsLayout } from '@/app/docs-layout';
import { MarkdownRenderer } from '@/components/MarkdownRenderer';

export default function PrdPage() {
  const content = `
# Product Requirements Document

**Status:** Pre-implementation  
**Version:** 0.4  
**Last Updated:** 2026-09-22  
**License:** Apache License 2.0  

---

## 1. Overview

NEBENK is an experimental, lightweight, device-agnostic distributed application runtime.

It is designed to allow an application to run across heterogeneous devices and maintain recoverable state when individual devices become unavailable.

A NEBENK cluster may consist of:
- smartphones
- laptops
- desktops
- Raspberry Pis
- set-top boxes
- servers
- other supported computing devices

The central idea is:

> **«The application survives. The device is disposable.»**

Instead of requiring an application to permanently belong to one server, NEBENK treats available devices as interchangeable infrastructure.

\`\`\`
                    Application
                         │
                  Portable State
                         │
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
        Phone          Laptop        Server
        Node A         Node B        Node C
\`\`\`

NEBENK is currently a design-stage project. This document defines the intended product and the requirements for its first implementation.

---

## 2. Problem

Modern application deployment commonly assumes relatively stable infrastructure:

\`\`\`
Application
     ↓
Server
     ↓
Database
     ↓
Persistent infrastructure
\`\`\`

High availability generally requires additional infrastructure such as:
- multiple servers
- replicated databases
- service discovery
- load balancing
- orchestration
- dedicated networking

This is appropriate for many production systems, but it can be excessive for smaller or more heterogeneous environments.

At the same time, users increasingly have multiple capable devices:
- Phone
- Laptop
- Desktop
- Raspberry Pi
- Old PC
- Server

These devices may have spare resources but are not normally treated as a unified infrastructure.

There is currently no simple application-level abstraction that allows a developer to say:

> **«"Run this stateful application wherever my available devices can host it, and allow another device to recover its state when one disappears."»**

NEBENK aims to provide that abstraction.

---

## 3. Vision

NEBENK aims to make distributed infrastructure feel like a property of the application rather than a collection of infrastructure systems the developer must manually operate.

**Traditional thinking:** *«Where does my application run?»*  
**NEBENK:** *«Which devices are currently available to run it?»*

The desired experience is:

\`\`\`
Build application
       ↓
Deploy to NEBENK
       ↓
Devices participate
       ↓
State is replicated
       ↓
Devices may disappear
       ↓
Application remains recoverable
\`\`\`

---

## 4. Product Thesis

NEBENK is based on several principles:

### 4.1 Application ≠ Device
An application should not fundamentally belong to one machine.

### 4.2 Application ≠ Process
The current process and its RAM are not the durable source of truth.

### 4.3 Application = Portable State Machine
The application exposes deterministic state transitions that can be persisted and reconstructed.

### 4.4 Devices Are Temporary
Devices can join, leave, disconnect, reconnect, crash, or be replaced without requiring manual restoration of application state.

### 4.5 Distributed Complexity Belongs in the Runtime
The developer should not need to implement peer discovery, replication, state snapshots, node identity, failure detection, or state synchronization for every application.

---

## 5. Goals

| ID | Goal | Description |
|----|------|-------------|
| **G1** | Portable Application Execution | An application should be executable on different supported OS and CPU architectures without requiring separate implementation per device. |
| **G2** | Portable Application State | Application state should be recoverable on another device. |
| **G3** | Dynamic Device Participation | Devices should be able to join and leave a cluster without rebuilding the application. |
| **G4** | Fault Recovery | The system should allow an eligible replica to recover application state following node failure. |
| **G5** | Lightweight Runtime | The runtime should be significantly lighter than requiring a complete VM or traditional container stack on every participating device. |
| **G6** | Device Agnosticism | Applications should not need to know whether they are running on Android, Linux, Windows, macOS, ARM, x86, or another supported platform. |
| **G7** | Developer Simplicity | The developer should define application behavior and state while NEBENK handles distributed infrastructure. |
| **G8** | Local-First Operation | The system should be useful on local networks without requiring a permanent cloud service. |
| **G9** | Open Infrastructure | NEBENK should be open-source and usable as a foundation for independent projects, research, commercial software, and community infrastructure. |

---

## 6. Non-Goals

The following are explicitly outside the initial scope:

| Non-Goal | Description |
|----------|-------------|
| **6.1** | Arbitrary Process Migration - NEBENK will not attempt to transparently migrate arbitrary process memory, CPU registers, thread stacks, OS state, or arbitrary running processes. |
| **6.2** | Full Virtual Machine - NEBENK is not intended to virtualize an entire operating system. |
| **6.3** | Kubernetes Replacement - Not intended to provide large-scale container orchestration, cloud-scale scheduling, service mesh, or enterprise cluster management. |
| **6.4** | Docker Replacement - Does not attempt to reproduce Docker's entire feature set. |
| **6.5** | Universal Distributed Database - NEBENK is not itself a general-purpose database. |
| **6.6** | Byzantine Fault Tolerance - Malicious or actively Byzantine nodes are outside the initial trust model. |
| **6.7** | Guaranteed Availability With No Nodes - If all eligible nodes are offline, NEBENK cannot provide service availability. |

---

## 7. Target Users

| Segment | Description |
|---------|-------------|
| **7.1** Independent Developers | Developers who want lightweight distributed deployment without maintaining permanent infrastructure. |
| **7.2** Distributed Systems Developers | Developers experimenting with replication, P2P systems, fault tolerance, edge computing, decentralized infrastructure. |
| **7.3** Edge and IoT Developers | Developers operating collections of Raspberry Pis, Android devices, embedded computers, local edge nodes. |
| **7.4** Small Organizations | Organizations that have several computers but do not want to maintain a dedicated server for every service. |
| **7.5** Researchers and Students | NEBENK can provide an accessible environment for experimenting with distributed systems using ordinary devices. |

---

## 8. Primary Use Cases

### 8.1 Personal Infrastructure
A user has Desktop, Laptop, Phone, Raspberry Pi and wants a personal application to survive the loss of any individual device.

### 8.2 Small Office
Several existing office machines participate in hosting a service.

### 8.3 Local/Edge Infrastructure
Devices in the same physical environment cooperate without relying entirely on cloud infrastructure.

### 8.4 Temporary Cluster
Users create a temporary cluster for an event, laboratory, classroom, field operation, or local network.

### 8.5 Development and Testing
Developers use multiple personal devices to test distributed applications.

---

## 9. Core Concepts

| Concept | Description |
|---------|-------------|
| **Application** | The software being executed (e.g., "counter") |
| **Application Artifact** | A portable representation of an application (initial target: WebAssembly-based) |
| **Node** | A device running the NEBENK runtime |
| **Cluster** | A logical group of authorized nodes participating in an application |
| **State** | The durable logical state of the application |
| **Operation** | A state-changing command applied to the application |
| **Snapshot** | A persisted representation of application state at a particular revision |
| **Replica** | A node maintaining a recoverable copy of application state |

---

## 10. Product Architecture

\`\`\`
┌──────────────────────────────┐
│         Application          │
│                              │
│          app.wasm            │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│       Application Runtime    │
│                              │
│       State Machine          │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│      Distributed Runtime     │
│                              │
│ Replication                  │
│ Membership                   │
│ Failure Detection            │
│ Synchronization              │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│          Node Runtime        │
│                              │
│ Networking                   │
│ Identity                     │
│ Storage                      │
│ Resource Management          │
└──────────────┬───────────────┘
               │
               ▼
           Host OS
\`\`\`

---

## 11. Application Model

The initial application model is a deterministic state machine:

\`\`\`
State + Command → New State
\`\`\`

**Example:**
- State: \`counter = 10\`
- Command: \`INCREMENT 5\`
- Result: \`counter = 15\`

The same command applied to the same state should produce the same result, allowing multiple nodes to reconstruct equivalent state.

---

## 12. State Durability Model

NEBENK represents durable application state through **Snapshot + Operation Log**:

\`\`\`
Snapshot #100
     +
Operation #101
     +
Operation #102
     +
Operation #103
     ↓
Current State
\`\`\`

A new node may receive:
- **Recent Snapshot** + **Missing Operations** (instead of entire historical log)

---

## 13. Replication Model

### V1: Primary/Replica
\`\`\`
             Primary
                │
          ┌─────┴─────┐
          ▼           ▼
       Replica A   Replica B
\`\`\`

Primary accepts state-changing operations; replicas maintain recoverable copies.

### V2: Consensus-Backed
Raft or similar for stronger consistency and quorum-based commitment.

### V3+: Eventual Consistency
CRDT-based multi-writer replication where appropriate.

---

## 14. V1 Consistency Boundary

V1 must explicitly distinguish replicated state from quorum-committed state.

\`\`\`
Client
  ↓
Primary
  ↓
Local commit
  ↓
Replication
  X
Replica unavailable
\`\`\`

If the primary fails before replication completes, the latest operation may not exist on the surviving replica. V1 acknowledges this trade-off explicitly until Raft consensus is introduced.

---

## 15. Node Lifecycle

Nodes move through states:

\`\`\`
CREATED
   ↓
JOINING
   ↓
SYNCING
   ↓
ACTIVE
   │
   ├──────→ LEAVING
   │
   └──────→ SUSPECTED
                │
                ▼
              FAILED
\`\`\`

A previously unavailable node may return:
\`\`\`
FAILED/OFFLINE
       ↓
SYNCING
       ↓
ACTIVE
\`\`\`

---

## 16. Node Joining

Conceptual flow:

\`\`\`
Existing Node
     │
     ▼
Join Invitation
     │
     ▼
QR / Token / Link
     │
     ▼
New Device
     │
     ▼
Authenticate
     │
     ▼
Discover Cluster
     ▼
Download Application
     │
     ▼
Synchronize State
     │
     ▼
ACTIVE
\`\`\`

---

## 17. Node Leaving

Controlled departure:
1. Stop accepting new responsibilities
2. Complete or transfer required replication work
3. Notify the cluster
4. Persist required local state
5. Leave the active membership

Unexpected disappearance is handled through failure detection.

---

## 18. Failure Handling

NEBENK must account for:
- Process crashes
- Device shutdown/restart
- Network interruption
- Temporary disconnection
- Permanent node failure

A temporary network interruption must not automatically be interpreted as permanent device failure.

---

## 19. Availability Model

NEBENK does not guarantee availability under arbitrary failure conditions. Availability depends on:
- Number of replicas
- Current node availability
- Replication state
- Consistency configuration
- Network connectivity
- Application requirements

**Core Guarantee:** *"An eligible surviving node can recover the application from sufficiently current replicated state."*

---

## 20. Device Heterogeneity

Nodes may differ in CPU, memory, storage, architecture, network connectivity, battery, availability, background execution capabilities.

Future scheduling layer may classify nodes:
- **Desktop** → primary candidate
- **Laptop** → replica
- **Phone** → standby replica
- **Raspberry Pi** → persistent edge node

---

## 21. Mobile Devices

Android is an important target but imposes restrictions:
- Background execution
- Battery usage
- Network access
- Process lifetime

Mobile participation is conditional:
- **Unsuitable:** Battery 12%, not charging, background restricted
- **Suitable:** Battery 95%, charging, Wi-Fi, user enabled participation

---

## 22. Application Portability

Initial execution model: WebAssembly/WASI

\`\`\`
                 app.wasm
                    │
       ┌────────────┼────────────┐
       ▼            ▼            ▼
    Android       Linux       Windows
       │            │            │
    NEBENK        NEBENK       NEBENK
\`\`\`

The runtime itself may remain platform-specific.

---

## 23. Application Permissions

Explicit permission model:
- State storage
- Network access
- Filesystem access
- Clock
- Randomness
- Hardware access
- Camera, microphone, GPU

Initial implementation exposes minimum required capabilities.

---

## 24. Determinism

Replicated applications must avoid uncontrolled divergence sources:
- System time
- Random numbers
- Host-specific behavior
- Floating-point differences
- External APIs
- Hardware state

NEBENK provides controlled interfaces for deterministic behavior.

---

## 25. External Side Effects

External effects (sending email, making payment, controlling hardware, calling external API) must be distinguished from replicated state transitions to prevent multiple replicas executing the same side effect.

Future architecture distinguishes:
\`\`\`
Committed State
      ↓
Effect Execution
      ↓
External System
\`\`\`

from:
\`\`\`
Command
      ↓
Deterministic State Transition
\`\`\`

---

## 26. Networking

P2P communication between nodes using:
- libp2p
- QUIC
- Encrypted peer connections
- Peer discovery
- NAT traversal where possible

Applications should not need to implement their own peer networking.

---

## 27. Identity

Each node has persistent cryptographic identity:
- Node ID
- Public Key
- Private Key

Identity does not depend solely on IP address, hostname, or MAC address, allowing devices to reconnect while retaining cluster identity.

---

## 28. Security

NEBENK provides:
- **Authentication:** Nodes prove their identities
- **Authorization:** Only authorized nodes may join a cluster
- **Encryption:** Peer communication must be encrypted
- **Revocation:** Admin/cluster mechanism can revoke a node
- **Isolation:** Applications should not receive arbitrary host privileges
- **Credential Protection:** Private keys and secrets use platform-appropriate secure storage

---

## 29. Threat Model (V1)

V1 considers:
- Unauthorized node joining
- Stolen join credentials
- Forged messages
- Replay attacks
- Compromised nodes
- Malicious applications
- Local state theft
- Network interception

V1 does not solve arbitrary Byzantine behavior.

---

## 30. Resource Management

Future resource dimensions:
- CPU, Memory, Storage, Network, Battery, Power state, Availability, Architecture

Applications may declare requirements:
\`\`\`
memory >= 128 MB
architecture = arm64
storage >= 1 GB
\`\`\`

---

## 31. Observability

NEBENK should provide:
- **Node status:** ACTIVE, SYNCING, OFFLINE, FAILED
- **Replication status:** revision, lag, last synchronization, replica count
- **Cluster status:** members, leaders/primary, membership changes
- **Application status:** version, state revision, runtime health, resource consumption
- CLI and machine-readable status output

---

## 32. Developer Experience

Developers work with:
- Application, State, Commands, Configuration

Not with:
- Raft, gossip, heartbeats, peer discovery, snapshot transfer, failure detection

---

## 33. Application Packaging

Initial artifact structure:
\`\`\`
my-app/
├── manifest.json
├── app.wasm
└── schema/
\`\`\`

Manifest describes application metadata and runtime requirements.

---

## 34. CLI Direction

Future CLI commands:
\`\`\`bash
nebenk init
nebenk build
nebenk run
nebenk deploy
nebenk cluster
nebenk join
nebenk leave
nebenk status
nebenk peers
nebenk snapshot
nebenk logs
\`\`\`

---

## 35. Example User Journey

1. **Create cluster:** \`nebenk cluster create home\`
2. **Deploy application:** \`nebenk deploy my-app\`
3. **Another device joins:** \`nebenk join <invite>\`
4. **Device synchronizes:** Snapshot + Missing Operations → Replica Ready
5. **Original device fails**
6. **Surviving replica continues** (per V1 consistency semantics)
7. **Original device returns:** Synchronize → Replica Ready

---

## 36. MVP

**Reference Application:** Distributed key-value store (PUT, GET, DELETE)

**MVP Infrastructure:**
- WASM application
- Node identity
- P2P networking
- Primary/replica replication
- Operation log
- Snapshots
- Node synchronization
- Failure recovery

---

## 37. MVP Demonstration (3-Node Canonical Demo)

| Step | Description |
|------|-------------|
| 1 | Node A starts as Primary |
| 2 | Node B joins as Replica |
| 3 | Write state: \`PUT name Muqtada\` |
| 4 | Node C joins, receives state |
| 5 | Node A fails |
| 6 | Node B/C continue (V1 semantics) |
| 7 | Node A returns, synchronizes, becomes Replica |

---

## 38. Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-001 | Create a node identity | MUST |
| FR-002 | Create a cluster | MUST |
| FR-003 | Join an existing cluster | MUST |
| FR-004 | Leave a cluster | MUST |
| FR-005 | Authenticate participating nodes | MUST |
| FR-006 | Execute a portable application | MUST |
| FR-007 | Persist application state | MUST |
| FR-008 | Record state-changing operations | MUST |
| FR-009 | Create and restore snapshots | MUST |
| FR-010 | Replicate application state | MUST |
| FR-011 | Synchronize a new node | MUST |
| FR-012 | Detect node failure | MUST |
| FR-013 | Recover state after node failure | MUST |
| FR-014 | Expose cluster/node status | SHOULD |
| FR-015 | QR-based joining | SHOULD |
| FR-016 | Device capability reporting | SHOULD |
| FR-017 | Raft-based consensus | FUTURE |
| FR-018 | CRDT/multi-writer replication | FUTURE |
| FR-019 | OCI execution backend | FUTURE |
| FR-020 | Advanced scheduling | FUTURE |

---

## 39. Non-Functional Requirements

| ID | Requirement | Description |
|----|-------------|-------------|
| NFR-001 | Portability | Target multiple OS and architectures |
| NFR-002 | Resource Efficiency | Minimize idle consumption, especially mobile |
| NFR-003 | Reliability | Safe recovery from process/device restarts |
| NFR-004 | Security | Modern cryptographic mechanisms |
| NFR-005 | Isolation | Sandboxed application execution |
| NFR-006 | Observability | Basic cluster health and sync status |
| NFR-007 | Recoverability | Reconstruct state from persisted recovery info |
| NFR-008 | Compatibility | Consistent behavior across platforms |

---

## 40. Success Metrics

**Developer Experience:** Time to create cluster, add node, deploy app, infrastructure code required  
**Runtime:** Memory/CPU usage, network/storage overhead, startup time  
**Replication:** Latency, sync throughput, snapshot transfer time, replica lag  
**Recovery:** Failure detection time, recovery time, state recovery success rate, lost operations  
**Mobile:** Battery consumption, background survival, thermal impact

---

## 41. Definition of Done — MVP

NEBENK MVP is complete when:

- [ ] Two nodes can create a cluster
- [ ] Nodes have persistent identities
- [ ] Nodes can authenticate each other
- [ ] A WASM application can execute
- [ ] Application state can be persisted
- [ ] State-changing operations are logged
- [ ] Snapshots can be created
- [ ] A second node can synchronize state
- [ ] Replicas can recover after restart
- [ ] Node failure can be detected
- [ ] Application state can be recovered on a surviving node
- [ ] A new node can synchronize from an existing replica
- [ ] Basic cluster status can be inspected
- [ ] The reference KV application works end-to-end
- [ ] Tests cover crash/restart and synchronization scenarios
- [ ] V1 consistency limitations are documented

---

## 42. Testing Strategy

**Unit Tests:** State transitions, serialization, snapshots, log handling, identity, configuration  
**Integration Tests:** Node joining, replication, synchronization, restart, application deployment  
**Fault Injection:** Kill node, disconnect network, restart node, delay/drop messages, corrupt local state  
**Cross-Platform Testing:** Test across supported runtime environments

---

## 43. Major Risks

| Risk | Mitigation |
|------|------------|
| R1: Distributed State Complexity | Start with deterministic, single-writer model |
| R2: Data Loss During Failover | Document semantics, introduce quorum later |
| R3: Split Brain | Explicit consistency rules, eventual consensus |
| R4: Mobile Restrictions | Treat mobile availability as conditional |
| R5: NAT Traversal | P2P NAT traversal and relay mechanisms |
| R6: Determinism | Restrict and control nondeterministic APIs |
| R7: External Side Effects | Separate state transitions from side effects |
| R8: Large State | Incremental sync, snapshot chunking, future CAS |
| R9: Scope Explosion | Maintain strict MVP boundary |

---

## 44. Open Questions

1. What is the exact application API?
2. What constitutes an acknowledged operation in V1?
3. When is an operation considered durable?
4. How is primary failure handled?
5. How is split brain prevented?
6. How is node membership persisted?
7. What exact protocol is used for synchronization?
8. What is the snapshot format?
9. How are state schema migrations handled?
10. How are application upgrades coordinated?
11. How are external side effects represented?
12. How are deterministic time and randomness exposed?
13. How should floating-point computation be handled across architectures?
14. What minimum hardware should be supported?
15. How should Android background execution be integrated?
16. Should browsers eventually become NEBENK nodes?
17. How should large snapshots be transferred?
18. What level of P2P networking should be exposed to applications?
19. How should secrets be managed?
20. How should revoked nodes be prevented from reconnecting?

---

## 45. Future Roadmap

| Phase | Focus |
|-------|-------|
| **Phase 0** | Architecture, PRD, Technical Design, Protocol Definition |
| **Phase 1** | Single Node: Runtime, WASM, State Machine, Local Persistence, Snapshots, CLI |
| **Phase 2** | Two Nodes: Identity, P2P, Operation Replication, Synchronization |
| **Phase 3** | Failure Recovery: Heartbeats, Failure Detection, Recovery, Replica Promotion |
| **Phase 4** | Dynamic Membership: Join, Leave, Rejoin, Revocation, Membership Sync |
| **Phase 5** | Android: Runtime, Lifecycle, Battery Awareness, Foreground Service, QR Join |
| **Phase 6** | Strong Consensus: Raft, Quorum Commitment, Stronger Failover |
| **Phase 7** | Eventual Consistency: CRDT, Multi-writer, Offline-first |
| **Phase 8** | Advanced Execution: OCI, Native, GPU, Specialized Hardware, Scheduling |

---

## 46. Technology Direction

| Layer | Planned Technology |
|-------|-------------------|
| Runtime | Rust |
| Async | Tokio |
| Application Execution | WebAssembly / WASI |
| WASM Runtime | Wasmtime |
| P2P | libp2p |
| Transport | QUIC |
| Local Storage | SQLite |
| Serialization | CBOR |
| CLI | Rust |

*Technologies are provisional and may change during implementation.*

---

## 47. Containerization

NEBENK is not based on containerization:

| Container | NEBENK |
|-----------|--------|
| Package + isolate an application | Run + persist + replicate + recover application state |

Future: NEBENK Runtime with WASM and OCI backends.

---

## 48. Competitive Positioning

NEBENK combines selected capabilities at the application runtime layer:

\`\`\`
Portable Execution
        +
Portable State
        +
Replication
        +
Dynamic Membership
        +
P2P Networking
        +
Device Awareness
\`\`\`

Overlaps with: Containers, Kubernetes, Raft, CRDTs, P2P frameworks, Databases — but replaces none directly.

---

## 49. Project Structure

\`\`\`
nebenk/
├── README.md
├── LICENSE
├── Cargo.toml
│
├── docs/
│   ├── PRD.md
│   └── TECHNICAL_DESIGN.md
│
├── crates/
│   ├── nebenk-core/
│   ├── nebenk-runtime/
│   ├── nebenk-network/
│   ├── nebenk-membership/
│   ├── nebenk-replication/
│   ├── nebenk-storage/
│   ├── nebenk-identity/
│   └── nebenk-wasm/
│
├── sdk/
│   └── wasm/
│
├── examples/
│   ├── counter/
│   └── kv/
│
├── android/
│
└── tests/
\`\`\`

---

## 50. Glossary

| Term | Definition |
|------|------------|
| **Application** | The software being executed by NEBENK |
| **Node** | A device running the NEBENK runtime |
| **Cluster** | A logical group of participating nodes |
| **State** | The logical data representing the current application condition |
| **Command** | An input that causes a state transition |
| **Operation Log** | An ordered record of state-changing operations |
| **Snapshot** | A persisted representation of application state at a particular revision |
| **Replica** | A node maintaining recoverable application state |
| **Primary** | The node accepting state-changing operations in V1 |
| **Failover** | Continuing application using another eligible node after primary failure |
| **Consensus** | Mechanism allowing distributed nodes to agree on shared ordering/commitment |
| **WASM** | WebAssembly, the initial portable application execution target |

---

## 51. Product Philosophy

NEBENK is built around a deliberately simple idea:

> **«Nebeng across all your devices.»**

The device should not be the permanent home of the application.

\`\`\`
          DEVICE A
             │
             │
             ▼
        ┌───────────┐
        │  NEBENK   │
        └─────┬─────┘
              │
          Application
              │
        Portable State
              │
      ┌───────┼───────┐
      ▼       ▼       ▼
   Device A Device B Device C
\`\`\`

The long-term goal is not simply to make another deployment tool. It is to make the device an implementation detail of where an application happens to be running.

---

## 52. Final Product Definition

> **«NEBENK is a lightweight distributed application runtime that enables portable stateful applications to run across dynamically available heterogeneous devices, with replicated state allowing surviving nodes to recover applications after individual device failures.»**

The MVP will prove this concept with:
- WASM application
- Two or more devices
- P2P communication
- State replication
- Snapshots
- Failure recovery

Everything beyond that is an incremental expansion of the same core idea.
`;

  return (
    <DocsLayout>
      <MarkdownRenderer content={content} />
    </DocsLayout>
  );
}
