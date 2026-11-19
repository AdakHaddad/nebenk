pub mod handshake;
pub mod marketplace;
pub mod peer;
pub mod protocol;
pub mod transport;

pub use handshake::{perform_client_handshake, perform_server_handshake, HandshakeResult};
pub use marketplace::{
    topics, MarketplaceBehaviour, MarketplaceBehaviourEvent, MarketplaceCommand, MarketplaceEvent,
    MarketplaceNetwork, MARKETPLACE_KAD_PROTOCOL,
};
pub use peer::{P2pListener, P2pPeer};
pub use protocol::{
    ClusterMember, ClusterPreferencesMsg, DeploymentAcceptanceMsg, DeploymentId,
    DeploymentManifestMsg, DeploymentOfferMsg, DeviceCapabilitiesMsg, GpuInfoMsg, Heartbeat,
    HeartbeatAck, NatTypeMsg, NetworkInfoMsg, NetworkMessage, NodeRole, NodeState, PricingModelMsg,
    ProviderAdvertisementMsg, ProviderFilterMsg, ProviderHeartbeatMsg, ProviderListRequestMsg,
    ProviderListResponseMsg, ReputationScoreMsg, ResourceRequirementsMsg, StakeInfoMsg,
};
pub use transport::P2pFramedStream;

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::ClusterId;
    use nebenk_identity::NodeIdentity;

    #[tokio::test]
    async fn test_p2p_listener_and_handshake() {
        let cluster = ClusterId::new("test-cluster");
        let id_server = NodeIdentity::generate();
        let id_client = NodeIdentity::generate();

        let mut listener = P2pListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let cluster_clone = cluster.clone();
        let expected_client_node_id = id_client.node_id();
        let server_handle = tokio::spawn(async move {
            let mut peer = listener.accept(&id_server, &cluster_clone).await.unwrap();
            assert_eq!(peer.peer_node_id, expected_client_node_id);

            // Receive ping, send pong
            if let Some(NetworkMessage::Heartbeat { revision, .. }) = peer.recv().await.unwrap() {
                assert_eq!(revision, 42);
                peer.send(&NetworkMessage::HeartbeatAck {
                    node_id: id_server.node_id(),
                    timestamp_ms: 2000,
                    members: vec![],
                })
                .await
                .unwrap();
            } else {
                panic!("Expected Heartbeat");
            }
        });

        let mut client_peer = P2pPeer::connect(&addr.to_string(), &id_client, &cluster)
            .await
            .unwrap();

        client_peer
            .send(&NetworkMessage::Heartbeat {
                node_id: id_client.node_id(),
                cluster_id: cluster.clone(),
                revision: 42,
                timestamp_ms: 1999,
                role: NodeRole::Replica,
                state: NodeState::Active,
            })
            .await
            .unwrap();

        if let Some(NetworkMessage::HeartbeatAck { timestamp_ms, .. }) =
            client_peer.recv().await.unwrap()
        {
            assert_eq!(timestamp_ms, 2000);
        } else {
            panic!("Expected HeartbeatAck");
        }

        server_handle.await.unwrap();
    }
}
