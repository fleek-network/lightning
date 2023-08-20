use std::marker::PhantomData;
use std::sync::Arc;

use affair::{AsyncWorker, Socket};
use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::{ConnectorInterface, ListenerInterface, SenderReceiver};
use tokio::sync::mpsc::{channel, Receiver};

use super::connection::{self, Sender};
use super::schema::ScopedFrame;
use super::transport::{GlobalMemoryTransport, MemoryConnection};
use super::{handle_raw_receiver, ConnectionPool, CHANNEL_BUFFER_LEN};

type ConnectorSocket<C, T> = Socket<NodePublicKey, Option<SenderReceiver<C, ConnectionPool<C>, T>>>;

/// ConnectorWorker for each scope
pub struct ConnectorWorker<C, T> {
    pub _x: PhantomData<(C, T)>,
    pub node: NodePublicKey,
    pub scope: ServiceScope,
    pub transport: GlobalMemoryTransport<ScopedFrame>,
    pub incoming: Arc<DashMap<ServiceScope, tokio::sync::mpsc::Sender<NodePublicKey>>>,
    pub senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    pub receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
}

#[async_trait]
impl<C: Collection, T: LightningMessage + 'static> AsyncWorker for ConnectorWorker<C, T> {
    type Request = NodePublicKey;
    type Response = Option<SenderReceiver<C, ConnectionPool<C>, T>>;

    /// Returns [`None`] if a connection for this scope already exists, or cannot be made.
    async fn handle(&mut self, key: Self::Request) -> Self::Response {
        // check if we have an existing outgoing channel
        if self.receivers.contains_key(&(key, self.scope)) || self.senders.contains_key(&key) {
            // TODO: should we error here, is there a need for there to be multiple connections per
            // scope?
            return None;
        }

        // create channel for incoming messages in the scope
        let (scope_tx, scope_rx) = channel(CHANNEL_BUFFER_LEN);

        // get an existing sender or create a new connection
        let sender = match self.senders.get(&key) {
            Some(sender) => {
                self.receivers.insert((key, self.scope), scope_tx);
                sender.clone()
            },
            None => {
                // connect to the node
                let MemoryConnection {
                    sender, receiver, ..
                } = self.transport.connect(self.node, key).await?;

                self.senders.insert(key, sender.clone());
                self.receivers.insert((key, self.scope), scope_tx);

                // spawn task for reading messages
                tokio::spawn(handle_raw_receiver(
                    key,
                    receiver,
                    self.incoming.clone(),
                    self.senders.clone(),
                    self.receivers.clone(),
                ));

                sender
            },
        };

        // send hello message
        sender
            .send(ScopedFrame::Hello { scope: self.scope })
            .await
            .expect("failed to send hello message");

        // create and return final sender and receiver pair.
        Some((
            Sender::new(self.scope, key, sender),
            super::connection::Receiver::new(key, scope_rx),
        ))
    }
}

/// Mock connector for initializing new connections for a scope
pub struct Connector<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    socket: ConnectorSocket<C, T>,
}

impl<C, T> Clone for Connector<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    fn clone(&self) -> Self {
        Self {
            socket: self.socket.clone(),
        }
    }
}

impl<C, T> Connector<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    pub fn new(socket: ConnectorSocket<C, T>) -> Self {
        Self { socket }
    }
}

#[async_trait]
impl<C, T> ConnectorInterface<T> for Connector<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    type Sender = connection::Sender<T>;
    type Receiver = connection::Receiver<T>;

    async fn connect(
        &self,
        to: &fleek_crypto::NodePublicKey,
    ) -> Option<lightning_interfaces::SenderReceiver<C, ConnectionPool<C>, T>> {
        self.socket
            .run(*to)
            .await
            .expect("failed to read from socket")
    }
}

/// Listener for accepting new connections for a scope
pub struct Listener<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    _x: PhantomData<(C, T)>,
    scope: ServiceScope,
    channel: Receiver<NodePublicKey>,
    senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
}

impl<C, T> Listener<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    pub fn new(
        scope: ServiceScope,
        channel: Receiver<NodePublicKey>,
        senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
        receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
    ) -> Self {
        Self {
            _x: PhantomData,
            scope,
            channel,
            senders,
            receivers,
        }
    }
}

#[async_trait]
impl<C, T> ListenerInterface<T> for Listener<C, T>
where
    C: Collection,
    T: LightningMessage,
{
    type Sender = connection::Sender<T>;
    type Receiver = connection::Receiver<T>;

    async fn accept(&mut self) -> Option<SenderReceiver<C, ConnectionPool<C>, T>> {
        let node = self.channel.recv().await?;

        // create new channel for incoming messages and insert it
        let (tx, receiver) = channel(CHANNEL_BUFFER_LEN);
        self.receivers.insert((node, self.scope), tx);
        let receiver = super::connection::Receiver::new(node, receiver);

        // clone a sender for messages to that node
        let sender = self
            .senders
            .get(&node)
            // TODO: should we attempt to reconnect?
            .expect("failed to find connection for scope")
            .clone();
        let sender = super::connection::Sender::new(self.scope, node, sender);

        Some((sender, receiver))
    }
}
