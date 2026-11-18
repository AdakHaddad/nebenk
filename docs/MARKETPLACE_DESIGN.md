# NEBENK Marketplace Design

**Status:** Draft / Proposal  
**Version:** 0.1.0  
**Related:** PRD v0.4, Technical Design v0.1.0  
**License:** Apache License 2.0  

---

## 1. Overview

This document proposes a **device marketplace** for NEBENK — a decentralized marketplace where device owners can offer their spare compute resources (CPU, memory, storage, bandwidth) to run NEBENK applications, and application developers can discover and deploy to these devices.

### 1.1 Vision

> "The device is an implementation detail of where an application happens to be running."

NEBENK already enables applications to run across heterogeneous devices. The marketplace extends this by creating an **economic layer** that:
- Incentivizes device owners to participate
- Helps developers find suitable devices
- Handles settlement, reputation, and trust

### 1.2 Core Principles

1. **Device-agnostic** — Works on Android, Linux, Windows, macOS, ARM, x86
2. **Local-first** — No central cloud required; P2P discovery and settlement
3. **Open protocol** — Anyone can build clients, indexers, or settlement layers
4. **Minimal trust** — Smart contracts / escrow for settlement; reputation for quality
5. **NEBENK-native** — Uses NEBENK's identity, networking, and state replication

---

## 2. Marketplace Actors

| Actor | Role | Incentive |
|-------|------|-----------|
| **Device Provider** | Runs NEBENK runtime, offers spare resources | Earn tokens/rewards for uptime & compute |
| **App Developer** | Deploys WASM apps to marketplace devices | Access to distributed, resilient infrastructure |
| **Indexer/Registry** | Discovers & lists available devices | Fee for matching / curation |
| **Settlement Layer** | Handles payments, escrow, slashing | Protocol fees |

---

## 3. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      NEBENK Marketplace                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │   Provider   │    │   Registry   │    │  Developer   │     │
│  │   (Node)     │◄───│  (Indexer)   │───►│  (Client)    │     │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘     │
│         │                   │                   │              │
│         ▼                   ▼                   ▼              │
│  ┌──────────────────────────────────────────────────────┐     │
│  │              NEBENK P2P Network (libp2p)             │     │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │     │
│  │  │Identity │ │Network  │ │Membership│ │Replication│   │     │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘    │     │
│  └──────────────────────────────────────────────────────┘     │
│         │                   │                   │              │
│         ▼                   ▼                   ▼              │
│  ┌──────────────────────────────────────────────────────┐     │
│  │              Settlement Layer (Smart Contracts)       │     │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │     │
│  │  │ Escrow  │ │Reputation│ │ Slashing │ │ Payments │   │     │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘    │     │
│  └──────────────────────────────────────────────────────┘     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. Provider Registration Flow

```
1. Provider starts NEBENK node with `--marketplace` flag
2. Node registers with Registry via libp2p:
   - NodeId (Ed25519 pubkey)
   - Capabilities: CPU cores, RAM, storage, GPU, arch, OS
   - Network: public IP, NAT type, bandwidth
   - Pricing: per-CPU-hour, per-GB-hour, per-GB-egress
   - Stake: bond tokens for slashing protection
3. Registry verifies:
   - Benchmark test (NEBENK stress test suite)
   - Network connectivity
   - Hardware attestation (optional, TPM/SGX)
4. Provider listed in Registry with reputation score
```

---

## 5. Developer Deployment Flow

```
1. Developer builds WASM app (`cargo build --target wasm32-wasip1`)
2. Developer creates deployment manifest:
   - App name, version, WASM hash
   - Resource requirements (min CPU, RAM, storage)
   - Replication factor (min replicas)
   - Max price per hour
   - Cluster preferences (region, arch, latency)
3. Client queries Registry for matching providers
4. Client creates cluster invitation tokens for selected providers
5. Providers accept invitation → join cluster
6. NEBENK runtime deploys app, starts replication
7. Settlement layer escrows payment
8. Provider earns per-epoch (hour) based on uptime & SLA
```

---

## 6. Data Structures

### 6.1 Provider Advertisement (CBOR)

```rust
pub struct ProviderAdvertisement {
    pub node_id: NodeId,
    pub cluster_id: ClusterId,  // marketplace cluster
    pub capabilities: DeviceCapabilities,
    pub pricing: PricingModel,
    pub stake: StakeInfo,
    pub reputation: ReputationScore,
    pub signature: Vec<u8>,  // signed by node_id
}

pub struct DeviceCapabilities {
    pub cpu_cores: u32,
    pub cpu_arch: String,  // "x86_64", "aarch64", "armv7"
    pub os: String,        // "linux", "android", "macos", "windows"
    pub memory_gb: u32,
    pub storage_gb: u32,
    pub storage_type: String,  // "ssd", "hdd", "nvme"
    pub gpu: Option<GpuInfo>,
    pub network: NetworkInfo,
    pub features: Vec<String>,  // "wasm", "oci", "sgx", "tp"
}

pub struct PricingModel {
    pub cpu_per_hour: u64,      // base token units
    pub memory_per_gb_hour: u64,
    pub storage_per_gb_hour: u64,
    pub egress_per_gb: u64,
    pub min_epoch_hours: u32,
}

pub struct StakeInfo {
    pub amount: u64,
    pub token: String,  // "ETH", "USDC", "NEBENK"
    pub lock_duration: u64,  // seconds
}
```

### 6.2 Deployment Manifest

```rust
pub struct DeploymentManifest {
    pub app_name: String,
    pub app_version: String,
    pub wasm_hash: [u8; 32],  // BLAKE3 of app.wasm
    pub wasm_url: String,     // IPFS/HTTP URL to fetch WASM
    pub requirements: ResourceRequirements,
    pub replication: ReplicationConfig,
    pub pricing: DeploymentPricing,
    pub cluster_prefs: ClusterPreferences,
}

pub struct ResourceRequirements {
    pub min_cpu_cores: u32,
    pub min_memory_gb: u32,
    pub min_storage_gb: u32,
    pub required_arch: Vec<String>,
    pub required_features: Vec<String>,
}

pub struct ReplicationConfig {
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub quorum_write: bool,
}

pub struct DeploymentPricing {
    pub max_total_per_hour: u64,
    pub max_per_node_per_hour: u64,
    pub payment_token: String,
}
```

---

## 7. Settlement & Economics

### 7.1 Epoch-Based Payment

- **Epoch**: 1 hour (configurable)
- **Payment**: Per-epoch, pro-rated for uptime
- **SLA**: Provider must maintain >99% uptime per epoch for full payment

### 7.2 Escrow Flow

```
1. Developer deposits estimated cost for N epochs into escrow
2. Each epoch:
   a. Registry verifies provider uptime (heartbeats + replication lag)
   b. If SLA met: release epoch payment to provider
   c. If SLA missed: slash stake, refund developer
3. Developer can top up or cancel deployment
4. Provider can withdraw earnings after lock period
```

### 7.3 Slashing Conditions

| Violation | Penalty |
|-----------|---------|
| Uptime < 99% in epoch | 10% of epoch stake |
| Replication lag > threshold | 20% of epoch stake |
| Data corruption (hash mismatch) | 50% of stake + reputation hit |
| Malicious behavior (equivocation) | 100% stake + permanent ban |

### 7.4 Reputation System

- **Score**: 0-100 (new providers start at 50)
- **Factors**: Uptime history, SLA compliance, benchmark scores, peer reviews
- **Decay**: Score decays 5% per month of inactivity
- **Usage**: Higher reputation = priority in matching, lower stake requirements

---

## 8. NEBENK Integration Points

### 8.1 New Crate: `nebenk-marketplace`

```
crates/
├── nebenk-marketplace/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── provider.rs      # Provider registration, advertisement
│   │   ├── client.rs        # Developer deployment client
│   │   ├── registry.rs      # Registry protocol (libp2p)
│   │   ├── settlement.rs    # Escrow, payments, slashing
│   │   ├── reputation.rs    # Reputation scoring
│   │   └── types.rs         # Shared types (advertisement, manifest)
│   ├── Cargo.toml
│   └── benches/
│       └── marketplace_bench.rs
```

### 8.2 CLI Extensions

```bash
# Provider side
nebenk marketplace register --stake 1000 --pricing-cpu 10 --pricing-mem 5
nebenk marketplace status
nebenk marketplace earnings

# Developer side
nebenk marketplace deploy --app ./app.wasm --replicas 3 --max-price 50/hr
nebenk marketplace list --cpu 4 --mem 8
nebenk marketplace logs <deployment-id>
```

### 8.3 Runtime Hooks

- `NodeRuntime::register_provider()` — called on startup with `--marketplace`
- `NodeRuntime::handle_deployment()` — receives deployment manifest, launches WASM
- `NodeRuntime::report_uptime()` — periodic heartbeat to registry
- `NodeRuntime::verify_sla()` — called by registry for epoch settlement

---

## 9. Phased Implementation

### Phase 1: Provider Registration & Discovery (MVP)
- [ ] `nebenk-marketplace` crate with provider/client types
- [ ] libp2p-based registry protocol (DHT or gossip)
- [ ] CLI commands for registration and listing
- [ ] Benchmark-based capability verification

### Phase 2: Deployment & Escrow
- [ ] Deployment manifest handling
- [ ] Cluster invitation & join flow
- [ ] Smart contract escrow (EVM-compatible)
- [ ] Epoch-based payment logic

### Phase 3: Reputation & Slashing
- [ ] Reputation scoring algorithm
- [ ] Slashing conditions & enforcement
- [ ] Dispute resolution mechanism
- [ ] Indexer API for marketplace explorers

### Phase 4: Advanced Features
- [ ] OCI/Docker support (Phase 8 PRD)
- [ ] GPU/accelerator marketplace
- [ ] Cross-chain settlement
- [ ] Federated registry (multiple indexers)
- [ ] Mobile provider support (Android background)

---

## 10. Open Questions

1. **Settlement Layer**: Native token vs stablecoin vs ETH? Bridge to existing chains?
2. **Registry Decentralization**: Single DHT vs multiple federated indexers?
3. **Pricing Model**: Fixed price vs auction vs dynamic based on demand?
4. **Identity**: Reuse NEBENK NodeId or separate marketplace identity?
5. **Legal/Compliance**: KYC for providers? Tax reporting?
6. **Mobile Constraints**: Android background limits, battery awareness
7. **Benchmark Standardization**: Which stress tests for capability verification?
8. **Dispute Resolution**: On-chain arbitration vs off-chain?

---

## 11. Related Work

- **Docker Hub** — Centralized container registry + paid plans
- **Akash Network** — Decentralized cloud marketplace (Kubernetes-based)
- **Bacalhau** — Compute over data (IPFS + WASM)
- **Fluence** — P2P compute network (WASM + Aqua)
- **Golem** — Decentralized compute marketplace
- **Render Network** — GPU compute marketplace

NEBENK differentiates by:
- **Application-level replication** (not just raw compute)
- **Device-agnostic** (mobile, edge, desktop, server)
- **Stateful applications** (portable state machine)
- **Local-first** (works without internet/cloud)

---

## 12. Next Steps

1. **Review & approve** this design
2. **Create `nebenk-marketplace` crate** with provider registration
3. **Extend CLI** with marketplace commands
4. **Build registry protocol** on libp2p
5. **Design smart contracts** for escrow/settlement
6. **Run testnet** with 10+ providers

---

*End of Marketplace Design Document*
