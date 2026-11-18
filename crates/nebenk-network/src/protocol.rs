use nebenk_core::types::{ClusterId, NodeId, OperationEnvelope, Snapshot};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// High-level P2P protocol messages exchanged between NEBENK nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// Initial mutual authentication message.
    Handshake {
        node_id: NodeId,
        cluster_id: ClusterId,
        nonce: [u8; 32],
        signature: Vec<u8>,
    },
    /// Handshake acknowledgment response.
    HandshakeAck {
        success: bool,
        peer_node_id: NodeId,
        error_message: Option<String>,
    },
    /// Periodic liveness ping with full membership info.
    Heartbeat {
        node_id: NodeId,
        cluster_id: ClusterId,
        revision: u64,
        timestamp_ms: u64,
        role: NodeRole,
        state: NodeState,
    },
    /// Acknowledgment of heartbeat with membership view for gossip.
    HeartbeatAck {
        node_id: NodeId,
        timestamp_ms: u64,
        members: Vec<ClusterMember>,
    },
    /// Request catch-up state starting from a replica's last known revision.
    SyncRequest { last_known_revision: u64 },
    /// State catch-up response containing optional snapshot and missing operations.
    SyncResponse {
        latest_revision: u64,
        snapshot: Option<Snapshot>,
        operations: Vec<OperationEnvelope>,
    },
    /// Live operation streaming from primary to replicas.
    ReplicationOp { envelope: OperationEnvelope },
    /// Confirmation of operation received and written to local storage.
    ReplicationAck { revision: u64, node_id: NodeId },

    // Marketplace messages (defined inline to avoid cyclic dependency)
    /// Provider advertisement broadcast via gossipsub
    ProviderAdvertisement(ProviderAdvertisementMsg),
    /// Provider heartbeat broadcast via gossipsub
    ProviderHeartbeat(ProviderHeartbeatMsg),
    /// Provider list request via DHT
    ProviderListRequest(ProviderListRequestMsg),
    /// Provider list response via DHT
    ProviderListResponse(ProviderListResponseMsg),
    /// Deployment offer from client to provider via DHT
    DeploymentOffer(DeploymentOfferMsg),
    /// Provider acceptance/rejection of deployment offer
    DeploymentAcceptance(DeploymentAcceptanceMsg),
}

/// Node role in cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Primary,
    Replica,
    Observer,
}

/// Current state of a node in the cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Active,
    Syncing,
    Suspected,
    Failed,
    Leaving,
}

/// Information about a cluster member for gossip.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterMember {
    pub node_id: NodeId,
    pub public_key: [u8; 32],
    pub role: NodeRole,
    pub state: NodeState,
    pub last_seen_ms: u64,
    pub last_synced_rev: u64,
    pub address: Option<String>,
}

/// Heartbeat message sent between nodes (lightweight version for membership).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heartbeat {
    pub node_id: NodeId,
    pub cluster_id: ClusterId,
    pub revision: u64,
    pub timestamp_ms: u64,
    pub role: NodeRole,
    pub state: NodeState,
}

/// Heartbeat acknowledgment with membership view for gossip.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeartbeatAck {
    pub node_id: NodeId,
    pub timestamp_ms: u64,
    pub members: Vec<ClusterMember>,
}

/// Unique identifier for a marketplace deployment
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeploymentId(pub Uuid);

/// Device capabilities advertised by providers (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeviceCapabilitiesMsg {
    pub cpu_cores: u32,
    pub cpu_arch: String,
    pub os: String,
    pub memory_gb: u32,
    pub storage_gb: u32,
    pub storage_type: String,
    pub gpu: Option<GpuInfoMsg>,
    pub network: NetworkInfoMsg,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GpuInfoMsg {
    pub vendor: String,
    pub model: String,
    pub memory_gb: u32,
    pub cuda_cores: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkInfoMsg {
    pub public_ip: Option<String>,
    pub nat_type: NatTypeMsg,
    pub bandwidth_mbps_up: u32,
    pub bandwidth_mbps_down: u32,
    pub latency_ms_avg: Option<u32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NatTypeMsg {
    None,
    FullCone,
    RestrictedCone,
    PortRestrictedCone,
    Symmetric,
    Unknown,
}

/// Pricing model for provider resources (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PricingModelMsg {
    pub cpu_per_hour: u64,
    pub memory_per_gb_hour: u64,
    pub storage_per_gb_hour: u64,
    pub gpu_per_hour: Option<u64>,
    pub egress_per_gb: u64,
    pub min_epoch_hours: u32,
}

/// Stake information for slashing protection (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StakeInfoMsg {
    pub amount: u64,
    pub token: String,
    pub lock_duration_secs: u64,
    pub contract_address: Option<String>,
}

/// Reputation score (0-100) (network protocol version)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReputationScoreMsg(pub u8);

/// Provider advertisement published to registry (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProviderAdvertisementMsg {
    pub node_id: NodeId,
    pub cluster_id: ClusterId,
    pub capabilities: DeviceCapabilitiesMsg,
    pub pricing: PricingModelMsg,
    pub stake: StakeInfoMsg,
    pub reputation: ReputationScoreMsg,
    pub registered_at: u64,
    pub last_heartbeat: u64,
    pub signature: Vec<u8>,
}

/// Provider heartbeat (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderHeartbeatMsg {
    pub node_id: NodeId,
    pub timestamp: u64,
    pub reputation: ReputationScoreMsg,
    pub active_deployments: u32,
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub signature: Vec<u8>,
}

/// Resource requirements for deployment (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResourceRequirementsMsg {
    pub min_cpu_cores: u32,
    pub min_memory_gb: u32,
    pub min_storage_gb: u32,
    pub required_arch: Vec<String>,
    pub required_features: Vec<String>,
    pub gpu_required: bool,
}

/// Cluster preferences for deployment (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClusterPreferencesMsg {
    pub preferred_regions: Vec<String>,
    pub max_latency_ms: Option<u32>,
    pub require_trusted_execution: bool,
}

/// Deployment manifest (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentManifestMsg {
    pub app_name: String,
    pub wasm_hash: Vec<u8>,
    pub requirements: ResourceRequirementsMsg,
    pub replicas: u32,
    pub env_vars: Vec<(String, String)>,
}

/// Provider list request (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderListRequestMsg {
    pub requester: NodeId,
    pub timestamp: u64,
    pub filter: Option<ProviderFilterMsg>,
}

/// Provider filter for discovery (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProviderFilterMsg {
    pub min_cpu_cores: Option<u32>,
    pub min_memory_gb: Option<u32>,
    pub min_storage_gb: Option<u32>,
    pub required_arch: Option<Vec<String>>,
    pub required_features: Option<Vec<String>>,
    pub min_reputation: Option<u8>,
    pub max_price_cpu: Option<u64>,
}

/// Provider list response (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderListResponseMsg {
    pub providers: Vec<ProviderAdvertisementMsg>,
    pub timestamp: u64,
}

/// Deployment offer (from client to provider) (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentOfferMsg {
    pub deployment_id: DeploymentId,
    pub manifest: DeploymentManifestMsg,
    pub cluster_id: ClusterId,
    pub invitation_token: String,
    pub expires_at: u64,
}

/// Provider acceptance response (network protocol version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentAcceptanceMsg {
    pub deployment_id: DeploymentId,
    pub provider_node_id: NodeId,
    pub accepted: bool,
    pub reason: Option<String>,
    pub timestamp: u64,
}
