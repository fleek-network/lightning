use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::ReceiverInterface;
use quinn::RecvStream;

pub struct Receiver<T> {
    peer: NodePublicKey,
    receive: RecvStream,
    _marker: PhantomData<T>,
}

impl<T> Receiver<T>
where
    T: LightningMessage,
{
    pub fn new(receive: RecvStream, peer: NodePublicKey) -> Self {
        Self {
            peer,
            receive,
            _marker: PhantomData::default(),
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
        let mut buf = Vec::with_capacity(4096);
        self.receive.read(buf.as_mut()).await.ok()?;
        match T::decode(&buf) {
            Ok(message) => Some(message),
            Err(e) => {
                tracing::error!("failed to decode: {e:?}");
                None
            },
        }
    }
}
