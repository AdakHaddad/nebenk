use thiserror::Error;

#[derive(Error, Debug)]
pub enum NebenkError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("WASM execution error: {0}")]
    Wasm(String),

    #[error("Identity / crypto error: {0}")]
    Identity(String),

    #[error("Replication error: {0}")]
    Replication(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid state transition: expected revision {expected}, got {actual}")]
    InvalidRevision { expected: u64, actual: u64 },

    #[error("Invalid state hash: expected {expected}, calculated {actual}")]
    StateHashMismatch { expected: String, actual: String },

    #[error("Node {0} is unauthorized or untrusted")]
    Unauthorized(String),

    #[error("Cluster error: {0}")]
    Cluster(String),

    #[error("General runtime error: {0}")]
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, NebenkError>;
