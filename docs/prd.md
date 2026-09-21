NEBENK — Product Requirements Document

«Nebeng across all your devices.»

Status: Pre-implementation
Version: 0.4
Last Updated: 2026-09-22
License: Apache License 2.0

---

1. Overview

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

«The application survives. The device is disposable.»

Instead of requiring an application to permanently belong to one server, NEBENK treats available devices as interchangeable infrastructure.

                    Application
                         │
                  Portable State
                         │
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
        Phone          Laptop        Server
        Node A         Node B        Node C

NEBENK is currently a design-stage project. This document defines the intended product and the requirements for its first implementation.

---

2. Problem

Modern application deployment commonly assumes relatively stable infrastructure:

Application
     ↓
Server
     ↓
Database
     ↓
Persistent infrastructure

High availability generally requires additional infrastructure such as:

- multiple servers
- replicated databases
- service discovery
- load balancing
- orchestration
- dedicated networking

This is appropriate for many production systems, but it can be excessive for smaller or more heterogeneous environments.

At the same time, users increasingly have multiple capable devices:

Phone
Laptop
Desktop
Raspberry Pi
Old PC
Server

These devices may have spare resources but are not normally treated as a unified infrastructure.

There is currently no simple application-level abstraction that allows a developer to say:

«“Run this stateful application wherever my available devices can host it, and allow another device to recover its state when one disappears.”»

NEBENK aims to provide that abstraction.

---

3. Vision

NEBENK aims to make distributed infrastructure feel like a property of the application rather than a collection of infrastructure systems the developer must manually operate.

Traditional thinking:

«Where does my application run?»

NEBENK:

«Which devices are currently available to run it?»

The desired experience is:

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

---

4. Product Thesis

NEBENK is based on several principles.

4.1 Application ≠ Device

An application should not fundamentally belong to one machine.

4.2 Application ≠ Process

The current process and its RAM are not the durable source of truth.

4.3 Application = Portable State Machine

The application exposes deterministic state transitions that can be persisted and reconstructed.

4.4 Devices Are Temporary

Devices can:

- join
- leave
- disconnect
- reconnect
- crash
- be replaced

without requiring manual restoration of application state.

4.5 Distributed Complexity Belongs in the Runtime

The developer should not need to implement:

- peer discovery
- replication
- state snapshots
- node identity
- failure detection
- state synchronization

for every application.

---

5. Goals

G1 — Portable Application Execution

An application should be executable on different supported operating systems and CPU architectures without requiring a separate application implementation for every device.

G2 — Portable Application State

Application state should be recoverable on another device.

G3 — Dynamic Device Participation

Devices should be able to join and leave a cluster without rebuilding the application.

G4 — Fault Recovery

The system should allow an eligible replica to recover application state following node failure.

G5 — Lightweight Runtime

The runtime should be significantly lighter than requiring a complete VM or traditional container stack on every participating device.

G6 — Device Agnosticism

Applications should not need to know whether they are running on:

- Android
- Linux
- Windows
- macOS
- ARM
- x86
- another supported platform

G7 — Developer Simplicity

The developer should define application behavior and state while NEBENK handles distributed infrastructure.

G8 — Local-First Operation

The system should be useful on local networks without requiring a permanent cloud service.

G9 — Open Infrastructure

NEBENK should be open-source and usable as a foundation for independent projects, research, commercial software, and community infrastructure.

---

6. Non-Goals

The following are explicitly outside the initial scope.

6.1 Arbitrary Process Migration

NEBENK will not attempt to transparently migrate:

- arbitrary process memory
- CPU registers
- thread stacks
- operating-system state
- arbitrary running processes

6.2 Full Virtual Machine

NEBENK is not intended to virtualize an entire operating system.

6.3 Kubernetes Replacement

NEBENK is not initially intended to provide:

- large-scale container orchestration
- cloud-scale scheduling
- service mesh infrastructure
- enterprise cluster management

6.4 Docker Replacement

NEBENK does not attempt to reproduce Docker's entire feature set.

Container/OCI execution may become an optional backend in the future.

6.5 Universal Distributed Database

NEBENK is not itself a general-purpose database.

Applications may use state storage provided by the runtime, but database functionality is not the primary product.

6.6 Byzantine Fault Tolerance

Malicious or actively Byzantine nodes are outside the initial trust model.

6.7 Guaranteed Availability With No Nodes

If all eligible nodes are offline, NEBENK cannot provide service availability.

---

7. Target Users

7.1 Independent Developers

Developers who want lightweight distributed deployment without maintaining permanent infrastructure.

7.2 Distributed Systems Developers

Developers experimenting with:

- replication
- P2P systems
- fault tolerance
- edge computing
- decentralized infrastructure

7.3 Edge and IoT Developers

Developers operating collections of:

- Raspberry Pis
- Android devices
- embedded computers
- local edge nodes

7.4 Small Organizations

Organizations that have several computers but do not want to maintain a dedicated server for every service.

7.5 Researchers and Students

NEBENK can provide an accessible environment for experimenting with distributed systems using ordinary devices.

---

8. Primary Use Cases

8.1 Personal Infrastructure

A user has:

Desktop
Laptop
Phone
Raspberry Pi

and wants a personal application to survive the loss of any individual device.

8.2 Small Office

Several existing office machines participate in hosting a service.

8.3 Local/Edge Infrastructure

Devices in the same physical environment cooperate without relying entirely on cloud infrastructure.

8.4 Temporary Cluster

Users create a temporary cluster for an event, laboratory, classroom, field operation, or local network.

8.5 Development and Testing

Developers use multiple personal devices to test distributed applications.

---

9. Core Concepts

NEBENK has several fundamental entities.

9.1 Application

The software being executed.

Example:

counter

9.2 Application Artifact

A portable representation of an application.

The initial target is a WebAssembly-based artifact.

9.3 Node

A device running the NEBENK runtime.

Node A
Node B
Node C

9.4 Cluster

A logical group of authorized nodes participating in an application.

9.5 State

The durable logical state of the application.

9.6 Operation

A state-changing command applied to the application.

9.7 Snapshot

A persisted representation of application state at a particular revision.

9.8 Replica

A node maintaining a recoverable copy of application state.

---

10. Product Architecture

At a conceptual level:

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

The exact implementation architecture will be defined separately in "TECHNICAL_DESIGN.md".

---

11. Application Model

The initial application model is a deterministic state machine.

Conceptually:

State + Command → New State

For example:

State:
counter = 10

Command:
INCREMENT 5

Result:
counter = 15

The same command applied to the same state should produce the same result.

This allows multiple nodes to reconstruct equivalent state.

---

12. State Durability Model

NEBENK will represent durable application state through:

Snapshot
+
Operation Log

Conceptually:

Snapshot #100
     +
Operation #101
     +
Operation #102
     +
Operation #103
     ↓
Current State

A new node should not necessarily need the entire historical log.

It may instead receive:

Recent Snapshot
+
Missing Operations

---

13. Replication Model

V1

The initial replication model will use a primary/replica architecture.

             Primary
                │
          ┌─────┴─────┐
          ▼           ▼
       Replica A   Replica B

The primary is responsible for accepting state-changing operations according to the V1 consistency model.

Replicas maintain recoverable copies of application state.

V2

A consensus-backed replication mechanism such as Raft may be introduced for applications requiring stronger consistency and quorum-based commitment.

V3+

Other models, including eventual consistency and CRDT-based multi-writer replication, may be supported where appropriate.

---

14. V1 Consistency Boundary

V1 must explicitly distinguish replicated state from quorum-committed state.

An operation may be accepted by a primary before every replica has persisted it.

Therefore:

Client
  ↓
Primary
  ↓
Local commit
  ↓
Replication
  X
Replica unavailable

If the primary fails before replication completes, the latest operation may not exist on the surviving replica.

V1 must document this behavior clearly.

NEBENK must not claim zero-data-loss failover until a replication mechanism providing the necessary commitment guarantees is implemented.

---

15. Node Lifecycle

Nodes may move through states such as:

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

A previously unavailable node may return:

FAILED/OFFLINE
       ↓
SYNCING
       ↓
ACTIVE

---

16. Node Joining

A new device should be able to join an existing cluster without manual configuration of every subsystem.

The conceptual flow is:

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
     │
     ▼
Download Application
     │
     ▼
Synchronize State
     │
     ▼
ACTIVE

The exact join protocol will be defined in the technical design.

---

17. Node Leaving

A node performing a controlled departure should:

1. Stop accepting new responsibilities.
2. Complete or transfer required replication work.
3. Notify the cluster.
4. Persist required local state.
5. Leave the active membership.

Unexpected disappearance is handled separately through failure detection.

---

18. Failure Handling

NEBENK must account for:

- process crashes
- device shutdown
- device restart
- network interruption
- temporary disconnection
- permanent node failure

A temporary network interruption must not automatically be interpreted as permanent device failure.

---

19. Availability Model

NEBENK does not guarantee that an application remains available under arbitrary failure conditions.

Availability depends on:

- number of replicas
- current node availability
- replication state
- consistency configuration
- network connectivity
- application requirements

The core guarantee is:

«An eligible surviving node can recover the application from sufficiently current replicated state.»

---

20. Device Heterogeneity

Nodes may differ significantly in:

- CPU
- memory
- storage
- architecture
- network connectivity
- battery
- availability
- background execution capabilities

NEBENK must therefore avoid assuming that every node behaves like a traditional server.

A future scheduling layer may classify nodes according to their capabilities.

Example:

Desktop
→ primary candidate

Laptop
→ replica

Phone
→ standby replica

Raspberry Pi
→ persistent edge node

These are examples, not fixed roles.

---

21. Mobile Devices

Android is an important target.

However, mobile operating systems impose restrictions on:

- background execution
- battery usage
- network access
- process lifetime

NEBENK must therefore treat mobile participation as conditional.

For example:

Phone
Battery: 12%
Not charging
Background restricted

may be unsuitable for an always-on role.

Whereas:

Phone
Battery: 95%
Charging
Wi-Fi
User enabled participation

may be suitable as a replica.

---

22. Application Portability

The initial application execution model is planned around WebAssembly/WASI.

This allows the application layer to remain independent of the host operating system.

Conceptually:

                 app.wasm
                    │
       ┌────────────┼────────────┐
       ▼            ▼            ▼
    Android       Linux       Windows
       │            │            │
    NEBENK        NEBENK       NEBENK

The runtime itself may remain platform-specific.

---

23. Application Permissions

Applications should operate within an explicit permission model.

Potential permissions include:

- state storage
- network access
- filesystem access
- clock
- randomness
- hardware access
- camera
- microphone
- GPU

The initial implementation should expose only the minimum required capabilities.

This is especially important for deterministic replicated execution.

---

24. Determinism

A replicated application must avoid uncontrolled sources of divergence.

Potential sources include:

- system time
- random numbers
- host-specific behavior
- floating-point differences
- external APIs
- hardware state

NEBENK should provide controlled interfaces where deterministic behavior is required.

The application should conceptually behave as:

Apply(State, Command) → State'

rather than relying on uncontrolled host state.

---

25. External Side Effects

External effects such as:

- sending an email
- making a payment
- controlling hardware
- calling an external API

must be distinguished from replicated state transitions.

Otherwise, multiple replicas could execute the same side effect.

The future architecture should therefore distinguish:

Committed State
      ↓
Effect Execution
      ↓
External System

from:

Command
      ↓
Deterministic State Transition

---

26. Networking

NEBENK requires peer-to-peer communication between participating nodes.

The initial technical direction is:

- libp2p
- QUIC
- encrypted peer connections
- peer discovery
- NAT traversal where possible

Applications should not need to implement their own peer networking.

---

27. Identity

Each node should have a persistent cryptographic identity.

Conceptually:

Node Identity
├── Node ID
├── Public Key
└── Private Key

Identity should not depend solely on:

- IP address
- hostname
- MAC address

This allows devices to reconnect while retaining their cluster identity.

---

28. Security

NEBENK must provide:

Authentication

Nodes prove their identities.

Authorization

Only authorized nodes may join a cluster.

Encryption

Peer communication must be encrypted.

Revocation

An administrator or authorized cluster mechanism must be able to revoke a node.

Isolation

Applications should not receive arbitrary host privileges.

Credential Protection

Private keys and application secrets should use platform-appropriate secure storage where available.

---

29. Threat Model

V1 should consider:

- unauthorized node joining
- stolen join credentials
- forged messages
- replay attacks
- compromised nodes
- malicious applications
- local state theft
- network interception

V1 does not attempt to solve arbitrary Byzantine behavior.

---

30. Resource Management

NEBENK should eventually expose resource requirements and node capabilities.

Potential resource dimensions:

CPU
Memory
Storage
Network
Battery
Power state
Availability
Architecture

Applications may declare requirements such as:

memory >= 128 MB
architecture = arm64
storage >= 1 GB

Advanced scheduling is outside the initial MVP.

---

31. Observability

A distributed system must expose enough information to understand its state.

NEBENK should eventually provide:

Node status

ACTIVE
SYNCING
OFFLINE
FAILED

Replication status

revision
lag
last synchronization
replica count

Cluster status

members
leaders/primary
membership changes

Application status

version
state revision
runtime health
resource consumption

CLI and machine-readable status output should eventually be supported.

---

32. Developer Experience

NEBENK should minimize the distributed-systems knowledge required by application developers.

The developer should primarily work with:

Application
State
Commands
Configuration

rather than:

Raft
gossip
heartbeats
peer discovery
snapshot transfer
failure detection

---

33. Application Packaging

The initial application artifact is conceptually:

my-app/
├── manifest.json
├── app.wasm
└── schema/

The manifest should describe application metadata and runtime requirements.

The exact schema is a technical-design concern.

---

34. CLI Direction

A future CLI may provide commands such as:

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

CLI syntax is provisional.

---

35. Example User Journey

A user starts a cluster:

nebenk cluster create home

They deploy an application:

nebenk deploy my-app

Another device joins:

nebenk join <invite>

The device synchronizes:

Application
     ↓
Snapshot
     ↓
Missing Operations
     ↓
Replica Ready

The original device later fails.

The surviving replica can continue according to the configured V1 consistency and failover behavior.

---

36. MVP

The MVP should intentionally be narrow.

Reference application

A distributed key-value store.

Operations:

PUT key value
GET key
DELETE key

MVP infrastructure

WASM application
       +
Node identity
       +
P2P networking
       +
Primary/replica replication
       +
Operation log
       +
Snapshots
       +
Node synchronization
       +
Failure recovery

---

37. MVP Demonstration

The canonical demonstration should involve three nodes.

Step 1 — Node A

Node A
Primary

Step 2 — Node B joins

Node A ───── Node B
Primary      Replica

Step 3 — Write state

PUT name Muqtada

Step 4 — Node C joins

Node A ───── Node B
  │           │
  └── Node C ─┘

Node C receives the required application state.

Step 5 — Node A fails

Node A ✕

Node B
Node C

Step 6 — Remaining eligible node continues

The application remains recoverable according to the V1 replication semantics.

Step 7 — Node A returns

Node A
   ↓
Synchronize
   ↓
Replica Ready

This demonstrates the core NEBENK concept.

---

38. Functional Requirements

ID| Requirement| Priority
FR-001| Create a node identity| MUST
FR-002| Create a cluster| MUST
FR-003| Join an existing cluster| MUST
FR-004| Leave a cluster| MUST
FR-005| Authenticate participating nodes| MUST
FR-006| Execute a portable application| MUST
FR-007| Persist application state| MUST
FR-008| Record state-changing operations| MUST
FR-009| Create and restore snapshots| MUST
FR-010| Replicate application state| MUST
FR-011| Synchronize a new node| MUST
FR-012| Detect node failure| MUST
FR-013| Recover state after node failure| MUST
FR-014| Expose cluster/node status| SHOULD
FR-015| QR-based joining| SHOULD
FR-016| Device capability reporting| SHOULD
FR-017| Raft-based consensus| FUTURE
FR-018| CRDT/multi-writer replication| FUTURE
FR-019| OCI execution backend| FUTURE
FR-020| Advanced scheduling| FUTURE

---

39. Non-Functional Requirements

NFR-001 — Portability

The runtime should target multiple operating systems and architectures.

NFR-002 — Resource Efficiency

Idle runtime resource consumption should be measured and minimized, particularly on mobile devices.

NFR-003 — Reliability

The runtime must recover safely from process and device restarts.

NFR-004 — Security

Peer communication and node identity must use modern cryptographic mechanisms.

NFR-005 — Isolation

Application execution should be sandboxed.

NFR-006 — Observability

Operators must be able to determine basic cluster health and synchronization status.

NFR-007 — Recoverability

A node must be able to reconstruct application state from persisted recovery information.

NFR-008 — Compatibility

Application behavior should remain consistent across supported platforms.

---

40. Success Metrics

The initial project should measure:

Developer Experience

- Time to create a cluster
- Time to add a node
- Time to deploy an application
- Amount of distributed infrastructure code required from an application developer

Runtime

- Memory usage
- CPU usage
- Network overhead
- Storage overhead
- Startup time

Replication

- Replication latency
- Synchronization throughput
- Snapshot transfer time
- Replica lag

Recovery

- Failure detection time
- Recovery time
- State recovery success rate
- Number of lost acknowledged operations under documented V1 semantics

Mobile

- Battery consumption
- Background survival
- Thermal/resource impact

---

41. Definition of Done — MVP

NEBENK MVP is complete when:

- [ ] Two nodes can create a cluster.
- [ ] Nodes have persistent identities.
- [ ] Nodes can authenticate each other.
- [ ] A WASM application can execute.
- [ ] Application state can be persisted.
- [ ] State-changing operations are logged.
- [ ] Snapshots can be created.
- [ ] A second node can synchronize state.
- [ ] Replicas can recover after restart.
- [ ] Node failure can be detected.
- [ ] Application state can be recovered on a surviving node.
- [ ] A new node can synchronize from an existing replica.
- [ ] Basic cluster status can be inspected.
- [ ] The reference KV application works end-to-end.
- [ ] Tests cover crash/restart and synchronization scenarios.
- [ ] V1 consistency limitations are documented.

---

42. Testing Strategy

NEBENK requires failure-oriented testing.

Unit Tests

Test:

- state transitions
- serialization
- snapshots
- log handling
- identity
- configuration

Integration Tests

Test:

- node joining
- replication
- synchronization
- restart
- application deployment

Fault Injection

Test:

kill node
disconnect network
restart node
delay messages
drop messages
corrupt local process state

Cross-Platform Testing

Applications must be tested across supported runtime environments.

---

43. Major Risks

R1 — Distributed State Complexity

State replication introduces significant correctness challenges.

Mitigation: Start with a deterministic, single-writer model.

R2 — Data Loss During Failover

V1 replication may acknowledge operations before all replicas have persisted them.

Mitigation: Clearly document semantics and introduce quorum-based commitment later.

R3 — Split Brain

Network partitions can create conflicting node views.

Mitigation: Explicit consistency rules and eventual consensus support.

R4 — Mobile Restrictions

Android may suspend or terminate background workloads.

Mitigation: Treat mobile availability as conditional and resource-aware.

R5 — NAT Traversal

Some peers cannot directly connect.

Mitigation: P2P NAT traversal and relay mechanisms.

R6 — Determinism

Platform differences can cause replicas to diverge.

Mitigation: Restrict and control nondeterministic runtime APIs.

R7 — External Side Effects

Replicated execution can accidentally repeat external actions.

Mitigation: Separate state transitions from side-effect execution.

R8 — Large State

Large snapshots can make synchronization expensive.

Mitigation: Incremental synchronization, snapshot chunking, and future content-addressed storage.

R9 — Scope Explosion

The project could grow into a combination of:

Docker
+
Kubernetes
+
database
+
Raft
+
P2P framework
+
mobile platform

Mitigation: Maintain a strict MVP boundary.

---

44. Open Questions

The following questions must be resolved during technical design.

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

45. Future Roadmap

Phase 0 — Architecture

- Product requirements
- Technical design
- Architecture decisions
- Protocol definition

Phase 1 — Single Node

- Runtime
- WASM execution
- State machine
- Local persistence
- Snapshots
- CLI

Phase 2 — Two Nodes

- Node identity
- P2P networking
- Operation replication
- Synchronization

Phase 3 — Failure Recovery

- Heartbeats
- Failure detection
- Recovery
- Replica promotion

Phase 4 — Dynamic Membership

- Join
- Leave
- Rejoin
- Revocation
- Membership synchronization

Phase 5 — Android

- Android runtime
- Lifecycle integration
- Battery awareness
- Foreground service
- QR joining

Phase 6 — Strong Consensus

- Raft
- quorum commitment
- stronger failover guarantees

Phase 7 — Eventual Consistency

- CRDT
- multi-writer replication
- offline-first applications

Phase 8 — Advanced Execution

- OCI
- native execution
- GPU
- specialized hardware
- advanced scheduling

---

46. Technology Direction

The current implementation direction is:

Layer| Planned Technology
Runtime| Rust
Async| Tokio
Application execution| WebAssembly / WASI
WASM runtime| Wasmtime
P2P| libp2p
Transport| QUIC
Local storage| SQLite
Serialization| CBOR
CLI| Rust

These technologies are provisional and may change during implementation.

The PRD defines product requirements, not technology mandates.

---

47. Containerization

Containerization is deliberately not the foundation of NEBENK.

The distinction is:

Container:
Package + isolate an application

while:

NEBENK:
Run + persist + replicate + recover application state

NEBENK may eventually support:

NEBENK Runtime
      │
      ├── WASM
      │
      └── OCI
             ↓
        Linux container

This allows capable nodes to use existing container ecosystems without forcing containers onto mobile devices.

---

48. Competitive Positioning

NEBENK overlaps with several existing technology categories but is not intended to replace them directly.

Containers

Solve application packaging and isolation.

Kubernetes

Solves large-scale orchestration.

Raft

Solves distributed consensus.

CRDTs

Solve specific classes of eventually consistent replicated state.

P2P frameworks

Provide peer-to-peer networking primitives.

Databases

Provide persistent structured data management.

NEBENK aims to combine selected capabilities at the application runtime layer:

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

---

49. Project Structure

The repository is expected to evolve toward:

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
│   ├── nebenk-wasm/
│   └── nebenk-cli/
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

This structure is provisional and should not constrain the technical design prematurely.

---

50. Glossary

Application

The software being executed by NEBENK.

Node

A device running the NEBENK runtime.

Cluster

A logical group of participating nodes.

State

The logical data representing the current application condition.

Command

An input that causes a state transition.

Operation Log

An ordered record of state-changing operations.

Snapshot

A persisted representation of application state at a particular revision.

Replica

A node maintaining recoverable application state.

Primary

The node responsible for accepting state-changing operations in the V1 replication model.

Failover

The process of continuing application operation using another eligible node after primary failure.

Consensus

A mechanism allowing distributed nodes to agree on a shared ordering/commitment of operations.

WASM

WebAssembly, the initial portable application execution target.

---

51. Product Philosophy

NEBENK is built around a deliberately simple idea:

«Nebeng across all your devices.»

The device should not be the permanent home of the application.

A laptop can leave.

A phone can disappear.

A Raspberry Pi can reboot.

A server can fail.

The application should remain recoverable because its state is not tied exclusively to any one of them.

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

The long-term goal is not simply to make another deployment tool.

It is to make the device an implementation detail of where an application happens to be running.

---

52. Final Product Definition

«NEBENK is a lightweight distributed application runtime that enables portable stateful applications to run across dynamically available heterogeneous devices, with replicated state allowing surviving nodes to recover applications after individual device failures.»

The MVP will prove this concept with:

WASM application
       +
Two or more devices
       +
P2P communication
       +
State replication
       +
Snapshots
       +
Failure recovery

Everything beyond that is an incremental expansion of the same core idea.
