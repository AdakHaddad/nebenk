# NEBENK

Lightweight distributed application runtime for running applications across multiple devices with replicated state.

> **The application survives. The device is disposable.**

## Overview

NEBENK is a distributed runtime that lets applications run across different devices (phone, laptop, server) and survive individual device failures. It provides:

- **Cluster membership** with Phi Accrual Failure Detector for adaptive failure detection
- **Leader election** and automatic failover
- **Replicated state machine** with write-ahead logging and snapshots
- **WASM runtime** for portable application execution
- **P2P networking** via libp2p with gossip protocol

## Architecture

```
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
```

## Quick Start

### Prerequisites

- **Rust** 1.75+ (`rustup install stable`)
- **SQLite3** development headers
- **OpenSSL** development headers (for libp2p TLS)

#### Ubuntu/Debian
```bash
sudo apt-get update && sudo apt-get install -y \
    libsqlite3-dev libssl-dev pkg-config
```

#### macOS
```bash
brew install sqlite3 openssl
```

#### Fedora/RHEL
```bash
sudo dnf install sqlite-devel openssl-devel pkg-config
```

### Build from Source

```bash
git clone https://github.com/AdakHaddad/nebenk
cd nebenk
cargo build --release
```

The binary will be at `target/release/nebenk`.

### Initialize a Node

```bash
# Initialize node identity and storage
nebenk init --data-dir .nebenk --cluster my-cluster

# Output:
# Node initialized successfully!
# Node ID:  12D3KooW...
# Key Path: .nebenk/node.key
# Cluster:  my-cluster
```

### Run as Primary (Leader)

```bash
# Start node as primary (accepts writes)
nebenk --data-dir .nebenk --cluster my-cluster start --primary
```

### Run as Replica

```bash
# Start node as replica (read-only, replicates from primary)
nebenk --data-dir .nebenk --cluster my-cluster start --replica
```

### Basic Operations

```bash
# Write a key-value pair (primary only)
nebenk put mykey "hello world"
# OK (revision: 1, state_hash: ...)

# Read a value
nebenk get mykey
# hello world

# List all keys
nebenk list
# mykey
# another-key

# Check node status
nebenk status
# NEBENK Node Status
# ==================
# Node ID:          12D3KooW...
# Cluster ID:       my-cluster
# Current Revision: 5
# Data Dir:         /path/to/.nebenk

# Force snapshot creation
nebenk snapshot
# Snapshot created successfully for revision 5 (size: 1024 bytes, hash: ...)
```

### Cluster Operations

```bash
# Create invitation token for another node to join
nebenk invite --expires-in 3600
# Invitation Token (valid for 3600s):
# base64-encoded-token...

# On the new node, join with invitation:
nebenk join --data-dir .nebenk --cluster my-cluster --invitation <token>
```

## Running a Cluster

### 3-Node Cluster Example

**Terminal 1 - Primary:**
```bash
nebenk --data-dir .nebenk/node1 --cluster demo start --primary --port 8001
```

**Terminal 2 - Replica 1:**
```bash
# Get invitation from primary first, then:
nebenk --data-dir .nebenk/node2 --cluster demo start --replica --port 8002
```

**Terminal 3 - Replica 2:**
```bash
nebenk --data-dir .nebenk/node3 --cluster demo start --replica --port 8003
```

### Test Failover

1. Write data on primary:
   ```bash
   nebenk --data-dir .nebenk/node1 put counter 1
   ```

2. Kill primary (Ctrl+C)

3. Replica automatically becomes new primary

4. Continue writing:
   ```bash
   nebenk --data-dir .nebenk/node2 put counter 2
   ```

## Configuration

### NodeConfig Options

```rust
NodeConfig {
    cluster_id: ClusterId,        // Cluster identifier
    is_primary: bool,             // true = primary (accepts writes), false = replica
    snapshot_interval: u64,       // Snapshot every N operations (0 = disabled)
    heartbeat_interval_ms: u64,   // Heartbeat interval in milliseconds
}
```

### CLI Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--data-dir` | Base directory for keys and storage | `.nebenk` |
| `--cluster` | Cluster identifier | `nebenk-default` |
| `--primary` | Run as primary node | false |
| `--replica` | Run as replica node | true |
| `--port` | libp2p listen port | auto |
| `--snapshot-interval` | Snapshot every N ops | 50 |

## Application Development

### Writing a WASM Application

Applications compile to WASM and run in the NEBENK runtime:

```rust
// Cargo.toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

```rust
// src/lib.rs
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

// Implement StateEngine trait for your app
```

### Native KV Engine (for testing)

The `nebenk-wasm` crate includes a native Rust KV engine for testing:

```rust
use nebenk_wasm::NativeKvEngine;
use nebenk_core::traits::StateEngine;
use nebenk_core::types::OperationEnvelope;

let mut engine = NativeKvEngine::new();
engine.init(b"").unwrap();

// Apply operations
let op = OperationEnvelope::new(...);
let result = engine.apply(&op).unwrap();

// Query state
let query = b"{\"Get\":{\"key\":\"mykey\"}}";
let response = engine.query(query).unwrap();
```

## API Reference

### Core Traits

```rust
// StateEngine - Application execution engine
pub trait StateEngine: Send {
    fn init(&mut self, config: &[u8]) -> Result<()>;
    fn apply(&mut self, op: &OperationEnvelope) -> Result<ApplyResult>;
    fn snapshot(&mut self) -> Result<Vec<u8>>;
    fn restore(&mut self, revision: u64, snapshot_data: &[u8]) -> Result<()>;
    fn query(&self, query_payload: &[u8]) -> Result<Vec<u8>>;
    fn current_revision(&self) -> u64;
}

// LogStore - Write-ahead log
pub trait LogStore: Send {
    fn append_operation(&mut self, op: &OperationEnvelope) -> Result<()>;
    fn get_operation(&self, revision: u64) -> Result<Option<OperationEnvelope>>;
    fn get_operations_since(&self, revision: u64, limit: usize) -> Result<Vec<OperationEnvelope>>;
    fn latest_revision(&self) -> Result<u64>;
    fn truncate_prefix(&mut self, up_to_revision: u64) -> Result<()>;
}

// SnapshotStore - State snapshots
pub trait SnapshotStore: Send {
    fn save_snapshot(&mut self, snapshot: &Snapshot) -> Result<()>;
    fn get_latest_snapshot(&self) -> Result<Option<Snapshot>>;
    fn get_snapshot_at(&self, revision: u64) -> Result<Option<Snapshot>>;
}
```

### Runtime API

```rust
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
```

### Membership API

```rust
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
```

## Project Structure

```
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
```

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific crate tests
cargo test -p nebenk-membership
cargo test -p nebenk-runtime

# Run release mode tests
cargo test --release
```

## Troubleshooting

### Build Issues

**"OpenSSL not found"**
```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# macOS
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
```

**"SQLite not found"**
```bash
# Ubuntu/Debian
sudo apt-get install libsqlite3-dev

# macOS
brew install sqlite3
```

### Runtime Issues

**"Only primary can accept commands"**
- Only the primary node accepts write operations (`put`, `delete`)
- Replicas are read-only (`get`, `list`)

**"State hash mismatch"**
- Corrupted snapshot or WAL
- Delete `.nebenk/state.db` and re-initialize, or restore from backup

**"Failed to bind to port"**
- Port already in use
- Use `--port` flag to specify different port

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Submit a PR

## Status

✅ **MVP Complete** - Core functionality implemented and tested

### Implemented Features
- [x] Cluster membership with Phi Accrual Failure Detector
- [x] Leader election and automatic failover
- [x] Write-ahead logging (SQLite)
- [x] State snapshots with hash verification
- [x] Sync replication (snapshot + operation replay)
- [x] Live operation replication
- [x] WASM runtime integration
- [x] P2P networking (libp2p)
- [x] CLI for node management

### Roadmap
- [ ] WASM application deployment API
- [ ] Multi-cluster support
- [ ] Encryption at rest
- [ ] Metrics / observability
- [ ] Web dashboard
