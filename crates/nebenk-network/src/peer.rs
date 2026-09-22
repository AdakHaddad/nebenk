use crate::handshake::{perform_client_handshake, perform_server_handshake};
use crate::protocol::NetworkMessage;
use crate::transport::P2pFramedStream;
use nebenk_core::error::{NebenkError, Result};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_identity::NodeIdentity;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

/// An authenticated, active P2P connection to another node.
pub struct P2pPeer {
    pub peer_node_id: NodeId,
    stream: P2pFramedStream,
}

impl P2pPeer {
    pub async fn connect(
        addr: &str,
        identity: &NodeIdentity,
        cluster_id: &ClusterId,
    ) -> Result<Self> {
        let tcp = TcpStream::connect(addr)
            .await
            .map_err(|e| NebenkError::Network(format!("Failed to connect to {addr}: {e}")))?;

        let framed = P2pFramedStream::new(tcp);
        let result = perform_client_handshake(framed, identity, cluster_id).await?;

        Ok(Self {
            peer_node_id: result.peer_node_id,
            stream: result.stream,
        })
    }

    pub async fn send(&mut self, msg: &NetworkMessage) -> Result<()> {
        self.stream.send_message(msg).await
    }

    pub async fn recv(&mut self) -> Result<Option<NetworkMessage>> {
        self.stream.recv_message().await
    }
}

/// Listener for accepting incoming peer connections.
pub struct P2pListener {
    listener: TcpListener,
}

impl P2pListener {
    pub async fn bind(addr: &str) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| NebenkError::Network(format!("Failed to bind to {addr}: {e}")))?;
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener
            .local_addr()
            .map_err(|e| NebenkError::Network(format!("Failed to get local addr: {e}")))
    }

    pub async fn accept(
        &mut self,
        identity: &NodeIdentity,
        cluster_id: &ClusterId,
    ) -> Result<P2pPeer> {
        let (tcp, _) = self
            .listener
            .accept()
            .await
            .map_err(|e| NebenkError::Network(format!("Accept failed: {e}")))?;

        let framed = P2pFramedStream::new(tcp);
        let result = perform_server_handshake(framed, identity, cluster_id).await?;

        Ok(P2pPeer {
            peer_node_id: result.peer_node_id,
            stream: result.stream,
        })
    }
}
