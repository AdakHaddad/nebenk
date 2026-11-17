# Contributing to NEBENK

Thank you for your interest in contributing to NEBENK! This document provides guidelines for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Style](#code-style)
- [Testing Guidelines](#testing-guidelines)
- [Pull Request Process](#pull-request-process)
- [Commit Message Conventions](#commit-message-conventions)
- [Project Structure](#project-structure)
- [Running Benchmarks](#running-benchmarks)

---

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md). Please be respectful and constructive in all interactions.

---

## Getting Started

### Prerequisites

- **Rust 1.75+** (install via [rustup](https://rustup.rs/))
- **SQLite3** development headers
- **OpenSSL** development headers (for libp2p TLS)
- **Git**

#### System Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get update && sudo apt-get install -y \
    libsqlite3-dev libssl-dev pkg-config
```

**macOS:**
```bash
brew install sqlite3 openssl
```

**Fedora/RHEL:**
```bash
sudo dnf install sqlite-devel openssl-devel pkg-config
```

### Clone and Build

```bash
git clone https://github.com/AdakHaddad/nebenk
cd nebenk
cargo build --release
```

The binary will be available at `target/release/nebenk`.

---

## Development Workflow

### 1. Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/nebenk
   cd nebenk
   git remote add upstream https://github.com/AdakHaddad/nebenk
   ```

### 2. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/your-bug-fix
```

Use descriptive branch names:
- `feature/add-wasm-deployment-api`
- `fix/snapshot-hash-verification`
- `docs/update-readme-quickstart`

### 3. Make Changes

- Follow the [Code Style](#code-style) guidelines
- Add tests for new functionality
- Update documentation as needed
- Run tests locally before pushing

### 4. Run Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific crate tests
cargo test -p nebenk-membership
cargo test -p nebenk-runtime

# Run release mode tests (recommended before PR)
cargo test --release
```

### 5. Run Lints

```bash
# Check for warnings and style issues
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt --all -- --check
```

### 6. Push and Create PR

```bash
git push origin feature/your-feature-name
```

Then open a Pull Request against the `main` branch of the upstream repository.

---

## Code Style

### Rust Style Guidelines

We follow the standard Rust style guide with a few project-specific conventions:

1. **Format with `rustfmt`** - Always run `cargo fmt` before committing
2. **Lint with `clippy`** - Fix all warnings: `cargo clippy --all-targets --all-features -- -D warnings`
3. **Use `#[must_use]`** for functions that return values that shouldn't be ignored
4. **Prefer `anyhow::Result`** for error handling in application code
5. **Use `thiserror`** for library error types (see `nebenk-core/src/error.rs`)
6. **Document public APIs** - All public types, traits, and functions should have doc comments

### Naming Conventions

| Item | Convention |
|------|------------|
| Crates | `kebab-case` (e.g., `nebenk-runtime`) |
| Modules | `snake_case` |
| Types/Structs/Enums | `PascalCase` |
| Functions/Methods | `snake_case` |
| Constants/Static | `SCREAMING_SNAKE_CASE` |
| Type Parameters | `PascalCase` (single letter: `T`, `E`, `K`, `V`) |
| Lifetimes | `'short_descriptive` |

### Error Handling

```rust
// Library code: use thiserror for structured errors
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("invalid revision: {revision}")]
    InvalidRevision { revision: u64 },
}

// Application code: use anyhow for context
use anyhow::{Context, Result};

fn do_something() -> Result<()> {
    let data = fs::read(path).context("failed to read config file")?;
    Ok(())
}
```

### Async Code

- Use `tokio` for async runtime
- Prefer `async fn` over `fn() -> impl Future`
- Avoid blocking in async contexts (use `tokio::task::spawn_blocking` for CPU-intensive work)
- Use `mpsc` channels for inter-task communication

### Documentation

- All public items must have doc comments (`///`)
- Include examples in doc comments where helpful
- Run `cargo doc --no-deps` to verify documentation builds

---

## Testing Guidelines

### Test Organization

```
crates/
├── nebenk-core/
│   └── src/
│       └── lib.rs          # Unit tests in `#[cfg(test)]` module
├── nebenk-membership/
│   └── src/
│       └── lib.rs          # Unit tests
└── ... (other crates)
```

### Writing Tests

1. **Unit tests** - Place in `#[cfg(test)]` module in the same file
2. **Integration tests** - Create `tests/` directory at crate root (for future use)
3. **Benchmark tests** - Place in `benches/` directory (criterion)

### Test Patterns

```rust
// Unit test example
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_works() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
}

// Async test example
#[tokio::test]
async fn test_async_operation() {
    let runtime = create_test_runtime().await;
    let result = runtime.some_async_op().await;
    assert!(result.is_ok());
}
```

### Test Coverage Goals

- **Core traits** (`StateEngine`, `LogStore`, `SnapshotStore`): 100% coverage
- **Membership/Failure Detector**: 90%+ coverage
- **Replication logic**: 90%+ coverage
- **Runtime integration**: 80%+ coverage
- **CLI commands**: Smoke tests for each command

### Property-Based Testing

For critical algorithms (failure detector, consensus), consider adding property-based tests using `proptest`:

```rust
#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_phi_calculation(heartbeats in vec(any::<u64>(), 1..100)) {
            // Property: phi should always be >= 0
            let phi = calculate_phi(&heartbeats);
            prop_assert!(phi >= 0.0);
        }
    }
}
```

---

## Pull Request Process

### Before Submitting

- [ ] All tests pass: `cargo test --release`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Code formatted: `cargo fmt --all -- --check`
- [ ] Documentation builds: `cargo doc --no-deps`
- [ ] Updated CHANGELOG.md (if applicable)
- [ ] Updated README.md (if user-facing changes)

### PR Requirements

1. **Clear title** - Describe what the PR does
2. **Description** - Explain the motivation and approach
3. **Linked issues** - Reference related issues (e.g., `Fixes #123`)
4. **Screenshots/logs** - For CLI or visual changes
5. **Breaking changes** - Clearly mark and document

### PR Template

```markdown
## Summary
Brief description of changes.

## Motivation
Why is this change needed?

## Changes
- Change 1
- Change 2

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests pass
- [ ] Benchmarks run (if performance-related)

## Checklist
- [ ] `cargo test --release` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] Documentation updated
```

### Review Process

1. **Automated checks** must pass (CI)
2. **Code review** by at least one maintainer
3. **Address feedback** - Push updates to the same branch
4. **Squash and merge** - Maintainers will squash commits on merge

---

## Commit Message Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, missing semicolons, etc. |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Adding missing tests or correcting existing tests |
| `chore` | Maintenance, dependency updates, build config |
| `ci` | CI/CD changes |

### Examples

```
feat(membership): add phi accrual failure detector config options

fix(storage): handle sqlite busy timeout correctly

docs(readme): update quickstart with join command

refactor(runtime): extract snapshot logic into separate module

perf(replication): batch operations during sync replication

test(membership): add property-based tests for phi calculation

chore(deps): update tokio to 1.43
```

### Scope Guidelines

Use the crate name as scope when changes are isolated:
- `core`, `identity`, `storage`, `wasm`, `network`, `membership`, `replication`, `runtime`, `cli`

Use `*` for cross-cutting changes:
- `*`: update MSRV, cargo workspace changes

---

## Project Structure

```
nebenk/
├── AGENTS.md                 # Agent instructions (this project)
├── Cargo.toml                # Workspace configuration
├── CONTRIBUTING.md           # This file
├── LICENSE                   # Apache-2.0
├── README.md                 # Project documentation
├── crates/
│   ├── nebenk-core/          # Core traits, types, errors
│   ├── nebenk-identity/      # Ed25519 keys, signing, invitations
│   ├── nebenk-storage/       # SQLite WAL + snapshots
│   ├── nebenk-wasm/          # WASM runtime (wasmtime) + native KV engine
│   ├── nebenk-network/       # libp2p transport, gossip, heartbeat protocols
│   ├── nebenk-membership/    # Cluster membership, Phi Accrual FD, leader election
│   ├── nebenk-replication/   # State sync (snapshot + replay) and live replication
│   ├── nebenk-runtime/       # Composed node runtime integrating all components
│   └── nebenk-cli/           # User-facing CLI commands
├── docs/
│   ├── TECHNICAL_DESIGN.md   # Detailed architecture
│   └── prd.md                # Product requirements
└── target/                   # Build output (gitignored)
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `nebenk-core` | Shared traits, types, error definitions |
| `nebenk-identity` | Cryptographic identity, node keys, cluster invitations |
| `nebenk-storage` | SQLite-backed WAL and snapshot storage |
| `nebenk-wasm` | WASM runtime (wasmtime) + native KV engine for testing |
| `nebenk-network` | libp2p transport, gossip, heartbeat protocols |
| `nebenk-membership` | Cluster membership, Phi Accrual FD, leader election |
| `nebenk-replication` | State sync (snapshot + replay) and live replication |
| `nebenk-runtime` | Composed node runtime integrating all components |
| `nebenk-cli` | User-facing CLI commands |

---

## Running Benchmarks

NEBENK uses [Criterion](https://github.com/bheisler/criterion.rs) for benchmarking.

### Run All Benchmarks

```bash
# From workspace root
cargo bench -p nebenk-runtime
```

### Run Specific Benchmark

```bash
# Run single benchmark group
cargo bench -p nebenk-runtime -- single_node_put_throughput

# Run with custom parameters
cargo bench -p nebenk-runtime -- --sample-size 20 --measurement-time 5
```

### Benchmark Output

Results are saved to `target/criterion/` with HTML reports. Open `target/criterion/report/index.html` in a browser to view.

### Adding New Benchmarks

1. Create a new function in `crates/nebenk-runtime/benches/stress_test.rs`
2. Follow the existing pattern using `BenchmarkGroup` for configuration:
   ```rust
   fn bench_my_operation(c: &mut Criterion) {
       let rt = Runtime::new().unwrap();
       let mut group = c.benchmark_group("my_operation");
       group.sample_size(10);
       group.measurement_time(Duration::from_secs(2));
       group.warm_up_time(Duration::from_secs(1));
       
       group.bench_function("my_operation", |b| {
           b.iter(|| { /* benchmark code */ })
       });
       group.finish();
   }
   ```
3. Add to `criterion_group!` macro

---

## Release Process

Releases are managed by maintainers:

1. Update version in `Cargo.toml` (workspace.package.version)
2. Update `CHANGELOG.md`
3. Create git tag: `git tag v0.x.x`
4. Push tag: `git push origin v0.x.x`
5. GitHub Actions builds and publishes artifacts

---

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/AdakHaddad/nebenk/issues)
- **Discussions**: [GitHub Discussions](https://github.com/AdakHaddad/nebenk/discussions)
- **Documentation**: See `docs/TECHNICAL_DESIGN.md` and `docs/prd.md`

---

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).

---

*Thank you for contributing to NEBENK!*
