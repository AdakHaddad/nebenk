'use client';

import { DocsLayout } from '../docs-layout';
import { MarkdownRenderer } from '../../components/MarkdownRenderer';

export default function ApiPage() {
  const content = `
# API Reference

This document covers the programmatic APIs for building applications on NEBENK and operating a NEBENK cluster.

---

## Runtime API

The top-level NodeRuntime coordinates storage, WASM execution, networking, membership, and replication.

### Creating a Runtime

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
\`\`\`

### Submitting Commands

Commands are state-changing operations submitted to the application:

\`\`\`rust
use nebenk_core::types::OperationEnvelope;

// Submit a command to the runtime
let result = runtime.submit_command(command_payload, timestamp_ms).await?;

// Result contains:
// - revision: the new revision number
// - state_hash: BLAKE3 hash of the new state
// - output: application-specific response bytes
\`\`\`

### Querying State

Read-only queries execute against the current state:

\`\`\`rust
// Query the current application state
let response = runtime.query(&query_payload).await?;

// Returns application-specific response bytes
\`\`\`

### Creating Snapshots

\`\`\`rust
// Create a snapshot at the current revision
let snapshot = runtime.create_snapshot(timestamp_ms).await?;

// Snapshot contains:
// - revision: u64
// - state_hash: [u8; 32] (BLAKE3)
// - compressed_data: Vec<u8>
// - timestamp_ms: u64
\`\`\`

---

## NodeConfig

Configuration for node behavior:

\`\`\`rust
pub struct NodeConfig {
    pub cluster_id: ClusterId,        // Cluster identifier
    pub is_primary: bool,             // true = primary (accepts writes), false = replica
    pub snapshot_interval: u64,       // Snapshot every N operations (0 = disabled)
    pub heartbeat_interval_ms: u64,   // Heartbeat interval in milliseconds
}
\`\`\`

| Field | Description | Default |
|-------|-------------|---------|
| \`cluster_id\` | Cluster identifier | Required |
| \`is_primary\` | Run as primary node | \`false\` |
| \`snapshot_interval\` | Snapshot every N ops | \`50\` |
| \`heartbeat_interval_ms\` | Heartbeat interval | \`1000\` |

---

## Core Traits

### StateEngine - Application Execution Engine

Implemented by application engines (WASM runtime, native KV engine):

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

**Methods:**

| Method | Purpose |
|--------|---------|
| \`init\` | Initialize engine with configuration bytes |
| \`apply\` | Apply a state-changing operation |
| \`snapshot\` | Capture complete state snapshot |
| \`restore\` | Restore state from snapshot |
| \`query\` | Execute read-only query |
| \`current_revision\` | Get current revision number |

---

### LogStore - Write-Ahead Log

\`\`\`rust
pub trait LogStore: Send {
    fn append_operation(&mut self, op: &OperationEnvelope) -> Result<()>;
    fn get_operation(&self, revision: u64) -> Result<Option<OperationEnvelope>>;
    fn get_operations_since(&self, revision: u64, limit: usize) -> Result<Vec<OperationEnvelope>>;
    fn latest_revision(&self) -> Result<u64>;
    fn truncate_prefix(&mut self, up_to_revision: u64) -> Result<()>;
}
\`\`\`

---

### SnapshotStore - State Snapshots

\`\`\`rust
pub trait SnapshotStore: Send {
    fn save_snapshot(&mut self, snapshot: &Snapshot) -> Result<()>;
    fn get_latest_snapshot(&self) -> Result<Option<Snapshot>>;
    fn get_snapshot_at(&self, revision: u64) -> Result<Option<Snapshot>>;
}
\`\`\`

---

## Application Development (WASM)

### Building a WASM Application

Applications compile to \`wasm32-wasip1\` or bare \`wasm32-unknown-unknown\`:

\`\`\`toml
# Cargo.toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
\`\`\`

### Guest SDK Types

\`\`\`rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Put { key: String, value: Vec<u8> },
    Delete { key: String },
}

#[derive(Serialize, Deserialize)]
pub enum Query {
    Get { key: String },
    ListKeys,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Value(Option<Vec<u8>>),
    Keys(Vec<String>),
}
\`\`\`

### Host ABI Exports

Guest modules must export these C-ABI functions:

\`\`\`c
// Allocate a buffer in guest memory
int32_t nebenk_alloc(int32_t size);

// Free a previously allocated buffer
void nebenk_free(int32_t ptr, int32_t size);

// Initialize application state
int32_t nebenk_init(int32_t config_ptr, int32_t config_len);

// Apply a state-changing operation
int32_t nebenk_apply(
    int32_t op_ptr,
    int32_t op_len,
    int64_t revision,
    int64_t timestamp_ms
);

// Capture a complete snapshot of current state
int32_t nebenk_snapshot(int32_t out_ptr_buf);

// Restore application state from a serialized snapshot
int32_t nebenk_restore(int32_t snapshot_ptr, int32_t snapshot_len);

// Execute a read-only query against current state
int32_t nebenk_query(int32_t query_ptr, int32_t query_len);
\`\`\`

### Determinism Requirements

Guests must be completely deterministic:
1. **No direct host clock access** - time supplied via \`OperationEnvelope.timestamp\`
2. **No arbitrary randomness** - deterministic seed from \`OperationEnvelope\`
3. **No direct I/O/network** - all persistence through state updates
4. **IEEE 754 canonical NaN handling** for floats

---

## Membership API

Manage cluster membership and failure detection:

\`\`\`rust
use nebenk_membership::Membership;
use nebenk_core::types::{ClusterId, NodeId};

let (membership, mut events) = Membership::new(cluster_id, local_node_id, 1000);

// Add a peer to the cluster
membership.upsert_member(peer_id, public_key, NodeRole::Replica, None).await?;

// Mark peer as fully synced
membership.mark_synced(&peer_id, revision).await?;

// Record received heartbeat
membership.record_heartbeat(peer_id, timestamp).await?;

// Run failure detection cycle
let events = membership.run_failure_detection().await;

// Handle primary failure and elect new primary
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

### MembershipEvent Types

| Event | Description |
|-------|-------------|
| \`NodeJoined(NodeId)\` | New node joined cluster |
| \`NodeLeft(NodeId)\` | Node gracefully left |
| \`NodeFailed(NodeId)\` | Node marked as failed |
| \`NodeSuspected(NodeId)\` | Node suspected (φ ≥ 8.0) |
| \`NodeRecovered(NodeId)\` | Previously failed node recovered |
| \`PrimaryChanged(NodeId)\` | New primary elected |
| \`RoleChanged(NodeId, NodeRole)\` | Node role changed |

---

## Replication API

### Primary Side

\`\`\`rust
use nebenk_replication::ReplicationManager;

// Start replication to connected replicas
replication_manager.start_replication(primary_runtime).await;

// Stream operation to replicas
replication_manager.broadcast_operation(&operation).await;
\`\`\`

### Replica Side

\`\`\`rust
// Handle incoming replicated operations
replication_manager.handle_replicated_operation(&operation).await;

// Request state synchronization
let sync_request = SyncRequest { last_known_rev };
replication_manager.request_sync(sync_request).await;
\`\`\`

---

## Storage API

### SqliteStore

\`\`\`rust
use nebenk_storage::SqliteStore;
use nebenk_core::types::ClusterId;

// Open or create SQLite database
let store = SqliteStore::open(path, cluster_id)?;

// In-memory for testing
let store = SqliteStore::open_in_memory(cluster_id)?;
\`\`\`

Implements both \`LogStore\` and \`SnapshotStore\` traits.

---

## Identity API

### NodeIdentity

\`\`\`rust
use nebenk_identity::NodeIdentity;

// Generate new identity
let identity = NodeIdentity::generate();

// Load from file
let identity = NodeIdentity::load(key_path)?;

// Get NodeId (base58-encoded public key)
let node_id = identity.node_id();

// Sign data
let signature = identity.sign(&data);

// Verify signature
identity.verify(&data, &signature)?;
\`\`\`

### Cluster Invitations

\`\`\`rust
use nebenk_identity::InvitationToken;

// Create invitation (by cluster admin/primary)
let token = InvitationToken::new(
    cluster_id,
    expiry_epoch,
    peer_address,
    &admin_identity
)?;

// Parse invitation (by joining node)
let parsed = InvitationToken::parse(&token_string)?;

// Verify invitation signature
parsed.verify(&cluster_admin_public_key)?;
\`\`\`

Invitation format:
\`\`\`
nebenk-invite://<CLUSTER_ID>/<EXPIRY_EPOCH>/<PEER_ADDR>?sig=<ED25519_SIG>
\`\`\`

---

## Network API

### libp2p Protocols

| Protocol | Purpose |
|----------|---------|
| \`/nebenk/id/1.0.0\` | Identity handshake |
| \`/nebenk/heartbeat/1.0.0\` | Liveness & membership gossip |
| \`/nebenk/rep/1.0.0\` | Live operation streaming |
| \`/nebenk/sync/1.0.0\` | Catch-up & snapshot transfer |

### Creating a Swarm

\`\`\`rust
use nebenk_network::NetworkBuilder;
use libp2p::Swarm;

let swarm = NetworkBuilder::new()
    .with_quic()
    .with_mdns()
    .build()
    .await?;
\`\`\`

---

## CLI Commands

The \`nebenk\` CLI binary provides operator commands:

\`\`\`bash
# Initialize a new node
nebenk init --data-dir .nebenk --cluster my-cluster

# Start node as primary
nebenk --data-dir .nebenk --cluster my-cluster start --primary

# Start node as replica
nebenk --data-dir .nebenk --cluster my-cluster start --replica

# Write a key-value pair (primary only)
nebenk put mykey "hello world"

# Read a value
nebenk get mykey

# List all keys
nebenk list

# Check node status
nebenk status

# Force snapshot creation
nebenk snapshot
\`\`\`

### CLI Flags

| Flag | Description | Default |
|------|-------------|---------|
| \`--data-dir\` | Base directory for keys and storage | \`.nebenk\` |
| \`--cluster\` | Cluster identifier | \`nebenk-default\` |
| \`--primary\` | Run as primary node | \`false\` |
| \`--replica\` | Run as replica node | \`true\` |
| \`--port\` | libp2p listen port | auto |
| \`--snapshot-interval\` | Snapshot every N ops | \`50\` |

---

## CBOR Serialization

All wire messages, WAL records, and snapshots use CBOR (RFC 8949).

### OperationEnvelope

\`\`\`rust
pub struct OperationEnvelope {
    pub revision: u64,
    pub cluster_id: ClusterId,
    pub author_node: NodeId,
    pub timestamp_ms: u64,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>, // Signed by author or primary
}
\`\`\`

### SnapshotHeader

\`\`\`rust
pub struct SnapshotHeader {
    pub format_version: u32,
    pub cluster_id: ClusterId,
    pub revision: u64,
    pub state_hash: [u8; 32], // BLAKE3 hash of snapshot data
    pub timestamp_ms: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}
\`\`\`

---

## Error Types

\`\`\`rust
pub enum NebenkError {
    #[error("Storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] cbor4ii::Error),
    
    #[error("WASM execution error: {0}")]
    Wasm(#[from] wasmtime::Error),
    
    #[error("Network error: {0}")]
    Network(#[from] libp2p::Error),
    
    #[error("Identity error: {0}")]
    Identity(#[from] ed25519_dalek::SignatureError),
    
    #[error("Replication error: {0}")]
    Replication(String),
    
    #[error("Membership error: {0}")]
    Membership(String),
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    #[error("Not primary: cannot accept write operations")]
    NotPrimary,
    
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
}
\`\`\`

---

## Testing Utilities

### NativeKvEngine

For testing without WASM:

\`\`\`rust
use nebenk_wasm::NativeKvEngine;
use nebenk_core::traits::StateEngine;
use nebenk_core::types::OperationEnvelope;

let mut engine = NativeKvEngine::new();
engine.init(b"").unwrap();

// Apply operations
let op = OperationEnvelope::new(...);
let result = engine.apply(&op).unwrap();

// Query state
let query = b"{\\"Get\\":{\\"key\\":\\"mykey\\"}}";
let response = engine.query(query).unwrap();
\`\`\`

---

## Next Steps

- [Getting Started](/getting-started) - Full installation and usage guide
- [Architecture](/architecture) - Deep technical architecture
- [Product Requirements](/prd) - Product vision and requirements
`;

  return (
    <DocsLayout>
      <MarkdownRenderer content={content} />
    </DocsLayout>
  );
}
