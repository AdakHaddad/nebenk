use nebenk_core::types::{ClusterId, NodeId, OperationEnvelope, Snapshot};
use serde::{Deserialize, Serialize};

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
