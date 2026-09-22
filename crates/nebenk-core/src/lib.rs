pub mod error;
pub mod traits;
pub mod types;

pub use error::{NebenkError, Result};
pub use traits::{LogStore, SnapshotStore, StateEngine};
pub use types::{ApplyResult, ClusterId, NodeId, OperationEnvelope, Snapshot, SnapshotHeader};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_roundtrip() {
        let raw = [42u8; 32];
        let node_id = NodeId::from_bytes(raw);
        let encoded = node_id.to_base58();
        let decoded = NodeId::from_base58(&encoded).expect("valid base58");
        assert_eq!(node_id, decoded);
    }

    #[test]
    fn test_snapshot_hash_verification() {
        let data = b"test state payload".to_vec();
        let state_hash = Snapshot::compute_hash(&data);
        let header = SnapshotHeader {
            format_version: 1,
            cluster_id: ClusterId::new("test-cluster"),
            revision: 5,
            state_hash,
            timestamp_ms: 1000,
            size_bytes: data.len() as u64,
        };
        let snapshot = Snapshot::new(header, data);
        assert!(snapshot.verify_hash());
    }
}
