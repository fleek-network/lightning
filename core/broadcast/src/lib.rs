pub mod config;
pub(crate) mod inner;
pub mod pubsub;

#[cfg(test)]
mod tests;

use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use config::Config;
use dashmap::DashMap;
use inner::BroadcastInner;
use lightning_interfaces::broadcast::BroadcastInterface;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::broadcast::BroadcastFrame;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::Topic;
use lightning_interfaces::{
    ConfigConsumer,
    ConnectionPoolInterface,
    ListenerConnector,
    ListenerInterface,
    NotifierInterface,
    WithStartAndShutdown,
};
use pubsub::PubSubTopic;
use tokio::select;
use tracing::error;

#[allow(clippy::type_complexity)]
pub struct Broadcast<C: Collection> {
    notifier: c![C::NotifierInterface],
    inner: BroadcastInner<C>,
    shutdown_signal: Arc<std::sync::RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
    listener:
        Arc<std::sync::Mutex<Option<c![C::ConnectionPoolInterface::Listener<BroadcastFrame>]>>>,
    /// Map of topic channel senders for incoming payloads
    channels: Arc<DashMap<Topic, tokio::sync::broadcast::Sender<Vec<u8>>>>,
    /// Sender for outgoing payloads, cloned and given to pubsub instances.
    sender: tokio::sync::mpsc::Sender<(Topic, Vec<u8>)>,
    /// Receiver for outgoing payloads
    receiver: Arc<std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<(Topic, Vec<u8>)>>>>,
    collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Broadcast<C> {
    const KEY: &'static str = "broadcast";

    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Broadcast<C> {
    fn is_running(&self) -> bool {
        self.shutdown_signal
            .read()
            .expect("failed to aquire lock")
            .is_some()
    }

    async fn start(&self) {
        // No-op if we're already running
        if self.shutdown_signal.read().unwrap().is_some() {
            return;
        }

        // initiate connections
        self.inner.apply_topology().await;

        // setup notifier
        let notifier = self.notifier.clone();
        let (epoch_tx, mut epoch_rx) = tokio::sync::mpsc::channel(1);
        notifier.notify_on_new_epoch(epoch_tx.clone());

        // setup shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self
            .shutdown_signal
            .write()
            .expect("failed to aquire shutdown lock") = Some(shutdown_tx);

        let inner = self.inner.clone();
        let mut listener = self.listener.lock().unwrap().take().unwrap();
        let mut outgoing = self.receiver.lock().unwrap().take().unwrap();

        // Spawn the main loop
        tokio::spawn(async move {
            loop {
                select! {
                    // Incoming connection
                    res = listener.accept() => {
                        let Some(conn) = res else {
                            // The listener should only return none here if it shuts down
                            break
                        };
                        if let Err(e) = inner.handle_connection(conn.1, conn.0).await {
                            error!("Error handling broadcast connection: {e}");
                        }
                    }
                    // Outgoing messages
                    message = outgoing.recv() => {
                        if let Some((topic, payload)) = message {
                            if let Err(e) = inner.broadcast(topic, payload).await {
                                error!("Failed to broadcast message: {e}");
                            }
                        }
                    }
                    _ = epoch_rx.recv() => {
                        // renew sender for next epoch
                        notifier.notify_on_new_epoch(epoch_tx.clone());
                        inner.apply_topology().await;
                    }
                    // Shutdown signal
                    _ = &mut shutdown_rx => break,
                }
            }
        });
    }

    async fn shutdown(&self) {
        if let Some(tx) = self
            .shutdown_signal
            .write()
            .expect("failed to aquire shutdown lock")
            .take()
        {
            tx.send(()).expect("failed to send shutdown signal")
        }
    }
}

#[async_trait]
impl<C: Collection> BroadcastInterface<C> for Broadcast<C> {
    type PubSub<M: LightningMessage + Clone> = PubSubTopic<M>;
    type Message = BroadcastFrame;

    fn init(
        _config: Self::Config,
        (listener, connector): ListenerConnector<C, c![C::ConnectionPoolInterface], Self::Message>,
        topology: c!(C::TopologyInterface),
        signer: &c!(C::SignerInterface),
        notifier: c!(C::NotifierInterface),
    ) -> Result<Self> {
        let channels = Arc::new(DashMap::new());
        let inner = BroadcastInner::new(topology, signer, connector, channels.clone());
        let (sender, receiver) = tokio::sync::mpsc::channel(256);
        Ok(Self {
            inner,
            notifier,
            shutdown_signal: std::sync::RwLock::new(None).into(),
            listener: std::sync::Mutex::new(Some(listener)).into(),
            channels,
            sender,
            receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
            collection: PhantomData,
        })
    }

    fn get_pubsub<M: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<M> {
        PubSubTopic::new(
            topic,
            self.sender.clone(),
            self.channels
                .entry(topic)
                .or_insert_with(|| tokio::sync::broadcast::channel(256).0)
                .subscribe(),
            self.channels.clone(),
        )
    }
}
