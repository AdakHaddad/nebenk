use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a node in a NEBENK cluster, derived from its 32-byte public key.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_base58(&self) -> String {
        bs58::encode(&self.0).into_string()
    }

    pub fn from_base58(s: &str) -> Result<Self, bs58::decode::Error> {
        let mut buf = [0u8; 32];
        bs58::decode(s).onto(&mut buf)?;
        Ok(Self(buf))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.to_base58())
    }
}

/// Logical cluster identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClusterId(pub String);

impl ClusterId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClusterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A serialized operation recorded in the append-only write-ahead log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEnvelope {
    pub revision: u64,
    pub cluster_id: ClusterId,
    pub author_node: NodeId,
    pub timestamp_ms: u64,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

impl OperationEnvelope {
    pub fn new(
        revision: u64,
        cluster_id: ClusterId,
        author_node: NodeId,
        timestamp_ms: u64,
        payload: Vec<u8>,
        signature: Vec<u8>,
    ) -> Self {
        Self {
            revision,
            cluster_id,
            author_node,
            timestamp_ms,
            payload,
            signature,
        }
    }

    /// Computes the BLAKE3 digest of the operation body (excluding signature) for signing.
    pub fn digest_for_signing(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.revision.to_le_bytes());
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(self.author_node.as_bytes());
        hasher.update(&self.timestamp_ms.to_le_bytes());
        hasher.update(&self.payload);
        *hasher.finalize().as_bytes()
    }
}

/// Header containing metadata about an application state snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotHeader {
    pub format_version: u32,
    pub cluster_id: ClusterId,
    pub revision: u64,
    pub state_hash: [u8; 32],
    pub timestamp_ms: u64,
    pub size_bytes: u64,
}

/// Full snapshot containing header metadata and serialized application state bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub header: SnapshotHeader,
    pub data: Vec<u8>,
}

impl Snapshot {
    pub fn new(header: SnapshotHeader, data: Vec<u8>) -> Self {
        Self { header, data }
    }

    pub fn compute_hash(data: &[u8]) -> [u8; 32] {
        *blake3::hash(data).as_bytes()
    }

    pub fn verify_hash(&self) -> bool {
        Self::compute_hash(&self.data) == self.header.state_hash
    }
}

/// Result returned from applying an operation to the state machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyResult {
    pub revision: u64,
    pub output: Vec<u8>,
    pub state_hash: [u8; 32],
}
