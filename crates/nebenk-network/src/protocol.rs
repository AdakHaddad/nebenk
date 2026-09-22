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
    /// Periodic liveness ping.
    Heartbeat {
        node_id: NodeId,
        revision: u64,
        timestamp_ms: u64,
    },
    /// Acknowledgment of heartbeat.
    HeartbeatAck {
        node_id: NodeId,
        timestamp_ms: u64,
    },
    /// Request catch-up state starting from a replica's last known revision.
    SyncRequest {
        last_known_revision: u64,
    },
    /// State catch-up response containing optional snapshot and missing operations.
    SyncResponse {
        latest_revision: u64,
        snapshot: Option<Snapshot>,
        operations: Vec<OperationEnvelope>,
    },
    /// Live operation streaming from primary to replicas.
    ReplicationOp {
        envelope: OperationEnvelope,
    },
    /// Confirmation of operation received and written to local storage.
    ReplicationAck {
        revision: u64,
        node_id: NodeId,
    },
}
