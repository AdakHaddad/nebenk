use crate::protocol::NetworkMessage;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use nebenk_core::error::{NebenkError, Result};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub struct P2pFramedStream {
    framed: Framed<TcpStream, LengthDelimitedCodec>,
}

impl P2pFramedStream {
    pub fn new(stream: TcpStream) -> Self {
        let codec = LengthDelimitedCodec::builder()
            .max_frame_length(32 * 1024 * 1024) // 32MB max frame for snapshots
            .new_codec();
        Self {
            framed: Framed::new(stream, codec),
        }
    }

    pub async fn send_message(&mut self, msg: &NetworkMessage) -> Result<()> {
        let bytes = serde_json::to_vec(msg)
            .map_err(|e| NebenkError::Serialization(format!("Failed to serialize message: {e}")))?;

        self.framed
            .send(Bytes::from(bytes))
            .await
            .map_err(|e| NebenkError::Network(format!("Failed to send frame: {e}")))?;
        Ok(())
    }

    pub async fn recv_message(&mut self) -> Result<Option<NetworkMessage>> {
        match self.framed.next().await {
            Some(Ok(bytes)) => {
                let msg: NetworkMessage = serde_json::from_slice(&bytes).map_err(|e| {
                    NebenkError::Serialization(format!("Failed to deserialize frame: {e}"))
                })?;
                Ok(Some(msg))
            }
            Some(Err(e)) => Err(NebenkError::Network(format!("Frame read error: {e}"))),
            None => Ok(None),
        }
    }
}
