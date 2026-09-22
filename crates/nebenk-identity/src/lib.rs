use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use nebenk_core::error::{NebenkError, Result};
use nebenk_core::types::{ClusterId, NodeId};
use rand::rngs::OsRng;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Cryptographic identity for a NEBENK node.
pub struct NodeIdentity {
    signing_key: SigningKey,
}

impl NodeIdentity {
    /// Generate a brand new random Ed25519 node identity.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Load or create node identity at the specified file path.
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            Self::load_from_file(path)
        } else {
            let identity = Self::generate();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| NebenkError::Identity(format!("Failed to create key directory: {e}")))?;
            }
            identity.save_to_file(path)?;
            Ok(identity)
        }
    }

    /// Load identity from a file containing a 32-byte Ed25519 secret seed.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let bytes = fs::read(path)
            .map_err(|e| NebenkError::Identity(format!("Failed to read key file: {e}")))?;
        if bytes.len() != 32 {
            return Err(NebenkError::Identity(format!(
                "Invalid key file length: expected 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        let signing_key = SigningKey::from_bytes(&key_bytes);
        Ok(Self { signing_key })
    }

    /// Save identity secret bytes to file with restricted permissions (0600 on Unix).
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);

        let mut file = options
            .open(path)
            .map_err(|e| NebenkError::Identity(format!("Failed to create key file: {e}")))?;
        file.write_all(&self.signing_key.to_bytes())
            .map_err(|e| NebenkError::Identity(format!("Failed to write key file: {e}")))?;
        Ok(())
    }

    /// Return the corresponding verifying (public) key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Return the NodeId derived from this identity's public key.
    pub fn node_id(&self) -> NodeId {
        NodeId::from_bytes(self.verifying_key().to_bytes())
    }

    /// Sign an arbitrary message slice.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature = self.signing_key.sign(message);
        signature.to_bytes().to_vec()
    }

    /// Verify a signature against a given NodeId.
    pub fn verify(node_id: &NodeId, message: &[u8], signature_bytes: &[u8]) -> Result<()> {
        let verifying_key = VerifyingKey::from_bytes(node_id.as_bytes())
            .map_err(|e| NebenkError::Identity(format!("Invalid public key bytes for node: {e}")))?;
        if signature_bytes.len() != 64 {
            return Err(NebenkError::Identity(format!(
                "Invalid signature length: expected 64, got {}",
                signature_bytes.len()
            )));
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(signature_bytes);
        let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);
        verifying_key
            .verify_strict(message, &signature)
            .map_err(|e| NebenkError::Identity(format!("Signature verification failed: {e}")))?;
        Ok(())
    }

    /// Create an invitation token for a new node to join a cluster.
    pub fn create_invitation(
        &self,
        cluster_id: &ClusterId,
        expires_at_epoch_s: u64,
    ) -> String {
        let mut payload = Vec::new();
        payload.extend_from_slice(cluster_id.as_str().as_bytes());
        payload.extend_from_slice(&expires_at_epoch_s.to_le_bytes());
        let sig = self.sign(&payload);

        format!(
            "nebenk-invite://{}/{}/{}?sig={}",
            cluster_id.as_str(),
            expires_at_epoch_s,
            self.node_id().to_base58(),
            bs58::encode(&sig).into_string()
        )
    }

    /// Verify an invitation token against the author node's public key.
    pub fn verify_invitation(
        token: &str,
        current_epoch_s: u64,
    ) -> Result<(ClusterId, NodeId)> {
        let prefix = "nebenk-invite://";
        if !token.starts_with(prefix) {
            return Err(NebenkError::Identity("Invalid invitation format".into()));
        }
        let rest = &token[prefix.len()..];
        let parts: Vec<&str> = rest.split('?').collect();
        if parts.len() != 2 {
            return Err(NebenkError::Identity("Malformed invitation query".into()));
        }
        let path_parts: Vec<&str> = parts[0].split('/').collect();
        if path_parts.len() != 3 {
            return Err(NebenkError::Identity("Malformed invitation path".into()));
        }
        let cluster_id = ClusterId::new(path_parts[0]);
        let expires_at_epoch_s: u64 = path_parts[1]
            .parse()
            .map_err(|_| NebenkError::Identity("Invalid expiry timestamp in token".into()))?;
        let author_node = NodeId::from_base58(path_parts[2])
            .map_err(|e| NebenkError::Identity(format!("Invalid author node ID: {e}")))?;

        if current_epoch_s > expires_at_epoch_s {
            return Err(NebenkError::Identity("Invitation token has expired".into()));
        }

        let query = parts[1];
        let sig_encoded = query
            .strip_prefix("sig=")
            .ok_or_else(|| NebenkError::Identity("Missing signature parameter in token".into()))?;
        let sig_bytes = bs58::decode(sig_encoded)
            .into_vec()
            .map_err(|e| NebenkError::Identity(format!("Invalid base58 signature: {e}")))?;

        let mut payload = Vec::new();
        payload.extend_from_slice(cluster_id.as_str().as_bytes());
        payload.extend_from_slice(&expires_at_epoch_s.to_le_bytes());

        Self::verify(&author_node, &payload, &sig_bytes)?;
        Ok((cluster_id, author_node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_sign_verify() {
        let id = NodeIdentity::generate();
        let message = b"nebenk operation payload";
        let sig = id.sign(message);
        assert!(NodeIdentity::verify(&id.node_id(), message, &sig).is_ok());
    }

    #[test]
    fn test_invitation_lifecycle() {
        let id = NodeIdentity::generate();
        let cluster_id = ClusterId::new("prod-cluster");
        let token = id.create_invitation(&cluster_id, 2000);
        let (verified_cluster, author) =
            NodeIdentity::verify_invitation(&token, 1000).expect("valid token");
        assert_eq!(cluster_id, verified_cluster);
        assert_eq!(id.node_id(), author);

        // Test expired token
        assert!(NodeIdentity::verify_invitation(&token, 2500).is_err());
    }
}
