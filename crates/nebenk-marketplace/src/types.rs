//! Marketplace core types

use crate::provider::ProviderHeartbeat;
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_network::protocol;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a marketplace deployment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeploymentId(pub Uuid);

impl DeploymentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DeploymentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DeploymentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Device capabilities advertised by providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceCapabilities {
    pub cpu_cores: u32,
    pub cpu_arch: String,          // "x86_64", "aarch64", "armv7"
    pub os: String,                // "linux", "android", "macos", "windows"
    pub memory_gb: u32,
    pub storage_gb: u32,
    pub storage_type: String,      // "ssd", "hdd", "nvme"
    pub gpu: Option<GpuInfo>,
    pub network: NetworkInfo,
    pub features: Vec<String>,     // "wasm", "oci", "sgx", "tp"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuInfo {
    pub vendor: String,            // "nvidia", "amd", "intel", "apple"
    pub model: String,
    pub memory_gb: u32,
    pub cuda_cores: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkInfo {
    pub public_ip: Option<String>,
    pub nat_type: NatType,
    pub bandwidth_mbps_up: u32,
    pub bandwidth_mbps_down: u32,
    pub latency_ms_avg: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NatType {
    None,
    FullCone,
    RestrictedCone,
    PortRestrictedCone,
    Symmetric,
    Unknown,
}

/// Pricing model for provider resources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PricingModel {
    pub cpu_per_hour: u64,              // base token units per CPU core hour
    pub memory_per_gb_hour: u64,        // per GB RAM hour
    pub storage_per_gb_hour: u64,       // per GB storage hour
    pub gpu_per_hour: Option<u64>,      // per GPU hour
    pub egress_per_gb: u64,             // per GB egress
    pub min_epoch_hours: u32,           // minimum deployment duration
}

/// Stake information for slashing protection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StakeInfo {
    pub amount: u64,
    pub token: String,                  // "ETH", "USDC", "NEBENK"
    pub lock_duration_secs: u64,        // lock period in seconds
    pub contract_address: Option<String>, // smart contract address
}

/// Reputation score (0-100)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReputationScore(pub u8);

impl ReputationScore {
    pub fn new(score: u8) -> Self {
        Self(score.min(100))
    }
    
    pub fn default() -> Self {
        Self(50) // New providers start at 50
    }
    
    pub fn decay(&mut self, months_inactive: u32) {
        let decay = (5u8.saturating_mul(months_inactive as u8)).min(self.0);
        self.0 = self.0.saturating_sub(decay);
    }
    
    pub fn is_eligible_for_premium(&self) -> bool {
        self.0 >= 75
    }
}

impl Default for ReputationScore {
    fn default() -> Self {
        Self::default()
    }
}

/// Provider advertisement published to registry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderAdvertisement {
    pub node_id: NodeId,
    pub cluster_id: ClusterId,
    pub capabilities: DeviceCapabilities,
    pub pricing: PricingModel,
    pub stake: StakeInfo,
    pub reputation: ReputationScore,
    pub registered_at: u64,             // Unix timestamp
    pub last_heartbeat: u64,            // Unix timestamp
    pub signature: Vec<u8>,             // Signed by node_id
}

impl ProviderAdvertisement {
    /// Create a new advertisement (unsigned)
    pub fn new(
        node_id: NodeId,
        cluster_id: ClusterId,
        capabilities: DeviceCapabilities,
        pricing: PricingModel,
        stake: StakeInfo,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            node_id,
            cluster_id,
            capabilities,
            pricing,
            stake,
            reputation: ReputationScore::default(),
            registered_at: now,
            last_heartbeat: now,
            signature: Vec::new(),
        }
    }
    
    /// Get the data to be signed (excludes signature field)
    pub fn signing_data(&self) -> Vec<u8> {
        let mut clone = self.clone();
        clone.signature = Vec::new();
        let mut buf = Vec::new();
        cbor4ii::serde::to_vec(buf, &clone).unwrap_or_default()
    }
    
    /// Verify the signature
    pub fn verify(&self) -> bool {
        if self.signature.is_empty() {
            return false;
        }
        
        use ed25519_dalek::{Verifier, VerifyingKey, Signature};
        let verifying_key = match VerifyingKey::from_bytes(&self.node_id.0) {
            Ok(k) => k,
            Err(_) => return false,
        };
        
        let sig_bytes: [u8; 64] = match self.signature.as_slice().try_into() {
            Ok(arr) => arr,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&sig_bytes);
        
        verifying_key.verify(&self.signing_data(), &sig).is_ok()
    }
}

/// Resource requirements for deployment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceRequirements {
    pub min_cpu_cores: u32,
    pub min_memory_gb: u32,
    pub min_storage_gb: u32,
    pub required_arch: Vec<String>,
    pub required_features: Vec<String>,
    pub gpu_required: bool,
}

/// Replication configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplicationConfig {
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub quorum_write: bool,
    pub preferred_regions: Vec<String>,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            min_replicas: 2,
            max_replicas: 5,
            quorum_write: true,
            preferred_regions: Vec::new(),
        }
    }
}

/// Deployment pricing constraints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeploymentPricing {
    pub max_total_per_hour: u64,
    pub max_per_node_per_hour: u64,
    pub payment_token: String,
    pub escrow_epochs: u32,
}

/// Cluster preferences for deployment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterPreferences {
    pub allowed_regions: Vec<String>,
    pub excluded_node_ids: Vec<NodeId>,
    pub min_reputation: ReputationScore,
    pub require_gpu: bool,
}

impl Default for ClusterPreferences {
    fn default() -> Self {
        Self {
            allowed_regions: Vec::new(),
            excluded_node_ids: Vec::new(),
            min_reputation: ReputationScore::new(25),
            require_gpu: false,
        }
    }
}

/// Deployment manifest created by developers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeploymentManifest {
    pub deployment_id: DeploymentId,
    pub app_name: String,
    pub app_version: String,
    pub wasm_hash: [u8; 32],            // BLAKE3 hash of app.wasm
    pub wasm_url: String,               // IPFS/HTTP URL to fetch WASM
    pub manifest_hash: [u8; 32],        // Hash of this manifest
    pub requirements: ResourceRequirements,
    pub replication: ReplicationConfig,
    pub pricing: DeploymentPricing,
    pub cluster_prefs: ClusterPreferences,
    pub created_at: u64,
    pub creator: NodeId,                // Developer's node ID
    pub signature: Vec<u8>,             // Signed by creator
}

impl DeploymentManifest {
    pub fn new(
        app_name: String,
        app_version: String,
        wasm_hash: [u8; 32],
        wasm_url: String,
        requirements: ResourceRequirements,
        replication: ReplicationConfig,
        pricing: DeploymentPricing,
        cluster_prefs: ClusterPreferences,
        creator: NodeId,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Compute manifest hash (without signature)
        let mut temp = Self {
            deployment_id: DeploymentId::new(),
            app_name,
            app_version,
            wasm_hash,
            wasm_url,
            manifest_hash: [0u8; 32],
            requirements,
            replication,
            pricing,
            cluster_prefs,
            created_at: now,
            creator,
            signature: Vec::new(),
        };
        let buf = Vec::new();
        cbor4ii::serde::to_vec(buf.clone(), &temp).unwrap_or_default();
        temp.manifest_hash = blake3::hash(&buf).into();
        
        temp
    }
}

/// Epoch settlement information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpochSettlement {
    pub epoch: u64,
    pub deployment_id: DeploymentId,
    pub provider_node_id: NodeId,
    pub epoch_start: u64,
    pub epoch_end: u64,
    pub uptime_percentage: f32,
    pub replication_lag_ms: u64,
    pub sla_met: bool,
    pub payment_amount: u64,
    pub slashed_amount: u64,
    pub settled_at: Option<u64>,
}

/// Provider earnings summary
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderEarnings {
    pub node_id: NodeId,
    pub total_earned: u64,
    pub total_slashed: u64,
    pub pending_payout: u64,
    pub epochs_completed: u64,
    pub current_stake: u64,
    pub reputation: ReputationScore,
}

/// Deployment status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Pending,        // Waiting for providers
    Provisioning,   // Providers joining cluster
    Running,        // App deployed and replicating
    Scaling,        // Adding/removing replicas
    Paused,         // Temporarily stopped
    Completed,      // Finished successfully
    Failed,         // Failed (insufficient providers, etc.)
    Cancelled,      // Cancelled by developer
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Active deployment info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveDeployment {
    pub deployment_id: DeploymentId,
    pub manifest: DeploymentManifest,
    pub status: DeploymentStatus,
    pub provider_node_ids: Vec<NodeId>,
    pub cluster_id: ClusterId,
    pub started_at: Option<u64>,
    pub current_epoch: u64,
    pub escrow_balance: u64,
    pub total_paid: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::{ClusterId, NodeId};
    use std::str::FromStr;
    
    #[test]
    fn test_provider_advertisement_signing() {
        let node_id = NodeId::from_bytes([1u8; 32]);
        let cluster_id = ClusterId::new("test-marketplace");
        let capabilities = DeviceCapabilities {
            cpu_cores: 8,
            cpu_arch: "x86_64".to_string(),
            os: "linux".to_string(),
            memory_gb: 32,
            storage_gb: 500,
            storage_type: "nvme".to_string(),
            gpu: None,
            network: NetworkInfo {
                public_ip: Some("1.2.3.4".to_string()),
                nat_type: NatType::None,
                bandwidth_mbps_up: 1000,
                bandwidth_mbps_down: 1000,
                latency_ms_avg: Some(10),
            },
            features: vec!["wasm".to_string(), "oci".to_string()],
        };
        let pricing = PricingModel {
            cpu_per_hour: 10,
            memory_per_gb_hour: 1,
            storage_per_gb_hour: 1,
            gpu_per_hour: None,
            egress_per_gb: 5,
            min_epoch_hours: 1,
        };
        let stake = StakeInfo {
            amount: 1000,
            token: "USDC".to_string(),
            lock_duration_secs: 86400 * 30,
            contract_address: None,
        };
        
        let adv = ProviderAdvertisement::new(node_id, cluster_id, capabilities, pricing, stake);
        let data = adv.signing_data();
        assert!(!data.is_empty());
        
        // Verify unsigned fails
        assert!(!adv.verify());
    }
    
    #[test]
    fn test_deployment_manifest_hash() {
        let node_id = NodeId::from_bytes([2u8; 32]);
        let requirements = ResourceRequirements {
            min_cpu_cores: 2,
            min_memory_gb: 4,
            min_storage_gb: 10,
            required_arch: vec!["x86_64".to_string()],
            required_features: vec!["wasm".to_string()],
            gpu_required: false,
        };
        let manifest = DeploymentManifest::new(
            "my-app".to_string(),
            "1.0.0".to_string(),
            [3u8; 32],
            "ipfs://Qm...".to_string(),
            requirements,
            ReplicationConfig::default(),
            DeploymentPricing {
                max_total_per_hour: 100,
                max_per_node_per_hour: 50,
                payment_token: "USDC".to_string(),
                escrow_epochs: 24,
            },
            ClusterPreferences::default(),
            node_id,
        );
        
        assert_ne!(manifest.manifest_hash, [0u8; 32]);
        assert_eq!(manifest.deployment_id.0.as_bytes().len(), 16);
    }
    
    #[test]
    fn test_reputation_decay() {
        let mut rep = ReputationScore::new(80);
        rep.decay(1); // 1 month inactive
        assert_eq!(rep.0, 75);
        
        rep.decay(10); // 10 months inactive
        assert_eq!(rep.0, 25); // min is 0, but decays by 50
        
        rep.decay(100);
        assert_eq!(rep.0, 0);
    }
}

// Conversion functions for network protocol types

// Conversion functions for network protocol types
impl ProviderAdvertisement {
    /// Convert to network protocol message type
    pub fn to_msg(&self) -> nebenk_network::protocol::ProviderAdvertisementMsg {
        nebenk_network::protocol::ProviderAdvertisementMsg {
            node_id: self.node_id,
            cluster_id: self.cluster_id.clone(),
            capabilities: self.capabilities.to_msg(),
            pricing: self.pricing.to_msg(),
            stake: self.stake.to_msg(),
            reputation: self.reputation.to_msg(),
            registered_at: self.registered_at,
            last_heartbeat: self.last_heartbeat,
            signature: self.signature.clone(),
        }
    }
    
    /// Create from network protocol message type
    pub fn from_msg(msg: nebenk_network::protocol::ProviderAdvertisementMsg) -> Self {
        Self {
            node_id: msg.node_id,
            cluster_id: msg.cluster_id,
            capabilities: DeviceCapabilities::from_msg(msg.capabilities),
            pricing: PricingModel::from_msg(msg.pricing),
            stake: StakeInfo::from_msg(msg.stake),
            reputation: ReputationScore::from_msg(msg.reputation),
            registered_at: msg.registered_at,
            last_heartbeat: msg.last_heartbeat,
            signature: msg.signature,
        }
    }
}

impl ProviderHeartbeat {
    /// Convert to network protocol message type
    pub fn to_msg(&self) -> nebenk_network::protocol::ProviderHeartbeatMsg {
        nebenk_network::protocol::ProviderHeartbeatMsg {
            node_id: self.node_id,
            timestamp: self.timestamp,
            reputation: self.reputation.to_msg(),
            active_deployments: self.active_deployments,
            cpu_usage_percent: 0.0,
            memory_usage_percent: 0.0,
            signature: Vec::new(),
        }
    }
    
    /// Create from network protocol message type
    pub fn from_msg(msg: nebenk_network::protocol::ProviderHeartbeatMsg) -> Self {
        Self {
            node_id: msg.node_id,
            timestamp: msg.timestamp,
            active_deployments: msg.active_deployments,
            reputation: ReputationScore::from_msg(msg.reputation),
        }
    }
}

impl DeviceCapabilities {
    pub fn to_msg(&self) -> nebenk_network::protocol::DeviceCapabilitiesMsg {
        nebenk_network::protocol::DeviceCapabilitiesMsg {
            cpu_cores: self.cpu_cores,
            cpu_arch: self.cpu_arch.clone(),
            os: self.os.clone(),
            memory_gb: self.memory_gb,
            storage_gb: self.storage_gb,
            storage_type: self.storage_type.clone(),
            gpu: self.gpu.as_ref().map(|g| g.to_msg()),
            network: self.network.to_msg(),
            features: self.features.clone(),
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::DeviceCapabilitiesMsg) -> Self {
        Self {
            cpu_cores: msg.cpu_cores,
            cpu_arch: msg.cpu_arch,
            os: msg.os,
            memory_gb: msg.memory_gb,
            storage_gb: msg.storage_gb,
            storage_type: msg.storage_type,
            gpu: msg.gpu.map(|g| GpuInfo::from_msg(g)),
            network: NetworkInfo::from_msg(msg.network),
            features: msg.features,
        }
    }
}

impl GpuInfo {
    pub fn to_msg(&self) -> nebenk_network::protocol::GpuInfoMsg {
        nebenk_network::protocol::GpuInfoMsg {
            vendor: self.vendor.clone(),
            model: self.model.clone(),
            memory_gb: self.memory_gb,
            cuda_cores: self.cuda_cores,
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::GpuInfoMsg) -> Self {
        Self {
            vendor: msg.vendor,
            model: msg.model,
            memory_gb: msg.memory_gb,
            cuda_cores: msg.cuda_cores,
        }
    }
}

impl NetworkInfo {
    pub fn to_msg(&self) -> nebenk_network::protocol::NetworkInfoMsg {
        nebenk_network::protocol::NetworkInfoMsg {
            public_ip: self.public_ip.clone(),
            nat_type: self.nat_type.to_msg(),
            bandwidth_mbps_up: self.bandwidth_mbps_up,
            bandwidth_mbps_down: self.bandwidth_mbps_down,
            latency_ms_avg: self.latency_ms_avg,
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::NetworkInfoMsg) -> Self {
        Self {
            public_ip: msg.public_ip,
            nat_type: NatType::from_msg(msg.nat_type),
            bandwidth_mbps_up: msg.bandwidth_mbps_up,
            bandwidth_mbps_down: msg.bandwidth_mbps_down,
            latency_ms_avg: msg.latency_ms_avg,
        }
    }
}

impl NatType {
    pub fn to_msg(&self) -> nebenk_network::protocol::NatTypeMsg {
        match self {
            NatType::None => nebenk_network::protocol::NatTypeMsg::None,
            NatType::FullCone => nebenk_network::protocol::NatTypeMsg::FullCone,
            NatType::RestrictedCone => nebenk_network::protocol::NatTypeMsg::RestrictedCone,
            NatType::PortRestrictedCone => nebenk_network::protocol::NatTypeMsg::PortRestrictedCone,
            NatType::Symmetric => nebenk_network::protocol::NatTypeMsg::Symmetric,
            NatType::Unknown => nebenk_network::protocol::NatTypeMsg::Unknown,
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::NatTypeMsg) -> Self {
        match msg {
            nebenk_network::protocol::NatTypeMsg::None => NatType::None,
            nebenk_network::protocol::NatTypeMsg::FullCone => NatType::FullCone,
            nebenk_network::protocol::NatTypeMsg::RestrictedCone => NatType::RestrictedCone,
            nebenk_network::protocol::NatTypeMsg::PortRestrictedCone => NatType::PortRestrictedCone,
            nebenk_network::protocol::NatTypeMsg::Symmetric => NatType::Symmetric,
            nebenk_network::protocol::NatTypeMsg::Unknown => NatType::Unknown,
        }
    }
}

impl PricingModel {
    pub fn to_msg(&self) -> nebenk_network::protocol::PricingModelMsg {
        nebenk_network::protocol::PricingModelMsg {
            cpu_per_hour: self.cpu_per_hour,
            memory_per_gb_hour: self.memory_per_gb_hour,
            storage_per_gb_hour: self.storage_per_gb_hour,
            gpu_per_hour: self.gpu_per_hour,
            egress_per_gb: self.egress_per_gb,
            min_epoch_hours: self.min_epoch_hours,
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::PricingModelMsg) -> Self {
        Self {
            cpu_per_hour: msg.cpu_per_hour,
            memory_per_gb_hour: msg.memory_per_gb_hour,
            storage_per_gb_hour: msg.storage_per_gb_hour,
            gpu_per_hour: msg.gpu_per_hour,
            egress_per_gb: msg.egress_per_gb,
            min_epoch_hours: msg.min_epoch_hours,
        }
    }
}

impl StakeInfo {
    pub fn to_msg(&self) -> nebenk_network::protocol::StakeInfoMsg {
        nebenk_network::protocol::StakeInfoMsg {
            amount: self.amount,
            token: self.token.clone(),
            lock_duration_secs: self.lock_duration_secs,
            contract_address: self.contract_address.clone(),
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::StakeInfoMsg) -> Self {
        Self {
            amount: msg.amount,
            token: msg.token,
            lock_duration_secs: msg.lock_duration_secs,
            contract_address: msg.contract_address,
        }
    }
}

impl ReputationScore {
    pub fn to_msg(&self) -> nebenk_network::protocol::ReputationScoreMsg {
        nebenk_network::protocol::ReputationScoreMsg(self.0)
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::ReputationScoreMsg) -> Self {
        Self(msg.0)
    }
}

impl ResourceRequirements {
    pub fn to_msg(&self) -> nebenk_network::protocol::ResourceRequirementsMsg {
        nebenk_network::protocol::ResourceRequirementsMsg {
            min_cpu_cores: self.min_cpu_cores,
            min_memory_gb: self.min_memory_gb,
            min_storage_gb: self.min_storage_gb,
            required_arch: self.required_arch.clone(),
            required_features: self.required_features.clone(),
            gpu_required: self.gpu_required,
        }
    }
    
    pub fn from_msg(msg: nebenk_network::protocol::ResourceRequirementsMsg) -> Self {
        Self {
            min_cpu_cores: msg.min_cpu_cores,
            min_memory_gb: msg.min_memory_gb,
            min_storage_gb: msg.min_storage_gb,
            required_arch: msg.required_arch,
            required_features: msg.required_features,
            gpu_required: msg.gpu_required,
        }
    }
}
