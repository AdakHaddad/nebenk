'use client';

import { DocsLayout } from '@/app/docs-layout';
import { MarkdownRenderer } from '@/components/MarkdownRenderer';

export default function GettingStartedPage() {
  const content = `
# Getting Started

## Prerequisites

- **Rust 1.75+** (install via [rustup](https://rustup.rs/))
- **SQLite3** development headers
- **OpenSSL** development headers (for libp2p TLS)

### System Dependencies

**Ubuntu/Debian:**
\`\`\`bash
sudo apt-get update && sudo apt-get install -y \\
    libsqlite3-dev libssl-dev pkg-config
\`\`\`

**macOS:**
\`\`\`bash
brew install sqlite3 openssl
\`\`\`

**Fedora/RHEL:**
\`\`\`bash
sudo dnf install sqlite-devel openssl-devel pkg-config
\`\`\`

## Build from Source

\`\`\`bash
git clone https://github.com/AdakHaddad/nebenk
cd nebenk
cargo build --release
\`\`\`

The binary will be available at \`target/release/nebenk\`.

## Initialize a Node

\`\`\`bash
# Initialize node identity and storage
nebenk init --data-dir .nebenk --cluster my-cluster

# Output:
# Node initialized successfully!
# Node ID:  12D3KooW...
# Key Path: .nebenk/node.key
# Cluster:  my-cluster
\`\`\`

## Run as Primary (Leader)

\`\`\`bash
# Start node as primary (accepts writes)
nebenk --data-dir .nebenk --cluster my-cluster start --primary
\`\`\`

## Run as Replica

\`\`\`bash
# Start node as replica (read-only, replicates from primary)
nebenk --data-dir .nebenk --cluster my-cluster start --replica
\`\`\`

## Basic Operations

\`\`\`bash
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
\`\`\`

## Running a Cluster

### 3-Node Cluster Example

**Terminal 1 - Primary:**
\`\`\`bash
nebenk --data-dir .nebenk/node1 --cluster demo start --primary --port 8001
\`\`\`

**Terminal 2 - Replica 1:**
\`\`\`bash
# Get invitation from primary first, then:
nebenk --data-dir .nebenk/node2 --cluster demo start --replica --port 8002
\`\`\`

**Terminal 3 - Replica 2:**
\`\`\`bash
nebenk --data-dir .nebenk/node3 --cluster demo start --replica --port 8003
\`\`\`

### Test Failover

1. Write data on primary:
   \`\`\`bash
   nebenk --data-dir .nebenk/node1 put counter 1
   \`\`\`

2. Kill primary (Ctrl+C)

3. Replica automatically becomes new primary

4. Continue writing:
   \`\`\`bash
   nebenk --data-dir .nebenk/node2 put counter 2
   \`\`\`

## Configuration

### NodeConfig Options

\`\`\`rust
NodeConfig {
    cluster_id: ClusterId,        // Cluster identifier
    is_primary: bool,             // true = primary (accepts writes), false = replica
    snapshot_interval: u64,       // Snapshot every N operations (0 = disabled)
    heartbeat_interval_ms: u64,   // Heartbeat interval in milliseconds
}
\`\`\`

### CLI Flags

| Flag | Description | Default |
|------|-------------|---------|
| \`--data-dir\` | Base directory for keys and storage | \`.nebenk\` |
| \`--cluster\` | Cluster identifier | \`nebenk-default\` |
| \`--primary\` | Run as primary node | false |
| \`--replica\` | Run as replica node | true |
| \`--port\` | libp2p listen port | auto |
| \`--snapshot-interval\` | Snapshot every N ops | 50 |

## Application Development

### Writing a WASM Application

Applications compile to WASM and run in the NEBENK runtime:

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

\`\`\`rust
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
\`\`\`

### Native KV Engine (for testing)

The \`nebenk-wasm\` crate includes a native Rust KV engine for testing:

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

## Testing

\`\`\`bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific crate tests
cargo test -p nebenk-membership
cargo test -p nebenk-runtime

# Run release mode tests
cargo test --release
\`\`\`

## Troubleshooting

### Build Issues

**"OpenSSL not found"**
\`\`\`bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# macOS
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
\`\`\`

**"SQLite not found"**
\`\`\`bash
# Ubuntu/Debian
sudo apt-get install libsqlite3-dev

# macOS
brew install sqlite3
\`\`\`

### Runtime Issues

**"Only primary can accept commands"**
- Only the primary node accepts write operations (\`put\`, \`delete\`)
- Replicas are read-only (\`get\`, \`list\`)

**"State hash mismatch"**
- Corrupted snapshot or WAL
- Delete \`.nebenk/state.db\` and re-initialize, or restore from backup

**"Failed to bind to port"**
- Port already in use
- Use \`--port\` flag to specify different port

## Next Steps

- Read the [Architecture](/architecture) guide for deep technical details
- Explore the [API Reference](/api) for programmatic usage
- Check the [Product Requirements](/prd) for the full vision
`;

  return (
    <DocsLayout>
      <MarkdownRenderer content={content} />
    </DocsLayout>
  );
}
