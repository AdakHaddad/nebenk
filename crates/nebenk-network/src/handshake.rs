use crate::protocol::NetworkMessage;
use crate::transport::P2pFramedStream;
use nebenk_core::error::{NebenkError, Result};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_identity::NodeIdentity;
use rand::RngCore;

pub struct HandshakeResult {
    pub peer_node_id: NodeId,
    pub stream: P2pFramedStream,
}

pub async fn perform_client_handshake(
    mut stream: P2pFramedStream,
    identity: &NodeIdentity,
    cluster_id: &ClusterId,
) -> Result<HandshakeResult> {
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);

    let mut payload = Vec::new();
    payload.extend_from_slice(cluster_id.as_str().as_bytes());
    payload.extend_from_slice(&nonce);
    let signature = identity.sign(&payload);

    let handshake = NetworkMessage::Handshake {
        node_id: identity.node_id(),
        cluster_id: cluster_id.clone(),
        nonce,
        signature,
    };

    stream.send_message(&handshake).await?;

    match stream.recv_message().await? {
        Some(NetworkMessage::HandshakeAck {
            success: true,
            peer_node_id,
            ..
        }) => Ok(HandshakeResult {
            peer_node_id,
            stream,
        }),
        Some(NetworkMessage::HandshakeAck {
            success: false,
            error_message,
            ..
        }) => Err(NebenkError::Unauthorized(
            error_message.unwrap_or_else(|| "Handshake rejected by peer".into()),
        )),
        _ => Err(NebenkError::Network("Expected HandshakeAck response".into())),
    }
}

pub async fn perform_server_handshake(
    mut stream: P2pFramedStream,
    identity: &NodeIdentity,
    expected_cluster: &ClusterId,
) -> Result<HandshakeResult> {
    match stream.recv_message().await? {
        Some(NetworkMessage::Handshake {
            node_id,
            cluster_id,
            nonce,
            signature,
        }) => {
            if cluster_id != *expected_cluster {
                let ack = NetworkMessage::HandshakeAck {
                    success: false,
                    peer_node_id: identity.node_id(),
                    error_message: Some("Cluster mismatch".into()),
                };
                let _ = stream.send_message(&ack).await;
                return Err(NebenkError::Unauthorized(format!(
                    "Cluster ID mismatch: expected {}, got {}",
                    expected_cluster, cluster_id
                )));
            }

            let mut payload = Vec::new();
            payload.extend_from_slice(cluster_id.as_str().as_bytes());
            payload.extend_from_slice(&nonce);

            if let Err(e) = NodeIdentity::verify(&node_id, &payload, &signature) {
                let ack = NetworkMessage::HandshakeAck {
                    success: false,
                    peer_node_id: identity.node_id(),
                    error_message: Some(format!("Invalid signature: {e}")),
                };
                let _ = stream.send_message(&ack).await;
                return Err(NebenkError::Unauthorized(format!("Verification failed: {e}")));
            }

            let ack = NetworkMessage::HandshakeAck {
                success: true,
                peer_node_id: identity.node_id(),
                error_message: None,
            };
            stream.send_message(&ack).await?;

            Ok(HandshakeResult {
                peer_node_id: node_id,
                stream,
            })
        }
        _ => Err(NebenkError::Network(
            "Expected Handshake message from peer".into(),
        )),
    }
}
