use nebenk_core::error::{NebenkError, Result};
use nebenk_core::traits::{LogStore, SnapshotStore, StateEngine};
use nebenk_core::types::{
    ApplyResult, ClusterId, NodeId, OperationEnvelope, Snapshot, SnapshotHeader,
};
use nebenk_identity::NodeIdentity;
use nebenk_storage::SqliteStore;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct NodeConfig {
    pub cluster_id: ClusterId,
    pub is_primary: bool,
    pub snapshot_interval: u64,
}

/// The core single-node runtime coordinator managing persistence, state transitions,
/// and crash/restart recovery.
pub struct NodeRuntime {
    identity: Arc<NodeIdentity>,
    config: NodeConfig,
    storage: Arc<Mutex<SqliteStore>>,
    engine: Box<dyn StateEngine>,
    ops_since_last_snapshot: u64,
}

impl NodeRuntime {
    pub fn new(
        identity: NodeIdentity,
        config: NodeConfig,
        storage: SqliteStore,
        engine: Box<dyn StateEngine>,
    ) -> Self {
        Self {
            identity: Arc::new(identity),
            config,
            storage: Arc::new(Mutex::new(storage)),
            engine,
            ops_since_last_snapshot: 0,
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.identity.node_id()
    }

    pub fn cluster_id(&self) -> &ClusterId {
        &self.config.cluster_id
    }

    pub fn current_revision(&self) -> u64 {
        self.engine.current_revision()
    }

    /// Recover state on node startup by restoring the latest snapshot and replaying the WAL.
    pub async fn recover(&mut self) -> Result<u64> {
        info!("Starting node recovery for cluster {}", self.config.cluster_id);
        let storage = self.storage.lock().await;

        let mut start_rev = 0;
        if let Some(snapshot) = storage.get_latest_snapshot()? {
            if !snapshot.verify_hash() {
                return Err(NebenkError::StateHashMismatch {
                    expected: hex::encode(snapshot.header.state_hash),
                    actual: hex::encode(Snapshot::compute_hash(&snapshot.data)),
                });
            }
            start_rev = snapshot.header.revision;
            info!("Restoring from snapshot at revision {}", start_rev);
            self.engine.restore(start_rev, &snapshot.data)?;
        }

        // Replay any subsequent operations from WAL
        let pending_ops = storage.get_operations_since(start_rev, usize::MAX)?;
        let count = pending_ops.len();
        for op in pending_ops {
            let digest = op.digest_for_signing();
            if !op.signature.is_empty() {
                NodeIdentity::verify(&op.author_node, &digest, &op.signature)?;
            }
            self.engine.apply(&op)?;
        }

        info!(
            "Recovery complete. Restored {} operations. Current revision: {}",
            count,
            self.engine.current_revision()
        );
        Ok(self.engine.current_revision())
    }

    /// Submit and execute a new state-changing command.
    pub async fn submit_command(
        &mut self,
        payload: Vec<u8>,
        timestamp_ms: u64,
    ) -> Result<ApplyResult> {
        let next_rev = self.engine.current_revision() + 1;
        let mut op = OperationEnvelope::new(
            next_rev,
            self.config.cluster_id.clone(),
            self.identity.node_id(),
            timestamp_ms,
            payload,
            Vec::new(),
        );

        // Sign the operation
        let digest = op.digest_for_signing();
        op.signature = self.identity.sign(&digest);

        // 1. Commit to persistent storage (WAL)
        {
            let mut storage = self.storage.lock().await;
            storage.append_operation(&op)?;
        }

        // 2. Apply to deterministic state machine
        let result = self.engine.apply(&op)?;
        self.ops_since_last_snapshot += 1;

        // 3. Periodic snapshot check
        if self.config.snapshot_interval > 0
            && self.ops_since_last_snapshot >= self.config.snapshot_interval
        {
            if let Err(e) = self.create_snapshot_internal(timestamp_ms).await {
                warn!("Failed to create periodic snapshot: {e}");
            }
        }

        Ok(result)
    }

    /// Manually trigger a snapshot of current state.
    pub async fn create_snapshot(&mut self, timestamp_ms: u64) -> Result<Snapshot> {
        self.create_snapshot_internal(timestamp_ms).await
    }

    async fn create_snapshot_internal(&mut self, timestamp_ms: u64) -> Result<Snapshot> {
        let rev = self.engine.current_revision();
        let data = self.engine.snapshot()?;
        let state_hash = Snapshot::compute_hash(&data);

        let header = SnapshotHeader {
            format_version: 1,
            cluster_id: self.config.cluster_id.clone(),
            revision: rev,
            state_hash,
            timestamp_ms,
            size_bytes: data.len() as u64,
        };

        let snapshot = Snapshot::new(header, data);
        let mut storage = self.storage.lock().await;
        storage.save_snapshot(&snapshot)?;
        self.ops_since_last_snapshot = 0;
        info!("Saved snapshot for revision {}", rev);

        Ok(snapshot)
    }

    /// Query the application state.
    pub fn query(&self, query_payload: &[u8]) -> Result<Vec<u8>> {
        self.engine.query(query_payload)
    }
}

// Minimal hex encoding helper for error messages without extra crate dependency
mod hex {
    pub fn encode(bytes: [u8; 32]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_wasm::{KvCommand, KvQuery, KvQueryResult, NativeKvEngine};

    #[tokio::test]
    async fn test_runtime_command_apply_and_recovery() {
        let identity = NodeIdentity::generate();
        let cluster = ClusterId::new("test-cluster");
        let storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
        let engine = Box::new(NativeKvEngine::new());

        let config = NodeConfig {
            cluster_id: cluster.clone(),
            is_primary: true,
            snapshot_interval: 2, // Snapshot every 2 operations
        };

        let mut runtime = NodeRuntime::new(identity, config, storage, engine);
        runtime.recover().await.unwrap();

        // 1. Submit PUT key1
        let cmd1 = KvCommand::Put {
            key: "key1".into(),
            value: b"val1".to_vec(),
        };
        let payload1 = serde_json::to_vec(&cmd1).unwrap();
        let res1 = runtime.submit_command(payload1, 1000).await.unwrap();
        assert_eq!(res1.revision, 1);

        // 2. Submit PUT key2 (should trigger snapshot at rev 2)
        let cmd2 = KvCommand::Put {
            key: "key2".into(),
            value: b"val2".to_vec(),
        };
        let payload2 = serde_json::to_vec(&cmd2).unwrap();
        let res2 = runtime.submit_command(payload2, 1001).await.unwrap();
        assert_eq!(res2.revision, 2);

        // 3. Submit PUT key3 (rev 3, in log after snapshot)
        let cmd3 = KvCommand::Put {
            key: "key3".into(),
            value: b"val3".to_vec(),
        };
        let payload3 = serde_json::to_vec(&cmd3).unwrap();
        let res3 = runtime.submit_command(payload3, 1002).await.unwrap();
        assert_eq!(res3.revision, 3);

        // Verify query
        let q = KvQuery::Get { key: "key2".into() };
        let q_bytes = serde_json::to_vec(&q).unwrap();
        let q_out = runtime.query(&q_bytes).unwrap();
        let val: KvQueryResult = serde_json::from_slice(&q_out).unwrap();
        match val {
            KvQueryResult::Value(Some(v)) => assert_eq!(v, b"val2"),
            _ => panic!("Expected val2"),
        }
    }
}
