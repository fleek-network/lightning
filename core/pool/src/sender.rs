use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::SenderInterface;
use quinn::{Connection, SendStream};

pub struct Sender<T> {
    peer: NodePublicKey,
    // Todo: Fix.
    // SendStream needs to be mutable to send which conflicts with interface.
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
            _marker: PhantomData::default(),
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
        let mut send = self.send.lock().unwrap().take().unwrap();
        let mut writer = Vec::new();
        match msg.encode::<Vec<_>>(writer.as_mut()) {
            Ok(_) => {
                let write_result = send.write(&writer).await.is_err();
                self.send.lock().unwrap().replace(send);
                write_result
            },
            Err(_) => false,
        }
    }
}
