use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use futures_util::StreamExt;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::ReceiverInterface;
use quinn::RecvStream;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

/// The receiver on this stream.
pub struct Receiver<T> {
    /// The peer's public key.
    peer: NodePublicKey,
    /// QUIC receive stream.
    receive: FramedRead<RecvStream, LengthDelimitedCodec>,
    _marker: PhantomData<T>,
}

impl<T> Receiver<T>
where
    T: LightningMessage,
{
    pub fn new(receive: RecvStream, peer: NodePublicKey) -> Self {
        Self {
            peer,
            receive: FramedRead::new(receive, LengthDelimitedCodec::new()),
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<T> ReceiverInterface<T> for Receiver<T>
where
    T: LightningMessage,
{
    fn pk(&self) -> &NodePublicKey {
        &self.peer
    }

    async fn recv(&mut self) -> Option<T> {
        let buf = match self.receive.next().await? {
            Ok(buf) => buf,
            Err(e) => {
                tracing::error!("failed to decode length delimited codec: {e:?}");
                return None;
            },
        };

        match T::decode(&buf) {
            Ok(message) => Some(message),
            Err(e) => {
                tracing::error!("failed to decode: {e:?}");
                None
            },
        }
    }
}
