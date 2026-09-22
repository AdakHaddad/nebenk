use nebenk_core::error::{NebenkError, Result};
use nebenk_core::traits::StateEngine;
use nebenk_core::types::{ApplyResult, OperationEnvelope, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KvCommand {
    Put { key: String, value: Vec<u8> },
    Delete { key: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KvQuery {
    Get { key: String },
    ListKeys,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KvQueryResult {
    Value(Option<Vec<u8>>),
    Keys(Vec<String>),
}

/// A reference in-memory Key-Value state engine implementing `StateEngine`.
/// This serves as the reference implementation for both testing and comparing
/// against the sandboxed WASM guest.
pub struct NativeKvEngine {
    state: BTreeMap<String, Vec<u8>>,
    current_rev: u64,
}

impl Default for NativeKvEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeKvEngine {
    pub fn new() -> Self {
        Self {
            state: BTreeMap::new(),
            current_rev: 0,
        }
    }
}

impl StateEngine for NativeKvEngine {
    fn init(&mut self, _config: &[u8]) -> Result<()> {
        self.state.clear();
        self.current_rev = 0;
        Ok(())
    }

    fn apply(&mut self, op: &OperationEnvelope) -> Result<ApplyResult> {
        let expected_rev = self.current_rev + 1;
        if op.revision != expected_rev {
            return Err(NebenkError::InvalidRevision {
                expected: expected_rev,
                actual: op.revision,
            });
        }

        // Deserialize command using serde_json or cbor
        let cmd: KvCommand = serde_json::from_slice(&op.payload)
            .map_err(|e| NebenkError::Serialization(format!("Invalid KV command payload: {e}")))?;

        let output = match cmd {
            KvCommand::Put { key, value } => {
                let prev = self.state.insert(key, value);
                serde_json::to_vec(&prev).unwrap_or_default()
            }
            KvCommand::Delete { key } => {
                let prev = self.state.remove(&key);
                serde_json::to_vec(&prev).unwrap_or_default()
            }
        };

        self.current_rev = op.revision;
        let snapshot_bytes = self.snapshot()?;
        let state_hash = Snapshot::compute_hash(&snapshot_bytes);

        Ok(ApplyResult {
            revision: self.current_rev,
            output,
            state_hash,
        })
    }

    fn snapshot(&mut self) -> Result<Vec<u8>> {
        serde_json::to_vec(&self.state)
            .map_err(|e| NebenkError::Serialization(format!("Snapshot serialize error: {e}")))
    }

    fn restore(&mut self, revision: u64, snapshot_data: &[u8]) -> Result<()> {
        let restored: BTreeMap<String, Vec<u8>> = serde_json::from_slice(snapshot_data)
            .map_err(|e| NebenkError::Serialization(format!("Snapshot deserialize error: {e}")))?;

        self.state = restored;
        self.current_rev = revision;
        Ok(())
    }

    fn query(&self, query_payload: &[u8]) -> Result<Vec<u8>> {
        let query: KvQuery = serde_json::from_slice(query_payload)
            .map_err(|e| NebenkError::Serialization(format!("Invalid query payload: {e}")))?;

        let res = match query {
            KvQuery::Get { key } => KvQueryResult::Value(self.state.get(&key).cloned()),
            KvQuery::ListKeys => KvQueryResult::Keys(self.state.keys().cloned().collect()),
        };

        serde_json::to_vec(&res)
            .map_err(|e| NebenkError::Serialization(format!("Query result serialize error: {e}")))
    }

    fn current_revision(&self) -> u64 {
        self.current_rev
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::{ClusterId, NodeId};

    #[test]
    fn test_native_kv_state_transitions_and_snapshots() {
        let mut engine = NativeKvEngine::new();
        let cluster = ClusterId::new("test");
        let node = NodeId::from_bytes([0u8; 32]);

        let cmd1 = KvCommand::Put {
            key: "user".into(),
            value: b"Muqtada".to_vec(),
        };
        let op1 = OperationEnvelope::new(
            1,
            cluster.clone(),
            node,
            1000,
            serde_json::to_vec(&cmd1).unwrap(),
            vec![],
        );

        let res1 = engine.apply(&op1).unwrap();
        assert_eq!(res1.revision, 1);

        // Query state
        let query = KvQuery::Get { key: "user".into() };
        let q_res = engine.query(&serde_json::to_vec(&query).unwrap()).unwrap();
        let val: KvQueryResult = serde_json::from_slice(&q_res).unwrap();
        match val {
            KvQueryResult::Value(Some(v)) => assert_eq!(v, b"Muqtada"),
            _ => panic!("Expected value"),
        }

        // Snapshot and restore onto a fresh engine
        let snap = engine.snapshot().unwrap();
        let mut engine2 = NativeKvEngine::new();
        engine2.restore(1, &snap).unwrap();
        assert_eq!(engine2.current_revision(), 1);

        let q_res2 = engine2.query(&serde_json::to_vec(&query).unwrap()).unwrap();
        let val2: KvQueryResult = serde_json::from_slice(&q_res2).unwrap();
        match val2 {
            KvQueryResult::Value(Some(v)) => assert_eq!(v, b"Muqtada"),
            _ => panic!("Expected value"),
        }
    }
}
