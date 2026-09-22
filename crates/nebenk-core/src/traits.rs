use crate::error::Result;
use crate::types::{ApplyResult, OperationEnvelope, Snapshot};

/// Trait implemented by application execution engines (e.g. WASM guest, native mock).
pub trait StateEngine: Send {
    /// Initialize the state engine with application-specific configuration.
    fn init(&mut self, config: &[u8]) -> Result<()>;

    /// Apply an operation deterministically to update the in-memory state machine.
    fn apply(&mut self, op: &OperationEnvelope) -> Result<ApplyResult>;

    /// Generate a serialized snapshot of the current state.
    fn snapshot(&mut self) -> Result<Vec<u8>>;

    /// Restore internal state from a serialized snapshot.
    fn restore(&mut self, revision: u64, snapshot_data: &[u8]) -> Result<()>;

    /// Execute a read-only query against current state.
    fn query(&self, query_payload: &[u8]) -> Result<Vec<u8>>;

    /// Returns the currently active revision.
    fn current_revision(&self) -> u64;
}

/// Trait for durable write-ahead logging of state operations.
pub trait LogStore: Send {
    /// Append an operation to the persistent write-ahead log.
    fn append_operation(&mut self, op: &OperationEnvelope) -> Result<()>;

    /// Retrieve a specific operation by its monotonic revision.
    fn get_operation(&self, revision: u64) -> Result<Option<OperationEnvelope>>;

    /// Retrieve a batch of operations beginning strictly after `revision`.
    fn get_operations_since(&self, revision: u64, limit: usize) -> Result<Vec<OperationEnvelope>>;

    /// Returns the highest revision recorded in the log, or 0 if empty.
    fn latest_revision(&self) -> Result<u64>;

    /// Truncate log entries up to the specified revision after taking a snapshot.
    fn truncate_prefix(&mut self, up_to_revision: u64) -> Result<()>;
}

/// Trait for persistent storage and retrieval of application state snapshots.
pub trait SnapshotStore: Send {
    /// Save a snapshot to durable storage.
    fn save_snapshot(&mut self, snapshot: &Snapshot) -> Result<()>;

    /// Retrieve the most recent snapshot available.
    fn get_latest_snapshot(&self) -> Result<Option<Snapshot>>;

    /// Retrieve a snapshot for a specific revision if available.
    fn get_snapshot_at(&self, revision: u64) -> Result<Option<Snapshot>>;
}
