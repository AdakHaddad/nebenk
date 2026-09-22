pub mod sync;

pub use sync::{apply_replicated_op, apply_sync_response, handle_sync_request};

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::traits::{LogStore, StateEngine};
    use nebenk_core::types::{ClusterId, OperationEnvelope, Snapshot, SnapshotHeader};
    use nebenk_identity::NodeIdentity;
    use nebenk_network::NetworkMessage;
    use nebenk_storage::SqliteStore;
    use nebenk_wasm::{KvCommand, KvQuery, KvQueryResult, NativeKvEngine};

    #[test]
    fn test_sync_catchup_with_snapshot_and_subsequent_ops() {
        let cluster = ClusterId::new("test-cluster");
        let primary_id = NodeIdentity::generate();

        let mut primary_storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
        let mut primary_engine = NativeKvEngine::new();

        // 1. Primary applies 2 ops
        let cmd1 = KvCommand::Put {
            key: "k1".into(),
            value: b"v1".to_vec(),
        };
        let mut op1 = OperationEnvelope::new(
            1,
            cluster.clone(),
            primary_id.node_id(),
            1000,
            serde_json::to_vec(&cmd1).unwrap(),
            vec![],
        );
        op1.signature = primary_id.sign(&op1.digest_for_signing());
        primary_storage.append_operation(&op1).unwrap();
        primary_engine.apply(&op1).unwrap();

        // 2. Primary takes snapshot at rev 1
        let snap_data = primary_engine.snapshot().unwrap();
        let snap = Snapshot::new(
            SnapshotHeader {
                format_version: 1,
                cluster_id: cluster.clone(),
                revision: 1,
                state_hash: Snapshot::compute_hash(&snap_data),
                timestamp_ms: 1001,
                size_bytes: snap_data.len() as u64,
            },
            snap_data,
        );
        primary_storage.save_snapshot(&snap).unwrap();

        // 3. Primary applies op 2
        let cmd2 = KvCommand::Put {
            key: "k2".into(),
            value: b"v2".to_vec(),
        };
        let mut op2 = OperationEnvelope::new(
            2,
            cluster.clone(),
            primary_id.node_id(),
            1002,
            serde_json::to_vec(&cmd2).unwrap(),
            vec![],
        );
        op2.signature = primary_id.sign(&op2.digest_for_signing());
        primary_storage.append_operation(&op2).unwrap();
        primary_engine.apply(&op2).unwrap();

        // 4. Replica connects with last_known_revision = 0
        let sync_msg = handle_sync_request(&primary_storage, 0).unwrap();

        // 5. Replica applies sync response
        let mut replica_storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
        let mut replica_engine = NativeKvEngine::new();

        let final_rev =
            apply_sync_response(&mut replica_engine, &mut replica_storage, sync_msg).unwrap();
        assert_eq!(final_rev, 2);

        // Verify replica state matches primary
        let q = KvQuery::Get { key: "k2".into() };
        let q_bytes = serde_json::to_vec(&q).unwrap();
        let res_bytes = replica_engine.query(&q_bytes).unwrap();
        let res: KvQueryResult = serde_json::from_slice(&res_bytes).unwrap();
        match res {
            KvQueryResult::Value(Some(v)) => assert_eq!(v, b"v2"),
            _ => panic!("Expected v2 on replica"),
        }

        // 6. Primary streams live op 3
        let cmd3 = KvCommand::Put {
            key: "k3".into(),
            value: b"v3".to_vec(),
        };
        let mut op3 = OperationEnvelope::new(
            3,
            cluster,
            primary_id.node_id(),
            1003,
            serde_json::to_vec(&cmd3).unwrap(),
            vec![],
        );
        op3.signature = primary_id.sign(&op3.digest_for_signing());

        let rev3 =
            apply_replicated_op(&mut replica_engine, &mut replica_storage, op3).unwrap();
        assert_eq!(rev3, 3);

        // Query key 3 on replica
        let q3 = KvQuery::Get { key: "k3".into() };
        let q3_bytes = serde_json::to_vec(&q3).unwrap();
        let res3_bytes = replica_engine.query(&q3_bytes).unwrap();
        let res3: KvQueryResult = serde_json::from_slice(&res3_bytes).unwrap();
        match res3 {
            KvQueryResult::Value(Some(v)) => assert_eq!(v, b"v3"),
            _ => panic!("Expected v3 on replica"),
        }
    }
}
