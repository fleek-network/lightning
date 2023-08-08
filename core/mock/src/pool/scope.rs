use std::{marker::PhantomData, sync::Arc};

use affair::{AsyncWorker, Socket};
use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{
    schema::LightningMessage, ConnectorInterface, ListenerInterface, SenderReceiver, ServiceScope,
    SignerInterface, SyncQueryRunnerInterface,
};
use tokio::sync::mpsc::{channel, Receiver};

use super::{
    connection::Sender,
    handle_raw_receiver,
    schema::ScopedFrame,
    transport::{GlobalMemoryTransport, MemoryConnection},
    ConnectionPool,
};

type ConnectorSocket<C, T> =
    Socket<NodePublicKey, Option<SenderReceiver<<C as ConnectorInterface<T>>::ConnectionPool, T>>>;

/// ConnectorWorker for each scope
pub struct ConnectorWorker<S, Q: SyncQueryRunnerInterface, T> {
    pub _x: PhantomData<(S, Q, T)>,
    pub node: NodePublicKey,
    pub scope: ServiceScope,
    pub transport: GlobalMemoryTransport<ScopedFrame>,
    pub incoming: Arc<DashMap<ServiceScope, tokio::sync::mpsc::Sender<NodePublicKey>>>,
    pub senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    pub receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
}

#[async_trait]
impl<S: SignerInterface + 'static, Q: SyncQueryRunnerInterface, T: LightningMessage + 'static>
    AsyncWorker for ConnectorWorker<S, Q, T>
{
    type Request = NodePublicKey;
    type Response = Option<SenderReceiver<ConnectionPool<S, Q>, T>>;

    /// Returns [`None`] if a connection for this scope already exists, or cannot be made.
    async fn handle(&mut self, key: Self::Request) -> Self::Response {
        // TODO: check if we have an existing outgoing channel
        if self.receivers.contains_key(&(key, self.scope)) || self.senders.contains_key(&key) {
            // TODO: should we error here?
            return None;
        }

        // create channel for incoming messages in the scope
        let (scope_tx, scope_rx) = channel(256);

        // get an existing sender or create a new connection
        let sender = match self.senders.get(&key) {
            Some(sender) => sender.clone(),
            None => {
                // connect to the node
                let MemoryConnection {
                    sender, receiver, ..
                } = self.transport.connect(self.node, key).await?;
                self.senders.insert(key, sender.clone());

                // spawn task for reading messages
                tokio::spawn(handle_raw_receiver(
                    self.node,
                    receiver,
                    self.incoming.clone(),
                    self.senders.clone(),
                    self.receivers.clone(),
                ));

                sender
            },
        };
        self.receivers.insert((key, self.scope), scope_tx);

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
pub struct Connector<S, Q, T>
where
    S: SignerInterface + 'static,
    Q: SyncQueryRunnerInterface,
    T: LightningMessage,
{
    _x: PhantomData<Q>,
    socket: ConnectorSocket<Self, T>,
}

impl<S, Q, T> Clone for Connector<S, Q, T>
where
    S: SignerInterface,
    Q: SyncQueryRunnerInterface,
    T: LightningMessage,
{
    fn clone(&self) -> Self {
        Self {
            _x: self._x,
            socket: self.socket.clone(),
        }
    }
}

impl<S, Q, T> Connector<S, Q, T>
where
    S: SignerInterface + 'static,
    Q: SyncQueryRunnerInterface,
    T: LightningMessage,
{
    pub fn new(socket: ConnectorSocket<Self, T>) -> Self {
        Self {
            _x: PhantomData,
            socket,
        }
    }
}

#[async_trait]
impl<S, Q, T> ConnectorInterface<T> for Connector<S, Q, T>
where
    S: SignerInterface + 'static,
    Q: SyncQueryRunnerInterface,
    T: LightningMessage,
{
    type ConnectionPool = ConnectionPool<S, Q>;

    async fn connect(
        &self,
        to: &fleek_crypto::NodePublicKey,
    ) -> Option<lightning_interfaces::SenderReceiver<Self::ConnectionPool, T>> {
        self.socket
            .run(*to)
            .await
            .expect("failed to read from socket")
    }
}

/// Listener for accepting new connections for a scope
pub struct Listener<S, Q, T>
where
    S: SignerInterface + 'static,
    Q: SyncQueryRunnerInterface,
    T: LightningMessage,
{
    _x: PhantomData<(S, Q, T)>,
    scope: ServiceScope,
    channel: Receiver<NodePublicKey>,
    senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
}

impl<S, Q, T> Listener<S, Q, T>
where
    S: SignerInterface,
    Q: SyncQueryRunnerInterface,
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
impl<S, Q, T> ListenerInterface<T> for Listener<S, Q, T>
where
    S: SignerInterface,
    Q: SyncQueryRunnerInterface,
    T: LightningMessage,
{
    type ConnectionPool = ConnectionPool<S, Q>;

    async fn accept(&mut self) -> Option<SenderReceiver<Self::ConnectionPool, T>> {
        let node = self.channel.recv().await?;

        // create new channel for incoming messages and insert it
        let (tx, receiver) = channel(256);
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
