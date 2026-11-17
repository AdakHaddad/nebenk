# Contributing to NEBENK

Thank you for your interest in contributing to NEBENK! This document provides guidelines for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Code Style](#code-style)
- [Architecture Overview](#architecture-overview)
- [Issue Reporting](#issue-reporting)

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to conduct@nebenk.dev.

## Getting Started

### Prerequisites

- **Rust 1.75+** (install via [rustup](https://rustup.rs/))
- **SQLite3** development headers
- **OpenSSL** development headers
- **Protocol Buffers** compiler (`protoc`)
- **Node.js 20+** (for documentation website)

### System Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get update && sudo apt-get install -y \
    libsqlite3-dev libssl-dev pkg-config \
    protobuf-compiler nodejs npm
```

**macOS:**
```bash
brew install sqlite3 openssl protobuf node
```

**Fedora/RHEL:**
```bash
sudo dnf install sqlite-devel openssl-devel pkg-config \
    protobuf-compiler nodejs npm
```

## Development Setup

### Clone and Build

```bash
git clone https://github.com/AdakHaddad/nebenk
cd nebenk
cargo build --release
```

The binary will be at `target/release/nebenk`.

### Run Tests

```bash
# All tests
cargo test

# With output
cargo test -- --nocapture

# Specific crate
cargo test -p nebenk-membership
cargo test -p nebenk-runtime

# Release mode tests
cargo test --release
```

### Documentation Website

```bash
cd web
npm install
npm run dev  # http://localhost:3000
npm run build  # static export to web/out/
```

## Making Changes

### Branch Naming

- `feat/<short-description>` - New features
- `fix/<short-description>` - Bug fixes
- `docs/<short-description>` - Documentation updates
- `refactor/<short-description>` - Code refactoring
- `test/<short-description>` - Test additions
- `chore/<short-description>` - Maintenance tasks

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

Examples:
```
feat(membership): add phi accrual failure detector
fix(storage): handle snapshot corruption on restart
docs(api): add WASM host ABI reference
test(replication): add 3-node sync integration test
```

### Pull Request Process

1. **Fork** the repository
2. **Create** a feature branch from `main`
3. **Make** your changes with tests
4. **Run** the full test suite: `cargo test`
5. **Format** code: `cargo fmt`
6. **Lint**: `cargo clippy -- -D warnings`
7. **Push** and open a Pull Request

### PR Requirements

- [ ] All tests pass (`cargo test`)
- [ ] Code formatted (`cargo fmt --check`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Documentation updated if needed
- [ ] CHANGELOG.md updated for user-facing changes
- [ ] Linked to relevant issue

## Testing

### Test Categories

| Category | Command | Purpose |
|----------|---------|---------|
| Unit | `cargo test -p nebenk-core` | Core types, traits, serialization |
| Integration | `cargo test -p nebenk-runtime` | Multi-component tests |
| E2E | `cargo test --test integration` | Multi-node scenarios |
| Fault Injection | `cargo test --test fault_injection` | Chaos testing |

### Writing Tests

```rust
// Unit test example
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_operation_envelope_serialization() {
        let envelope = OperationEnvelope::new(...);
        let encoded = cbor4ii::serde::to_vec(&envelope).unwrap();
        let decoded: OperationEnvelope = cbor4ii::serde::from_slice(&encoded).unwrap();
        assert_eq!(envelope, decoded);
    }
}
```

### Test Guidelines

- **Deterministic**: Tests must not depend on timing or external state
- **Isolated**: Each test sets up its own fixtures
- **Fast**: Unit tests < 100ms; integration tests < 10s
- **Descriptive**: Test names explain what is being verified

## Code Style

### Rust

- **Edition**: 2021
- **Formatter**: `rustfmt` (run `cargo fmt`)
- **Linter**: `clippy` (run `cargo clippy`)
- **Naming**: Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### Key Patterns

```rust
// Error handling with thiserror
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    
    #[error("Snapshot not found at revision {0}")]
    SnapshotNotFound(u64),
}

// Async with tokio
pub async fn replicate(&mut self, op: OperationEnvelope) -> Result<()> {
    // ...
}

// Traits for abstraction
#[async_trait]
pub trait StateEngine: Send {
    async fn apply(&mut self, op: &OperationEnvelope) -> Result<ApplyResult>;
}
```

### Documentation

- Public APIs must have `///` doc comments
- Include examples in doc comments
- Run `cargo doc --no-deps --document-private-items`

## Architecture Overview

### Crate Structure

```
nebenk/
├── crates/
│   ├── nebenk-core/        # Shared types, traits, errors
│   ├── nebenk-identity/    # Ed25519 keys, invitations
│   ├── nebenk-storage/     # SQLite WAL + snapshots
│   ├── nebenk-wasm/        # Wasmtime runtime + native KV
│   ├── nebenk-network/     # libp2p P2P networking
│   ├── nebenk-membership/  # Cluster membership, FD
│   ├── nebenk-replication/ # Primary/replica sync
│   ├── nebenk-runtime/     # Node coordinator
│   └── nebenk-cli/         # CLI binary
├── examples/
│   ├── counter/            # Minimal counter app
│   └── kv/                 # Distributed KV store
└── tests/                  # E2E integration tests
```

### Dependency Graph

```
nebenk-cli
    ↓
nebenk-runtime
    ↓
┌─────┬─────┬─────┬─────┬─────┐
↓     ↓     ↓     ↓     ↓
wasm storage network membership replication
    ↓     ↓     ↓     ↓
└─────────── nebenk-core ────────────┘
    ↓
nebenk-identity
```

### Key Design Principles

1. **Deterministic State Machines** - Same input → same output
2. **Layered Decoupling** - Core traits independent of I/O
3. **Explicit Durability** - V1 acknowledges replication lag
4. **Pragmatic V1** - Single-writer primary/replica model

## Issue Reporting

### Bug Reports

Include:
- **Environment**: OS, Rust version, hardware
- **Steps to reproduce**: Minimal reproduction case
- **Expected vs actual behavior**
- **Logs**: Relevant output with `RUST_LOG=debug`
- **Version**: Git commit or release tag

### Feature Requests

- **Problem statement**: What problem does this solve?
- **Proposed solution**: High-level approach
- **Alternatives considered**: Why this approach?
- **Breaking changes**: Any API impact?

### Security Issues

Report privately to security@nebenk.dev (or GitHub Security Advisories). Do not file public issues for vulnerabilities.

## Release Process

1. Update `Cargo.toml` versions
2. Update `CHANGELOG.md`
3. Create release PR
4. Tag release: `git tag v0.x.y`
5. GitHub Actions publishes artifacts

## Questions?

- **Discussions**: GitHub Discussions tab
- **Discord**: [NEBENK Community](https://discord.gg/nebenk)
- **Email**: dev@nebenk.dev

---

*Thank you for contributing to NEBENK!*
