use nebenk_core::error::{NebenkError, Result};
use nebenk_core::traits::{LogStore, SnapshotStore};
use nebenk_core::types::{ClusterId, NodeId, OperationEnvelope, Snapshot, SnapshotHeader};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct SqliteStore {
    conn: Connection,
    cluster_id: ClusterId,
}

impl SqliteStore {
    /// Open or create a SQLite database at the specified path.
    pub fn open<P: AsRef<Path>>(path: P, cluster_id: ClusterId) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| NebenkError::Storage(format!("Failed to open SQLite database: {e}")))?;

        let store = Self { conn, cluster_id };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory SQLite database (primarily for testing).
    pub fn open_in_memory(cluster_id: ClusterId) -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| NebenkError::Storage(format!("Failed to open in-memory SQLite: {e}")))?;

        let store = Self { conn, cluster_id };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;

                 CREATE TABLE IF NOT EXISTS metadata (
                     key TEXT PRIMARY KEY,
                     value BLOB NOT NULL
                 );

                 CREATE TABLE IF NOT EXISTS operation_log (
                     revision INTEGER PRIMARY KEY,
                     author_node_id BLOB NOT NULL,
                     timestamp_ms INTEGER NOT NULL,
                     payload BLOB NOT NULL,
                     signature BLOB NOT NULL
                 );

                 CREATE TABLE IF NOT EXISTS snapshots (
                     revision INTEGER PRIMARY KEY,
                     timestamp_ms INTEGER NOT NULL,
                     state_hash BLOB NOT NULL,
                     data BLOB NOT NULL
                 );",
            )
            .map_err(|e| NebenkError::Storage(format!("Failed to initialize schema: {e}")))?;
        Ok(())
    }

    pub fn set_metadata(&mut self, key: &str, value: &[u8]) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|e| NebenkError::Storage(format!("Failed to set metadata: {e}")))?;
        Ok(())
    }

    pub fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM metadata WHERE key = ?1")
            .map_err(|e| NebenkError::Storage(format!("Query prep failed: {e}")))?;
        let result = stmt
            .query_row(params![key], |row| row.get(0))
            .optional()
            .map_err(|e| NebenkError::Storage(format!("Failed to read metadata: {e}")))?;
        Ok(result)
    }
}

impl LogStore for SqliteStore {
    fn append_operation(&mut self, op: &OperationEnvelope) -> Result<()> {
        let author_bytes = op.author_node.as_bytes().to_vec();
        self.conn
            .execute(
                "INSERT INTO operation_log (revision, author_node_id, timestamp_ms, payload, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    op.revision as i64,
                    author_bytes,
                    op.timestamp_ms as i64,
                    op.payload,
                    op.signature
                ],
            )
            .map_err(|e| NebenkError::Storage(format!("Failed to append to WAL: {e}")))?;
        Ok(())
    }

    fn get_operation(&self, revision: u64) -> Result<Option<OperationEnvelope>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT revision, author_node_id, timestamp_ms, payload, signature
                 FROM operation_log WHERE revision = ?1",
            )
            .map_err(|e| NebenkError::Storage(format!("Query prep failed: {e}")))?;

        let cluster_id = self.cluster_id.clone();
        let result = stmt
            .query_row(params![revision as i64], |row| {
                let rev: i64 = row.get(0)?;
                let author_raw: Vec<u8> = row.get(1)?;
                let ts: i64 = row.get(2)?;
                let payload: Vec<u8> = row.get(3)?;
                let sig: Vec<u8> = row.get(4)?;

                let mut node_bytes = [0u8; 32];
                node_bytes.copy_from_slice(&author_raw);

                Ok(OperationEnvelope {
                    revision: rev as u64,
                    cluster_id: cluster_id.clone(),
                    author_node: NodeId::from_bytes(node_bytes),
                    timestamp_ms: ts as u64,
                    payload,
                    signature: sig,
                })
            })
            .optional()
            .map_err(|e| NebenkError::Storage(format!("Failed to read operation: {e}")))?;

        Ok(result)
    }

    fn get_operations_since(&self, revision: u64, limit: usize) -> Result<Vec<OperationEnvelope>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT revision, author_node_id, timestamp_ms, payload, signature
                 FROM operation_log WHERE revision > ?1 ORDER BY revision ASC LIMIT ?2",
            )
            .map_err(|e| NebenkError::Storage(format!("Query prep failed: {e}")))?;

        let cluster_id = self.cluster_id.clone();
        let rows = stmt
            .query_map(params![revision as i64, limit as i64], |row| {
                let rev: i64 = row.get(0)?;
                let author_raw: Vec<u8> = row.get(1)?;
                let ts: i64 = row.get(2)?;
                let payload: Vec<u8> = row.get(3)?;
                let sig: Vec<u8> = row.get(4)?;

                let mut node_bytes = [0u8; 32];
                node_bytes.copy_from_slice(&author_raw);

                Ok(OperationEnvelope {
                    revision: rev as u64,
                    cluster_id: cluster_id.clone(),
                    author_node: NodeId::from_bytes(node_bytes),
                    timestamp_ms: ts as u64,
                    payload,
                    signature: sig,
                })
            })
            .map_err(|e| NebenkError::Storage(format!("Failed to query operations since: {e}")))?;

        let mut ops = Vec::new();
        for row in rows {
            ops.push(row.map_err(|e| NebenkError::Storage(format!("Row read error: {e}")))?);
        }
        Ok(ops)
    }

    fn latest_revision(&self) -> Result<u64> {
        let mut stmt = self
            .conn
            .prepare("SELECT COALESCE(MAX(revision), 0) FROM operation_log")
            .map_err(|e| NebenkError::Storage(format!("Query prep failed: {e}")))?;

        let rev: i64 = stmt
            .query_row([], |row| row.get(0))
            .map_err(|e| NebenkError::Storage(format!("Failed to get latest revision: {e}")))?;

        Ok(rev as u64)
    }

    fn truncate_prefix(&mut self, up_to_revision: u64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM operation_log WHERE revision <= ?1",
                params![up_to_revision as i64],
            )
            .map_err(|e| NebenkError::Storage(format!("Failed to truncate log prefix: {e}")))?;
        Ok(())
    }
}

impl SnapshotStore for SqliteStore {
    fn save_snapshot(&mut self, snapshot: &Snapshot) -> Result<()> {
        let hash_vec = snapshot.header.state_hash.to_vec();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO snapshots (revision, timestamp_ms, state_hash, data)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    snapshot.header.revision as i64,
                    snapshot.header.timestamp_ms as i64,
                    hash_vec,
                    snapshot.data
                ],
            )
            .map_err(|e| NebenkError::Storage(format!("Failed to save snapshot: {e}")))?;
        Ok(())
    }

    fn get_latest_snapshot(&self) -> Result<Option<Snapshot>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT revision, timestamp_ms, state_hash, data
                 FROM snapshots ORDER BY revision DESC LIMIT 1",
            )
            .map_err(|e| NebenkError::Storage(format!("Query prep failed: {e}")))?;

        let cluster_id = self.cluster_id.clone();
        let result = stmt
            .query_row([], |row| {
                let rev: i64 = row.get(0)?;
                let ts: i64 = row.get(1)?;
                let hash_raw: Vec<u8> = row.get(2)?;
                let data: Vec<u8> = row.get(3)?;

                let mut state_hash = [0u8; 32];
                state_hash.copy_from_slice(&hash_raw);

                Ok(Snapshot {
                    header: SnapshotHeader {
                        format_version: 1,
                        cluster_id: cluster_id.clone(),
                        revision: rev as u64,
                        state_hash,
                        timestamp_ms: ts as u64,
                        size_bytes: data.len() as u64,
                    },
                    data,
                })
            })
            .optional()
            .map_err(|e| NebenkError::Storage(format!("Failed to query latest snapshot: {e}")))?;

        Ok(result)
    }

    fn get_snapshot_at(&self, revision: u64) -> Result<Option<Snapshot>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT revision, timestamp_ms, state_hash, data
                 FROM snapshots WHERE revision = ?1",
            )
            .map_err(|e| NebenkError::Storage(format!("Query prep failed: {e}")))?;

        let cluster_id = self.cluster_id.clone();
        let result = stmt
            .query_row(params![revision as i64], |row| {
                let rev: i64 = row.get(0)?;
                let ts: i64 = row.get(1)?;
                let hash_raw: Vec<u8> = row.get(2)?;
                let data: Vec<u8> = row.get(3)?;

                let mut state_hash = [0u8; 32];
                state_hash.copy_from_slice(&hash_raw);

                Ok(Snapshot {
                    header: SnapshotHeader {
                        format_version: 1,
                        cluster_id: cluster_id.clone(),
                        revision: rev as u64,
                        state_hash,
                        timestamp_ms: ts as u64,
                        size_bytes: data.len() as u64,
                    },
                    data,
                })
            })
            .optional()
            .map_err(|e| NebenkError::Storage(format!("Failed to query snapshot at revision: {e}")))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_log_and_snapshot_lifecycle() {
        let cluster = ClusterId::new("test-cluster");
        let mut store = SqliteStore::open_in_memory(cluster.clone()).unwrap();

        assert_eq!(store.latest_revision().unwrap(), 0);

        let node = NodeId::from_bytes([1u8; 32]);
        let op1 = OperationEnvelope::new(1, cluster.clone(), node, 1000, b"op1".to_vec(), vec![]);
        let op2 = OperationEnvelope::new(2, cluster.clone(), node, 1001, b"op2".to_vec(), vec![]);

        store.append_operation(&op1).unwrap();
        store.append_operation(&op2).unwrap();

        assert_eq!(store.latest_revision().unwrap(), 2);

        let retrieved = store.get_operation(1).unwrap().expect("should exist");
        assert_eq!(retrieved.payload, b"op1");

        let batch = store.get_operations_since(0, 10).unwrap();
        assert_eq!(batch.len(), 2);

        let snap_data = b"serialized snapshot data".to_vec();
        let snap = Snapshot::new(
            SnapshotHeader {
                format_version: 1,
                cluster_id: cluster,
                revision: 2,
                state_hash: Snapshot::compute_hash(&snap_data),
                timestamp_ms: 1002,
                size_bytes: snap_data.len() as u64,
            },
            snap_data,
        );

        store.save_snapshot(&snap).unwrap();
        let loaded = store.get_latest_snapshot().unwrap().expect("snapshot exists");
        assert_eq!(loaded.header.revision, 2);
        assert!(loaded.verify_hash());

        store.truncate_prefix(1).unwrap();
        assert!(store.get_operation(1).unwrap().is_none());
        assert!(store.get_operation(2).unwrap().is_some());
    }
}
