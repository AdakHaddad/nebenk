'use client';

import { DocsLayout } from '@/app/docs-layout';
import { MarkdownRenderer } from '@/components/MarkdownRenderer';

export default function ArchitecturePage() {
  const content = `
# Architecture

## Overview

NEBENK is a distributed runtime that lets applications run across different devices (phone, laptop, server) and survive individual device failures. It provides:

- **Cluster membership** with Phi Accrual Failure Detector for adaptive failure detection
- **Leader election** and automatic failover
- **Replicated state machine** with write-ahead logging and snapshots
- **WASM runtime** for portable application execution
- **P2P networking** via libp2p with gossip protocol

## Architecture Diagram

\`\`\`
┌─────────────────────────────────────────────────────────────┐
│                      Node Runtime                           │
├─────────────┬─────────────┬─────────────┬───────────────────┤
│  Membership │  Replication │  Storage    │   WASM Engine    │
│  (failure   │  (sync +     │  (SQLite    │   (Native or     │
│   detector) │   live op)   │   WAL)      │    WASM)         │
└──────┬──────┴──────┬───────┴──────┬──────┴────────┬─────────┘
       │             │             │              │
       ▼             ▼             ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│                    libp2p Network                           │
│              (Gossip / Heartbeat / Sync)                    │
└─────────────────────────────────────────────────────────────┘
\`\`\`

## Workspace Structure

\`\`\`
nebenk/
├── Cargo.toml                 # Workspace config
├── crates/
│   ├── nebenk-core/           # Core traits, types, errors
│   ├── nebenk-identity/       # Ed25519 keys, signing, invitations
│   ├── nebenk-storage/        # SQLite WAL + snapshots
│   ├── nebenk-wasm/           # WASM runtime + native KV engine
│   ├── nebenk-network/        # libp2p networking, gossip
│   ├── nebenk-membership/     # Cluster membership, failure detector
│   ├── nebenk-replication/    # Sync + live replication
│   ├── nebenk-runtime/        # Integrated node runtime
│   └── nebenk-cli/            # Command-line interface
└── target/
    └── release/
        └── nebenk             # Built binary
\`\`\`

## Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| \`nebenk-core\` | Shared traits, types, error definitions |
| \`nebenk-identity\` | Cryptographic identity, node keys, cluster invitations |
| \`nebenk-storage\` | SQLite-backed WAL and snapshot storage |
| \`nebenk-wasm\` | WASM runtime (wasmtime) + native KV engine for testing |
| \`nebenk-network\` | libp2p transport, gossip, heartbeat protocols |
| \`nebenk-membership\` | Cluster membership, Phi Accrual FD, leader election |
| \`nebenk-replication\` | State sync (snapshot + replay) and live replication |
| \`nebenk-runtime\` | Composed node runtime integrating all components |
| \`nebenk-cli\` | User-facing CLI commands |

## Core Traits

### StateEngine - Application execution engine

\`\`\`rust
pub trait StateEngine: Send {
    fn init(&mut self, config: &[u8]) -> Result<()>;
    fn apply(&mut self, op: &OperationEnvelope) -> Result<ApplyResult>;
    fn snapshot(&mut self) -> Result<Vec<u8>>;
    fn restore(&mut self, revision: u64, snapshot_data: &[u8]) -> Result<()>;
    fn query(&self, query_payload: &[u8]) -> Result<Vec<u8>>;
    fn current_revision(&self) -> u64;
}
\`\`\`

### LogStore - Write-ahead log

\`\`\`rust
pub trait LogStore: Send {
    fn append_operation(&mut self, op: &OperationEnvelope) -> Result<()>;
    fn get_operation(&self, revision: u64) -> Result<Option<OperationEnvelope>>;
    fn get_operations_since(&self, revision: u64, limit: usize) -> Result<Vec<OperationEnvelope>>;
    fn latest_revision(&self) -> Result<u64>;
    fn truncate_prefix(&mut self, up_to_revision: u64) -> Result<()>;
}
\`\`\`

### SnapshotStore - State snapshots

\`\`\`rust
pub trait SnapshotStore: Send {
    fn save_snapshot(&mut self, snapshot: &Snapshot) -> Result<()>;
    fn get_latest_snapshot(&self) -> Result<Option<Snapshot>>;
    fn get_snapshot_at(&self, revision: u64) -> Result<Option<Snapshot>>;
}
\`\`\`

## Runtime API

\`\`\`rust
use nebenk_runtime::{NodeRuntime, NodeConfig};
use nebenk_identity::NodeIdentity;
use nebenk_storage::SqliteStore;
use nebenk_wasm::NativeKvEngine;
use nebenk_core::types::ClusterId;

let identity = NodeIdentity::generate();
let cluster = ClusterId::new("my-cluster");
let storage = SqliteStore::open_in_memory(cluster.clone())?;
let engine = Box::new(NativeKvEngine::new());

let config = NodeConfig {
    cluster_id: cluster,
    is_primary: true,
    snapshot_interval: 100,
    heartbeat_interval_ms: 1000,
};

let mut runtime = NodeRuntime::new(identity, config, storage, engine);
runtime.recover().await?;

// Submit commands
let result = runtime.submit_command(payload, timestamp).await?;

// Query state
let response = runtime.query(&query_bytes).await?;

// Create snapshot
let snapshot = runtime.create_snapshot(timestamp).await?;
\`\`\`

## Membership API

\`\`\`rust
use nebenk_membership::Membership;
use nebenk_core::types::{ClusterId, NodeId};

let (membership, mut events) = Membership::new(cluster_id, local_node_id, 1000);

// Add peer
membership.upsert_member(peer_id, public_key, NodeRole::Replica, None).await?;

// Mark peer as synced
membership.mark_synced(&peer_id, revision).await?;

// Record heartbeat
membership.record_heartbeat(peer_id, timestamp).await?;

// Run failure detection
let events = membership.run_failure_detection().await;

// Handle failover
if let Some(new_primary) = membership.handle_primary_failure().await? {
    println!("New primary elected: {}", new_primary);
}

// Listen for membership events
while let Some(event) = events.recv().await {
    match event {
        MembershipEvent::NodeJoined(id) => println!("Node joined: {}", id),
        MembershipEvent::NodeFailed(id) => println!("Node failed: {}", id),
        MembershipEvent::PrimaryChanged(id) => println!("New primary: {}", id),
        _ => {}
    }
}
\`\`\`

## Replication Model

### V1: Primary/Replica Architecture

\`\`\`
             Primary
                │
          ┌─────┴─────┐
          ▼           ▼
       Replica A   Replica B
\`\`\`

- Primary accepts writes, replicates to replicas asynchronously
- Replicas maintain recoverable copies of application state
- Failover promotes highest-revision replica when primary fails

### Sync Replication (Catch-up)

When a replica connects or reconnects:
1. Replica sends \`SyncRequest { last_known_rev }\`
2. Primary inspects available range:
   - **Log replay**: If \`last_known_rev >= lowest_available_log_rev\`, stream missing operations
   - **Snapshot + Log**: If behind, send snapshot then remaining operations
3. Replica verifies BLAKE3 hash, restores WASM state, applies operations, transitions to ACTIVE

## Failure Detection

### Phi Accrual Failure Detector

Based on "The Phi Accrual Failure Detector" by Hayashibara et al.:

- Heartbeats every **1,000 ms**
- Maintains sliding window of inter-arrival times
- Calculates φ (phi) = -log₁₀(1 - CDF(time_since_last_heartbeat))
- **Suspicion threshold**: φ ≥ 8.0
- **Failure threshold**: φ ≥ 16.0

### Node States

- **Active**: Healthy, responding to heartbeats
- **Suspected**: φ ≥ 8.0, no heartbeat for ~3s
- **Failed**: φ ≥ 16.0, no heartbeat for ~7s
- **Syncing**: Catching up state
- **Leaving**: Graceful departure

## Storage Schema (SQLite)

\`\`\`sql
-- Append-only operation log
CREATE TABLE operation_log (
    revision INTEGER PRIMARY KEY,
    author_node_id BLOB NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    payload BLOB NOT NULL,
    signature BLOB NOT NULL
);

-- Snapshots index
CREATE TABLE snapshots (
    revision INTEGER PRIMARY KEY,
    timestamp_ms INTEGER NOT NULL,
    state_hash BLOB NOT NULL,
    compressed_data BLOB NOT NULL
);

-- Known cluster members
CREATE TABLE cluster_members (
    node_id BLOB PRIMARY KEY,
    public_key BLOB NOT NULL,
    role TEXT NOT NULL,          -- 'primary', 'replica', 'observer'
    status TEXT NOT NULL,        -- 'active', 'syncing', 'suspected', 'left'
    last_seen_ms INTEGER NOT NULL,
    last_synced_rev INTEGER NOT NULL DEFAULT 0
);
\`\`\`

## Cryptographic Identity

- **Ed25519** keypair per node (generated on \`nebenk init\`)
- **NodeId**: Base58-encoded 32-byte public key
- **Private key**: Stored locally with restricted permissions (0600)
- **Cluster invitations**: Signed tokens with expiry for joining

## P2P Networking

- **libp2p** with QUIC transport (TLS 1.3)
- **Protocols**:
  - \`/nebenk/id/1.0.0\`: Identity handshake
  - \`/nebenk/heartbeat/1.0.0\`: Liveness & membership gossip
  - \`/nebenk/rep/1.0.0\`: Live operation streaming
  - \`/nebenk/sync/1.0.0\`: Catch-up & snapshot transfer
- **Discovery**: mDNS for local, direct dial for remote

## WASM Host ABI

Guest modules export:

\`\`\`c
int32_t nebenk_alloc(int32_t size);
void nebenk_free(int32_t ptr, int32_t size);
int32_t nebenk_init(int32_t config_ptr, int32_t config_len);
int32_t nebenk_apply(int32_t op_ptr, int32_t op_len, int64_t revision, int64_t timestamp_ms);
int32_t nebenk_snapshot(int32_t out_ptr_buf);
int32_t nebenk_restore(int32_t snapshot_ptr, int32_t snapshot_len);
int32_t nebenk_query(int32_t query_ptr, int32_t query_len);
\`\`\`

### Determinism Constraints

1. No direct host clock access - time supplied via OperationEnvelope
2. No arbitrary randomness - deterministic seed from OperationEnvelope
3. No direct I/O/network - all persistence through state updates
4. IEEE 754 canonical NaN handling for floats

## Testing

\`\`\`bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p nebenk-membership
cargo test -p nebenk-runtime

# Run release mode tests
cargo test --release
\`\`\`

## Benchmarks

\`\`\`bash
cargo bench -p nebenk-runtime
\`\`\`

Benchmarks include:
- Single node PUT throughput
- Snapshot creation
- Recovery from WAL
- Concurrent queries
- Membership failure detection (100 nodes)
`;

  return (
    <DocsLayout>
      <MarkdownRenderer content={content} />
    </DocsLayout>
  );
}
