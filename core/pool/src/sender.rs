use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::SenderInterface;
use quinn::SendStream;

/// The sender on this stream.
pub struct Sender<T> {
    /// The peer's public key.
    peer: NodePublicKey,
    // Todo: Fix.
    // SendStream needs to be mutable to send which conflicts with interface.
    /// QUIC send stream.
    send: Arc<Mutex<Option<SendStream>>>,
    _marker: PhantomData<T>,
}

impl<T> Sender<T>
where
    T: LightningMessage,
{
    pub fn new(send: SendStream, peer: NodePublicKey) -> Self {
        Self {
            peer,
            send: Arc::new(Mutex::new(Some(send))),
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<T> SenderInterface<T> for Sender<T>
where
    T: LightningMessage,
{
    fn pk(&self) -> &NodePublicKey {
        &self.peer
    }

    async fn send(&self, msg: T) -> bool {
        // See comment in sender about why we use locks.
        let mut send = self.send.lock().unwrap().take().unwrap();
        let mut writer = Vec::new();
        let result = match msg.encode::<Vec<_>>(writer.as_mut()) {
            Ok(_) => {
                let write_result = send.write(&writer).await.is_err();
                !write_result
            },
            Err(_) => false,
        };
        self.send.lock().unwrap().replace(send);
        result
    }
}
