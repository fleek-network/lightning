#![allow(clippy::type_complexity)]
//! Mocked connection pool, backed by tcp streams

pub mod connection;
pub mod schema;
pub mod scope;
pub mod transport;

use std::{
    collections::HashSet,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use affair::{Executor, TokioSpawn};
use async_trait::async_trait;
use connection::{Receiver, Sender};
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{
    schema::LightningMessage, ConfigConsumer, ConnectionPoolInterface, ServiceScope,
    SignerInterface, SyncQueryRunnerInterface, WithStartAndShutdown,
};
use scope::{Connector, Listener};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::channel;

use self::{
    schema::ScopedFrame,
    scope::ConnectorWorker,
    transport::{GlobalMemoryTransport, MemoryConnection},
};

pub const CHANNEL_BUFFER_LEN: usize = 64;

pub struct ConnectionPool<S, Q: SyncQueryRunnerInterface> {
    _q: PhantomData<(S, Q)>,
    pubkey: NodePublicKey,
    shutdown_signal: Arc<Mutex<Option<Sender<()>>>>,
    transport: Arc<Mutex<Option<GlobalMemoryTransport<ScopedFrame>>>>,
    /// Senders for incoming scope connections
    incoming: Arc<DashMap<ServiceScope, tokio::sync::mpsc::Sender<NodePublicKey>>>,
    /// Senders for outgoing messages, used by individual [`connection::Sender`]s.
    senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    /// Senders for communicating with [`connection::Receiver`]s for incoming messages
    receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {}

impl<S, Q: SyncQueryRunnerInterface> ConfigConsumer for ConnectionPool<S, Q> {
    const KEY: &'static str = "connection-pool";

    type Config = Config;
}

#[async_trait]
impl<S: SignerInterface, Q: SyncQueryRunnerInterface> WithStartAndShutdown
    for ConnectionPool<S, Q>
{
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        self.shutdown_signal
            .lock()
            .expect("failed to aquire lock")
            .is_some()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    ///
    /// # Panics
    ///
    /// If [`Self::with_transport`] has not been called beforehand.
    async fn start(&self) {
        let (tx, mut rx) = channel(CHANNEL_BUFFER_LEN);
        let transport = self
            .transport
            .lock()
            .expect("failed to aquire lock on transport")
            .clone()
            .expect("transport not initialized, has `with_transport` been called?");
        transport
            .bind(self.pubkey, tx)
            .await
            .expect("failed to bind to transport");
        let incoming = self.incoming.clone();
        let receivers = self.receivers.clone();
        let senders = self.senders.clone();

        // spawn loop for accepting raw incoming connections
        tokio::spawn(async move {
            while let Some(MemoryConnection {
                sender,
                receiver,
                node,
            }) = rx.recv().await
            {
                // insert to senders
                if senders.contains_key(&node) {
                    // how should we handle duplicate connection requests?
                    eprintln!("warning: replaced existing connection sender");
                    continue;
                }
                senders.insert(node, sender);

                // spawn task for listening for incoming frames (hello and messages)
                let incoming = incoming.clone();
                let receivers = receivers.clone();
                let senders = senders.clone();
                tokio::spawn(handle_raw_receiver(
                    node, receiver, incoming, senders, receivers,
                ));
            }
        });
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

pub(crate) async fn handle_raw_receiver(
    node: NodePublicKey,
    mut receiver: tokio::sync::mpsc::Receiver<ScopedFrame>,
    incoming: Arc<DashMap<ServiceScope, tokio::sync::mpsc::Sender<NodePublicKey>>>,
    senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
) {
    let mut scopes = HashSet::new();
    while let Some(frame) = receiver.recv().await {
        match frame {
            ScopedFrame::Hello { scope } => {
                if let Some(sender) = incoming.get_mut(&scope) {
                    sender
                        .send(node)
                        .await
                        .expect("failed to send incoming scope to listener");
                    scopes.insert(scope);
                }
            },
            ScopedFrame::Message { scope, payload } => {
                // route the message to the receiver if it exists
                if let Some(sender) = receivers.get(&(node, scope)) {
                    sender
                        .send(payload)
                        .await
                        .expect("failed to send incoming message to receiver");
                } else {
                    eprintln!("warning: receiver not found for {scope:?}");
                }
            },
        }
    }

    // once connection is finished, remove the channels
    senders.remove(&node);
    for scope in scopes {
        receivers.remove(&(node, scope));
    }
}

impl<S, Q: SyncQueryRunnerInterface> ConnectionPool<S, Q> {
    /// Required for mock initialization, provides the pool with a global memory transport
    /// to bind and request new connections with.
    pub fn with_transport(&mut self, transport: GlobalMemoryTransport<ScopedFrame>) {
        let _ = self
            .transport
            .lock()
            .expect("failed to aquire lock")
            .insert(transport);
    }
}

impl<S: SignerInterface + 'static, Q: SyncQueryRunnerInterface + 'static> ConnectionPoolInterface
    for ConnectionPool<S, Q>
{
    type QueryRunner = Q;
    type Signer = S;

    // Bounded Types
    type Connector<T: LightningMessage> = Connector<S, Q, T>;
    type Listener<T: LightningMessage> = Listener<S, Q, T>;
    type Sender<T: LightningMessage> = Sender<T>;
    type Receiver<T: LightningMessage> = Receiver<T>;

    fn init(
        _config: Self::Config,
        signer: &Self::Signer,
        _query_runner: Self::QueryRunner,
    ) -> Self {
        let pubkey = signer.get_bls_pk();
        Self {
            pubkey,
            _q: PhantomData,
            transport: Mutex::new(None).into(),
            incoming: DashMap::new().into(),
            shutdown_signal: Mutex::new(None).into(),
            senders: DashMap::new().into(),
            receivers: DashMap::new().into(),
        }
    }

    fn bind<T>(
        &self,
        scope: lightning_interfaces::ServiceScope,
    ) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage + 'static,
    {
        let transport = self
            .transport
            .lock()
            .expect("failed to aquire lock on transport")
            .clone()
            .expect("transport not found, has `with_transport` been called?");

        let worker = ConnectorWorker::<S, Q, T> {
            _x: PhantomData,
            transport,
            scope,
            node: self.pubkey,
            incoming: self.incoming.clone(),
            senders: self.senders.clone(),
            receivers: self.receivers.clone(),
        };

        // get the connector socket and spawn a worker for getting new node connections
        let socket = TokioSpawn::spawn_async(worker);
        let connector = Connector::<S, Q, T>::new(socket);

        // create a channel to notify the listener about new scoped connections
        let (tx, rx) = channel(CHANNEL_BUFFER_LEN);
        self.incoming.insert(scope, tx);
        let listener =
            Listener::<S, Q, T>::new(scope, rx, self.senders.clone(), self.receivers.clone());

        (listener, connector)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use lightning_application::config::Mode;
    use lightning_interfaces::{
        ApplicationInterface, ConnectionPoolInterface, ConnectorInterface, ListenerInterface,
        ReceiverInterface, SenderInterface, ServiceScope, SignerInterface, WithStartAndShutdown,
    };
    use lightning_schema::AutoImplSerde;
    use lightning_signer::Signer;
    use serde::{Deserialize, Serialize};

    use crate::pool::{transport::GlobalMemoryTransport, Config, ConnectionPool};

    #[tokio::test]
    async fn ping_pong() -> anyhow::Result<()> {
        #[derive(Clone, Serialize, Deserialize)]
        enum TestFrame {
            Ping,
            Pong,
        }
        impl AutoImplSerde for TestFrame {}

        let global_transport = GlobalMemoryTransport::default();

        // we dont actually use query runner, so it doesn't really matter
        let application =
            lightning_application::app::Application::init(lightning_application::config::Config {
                mode: Mode::Test,
                genesis: None,
            })
            .await?;
        let query_runner = application.sync_query();

        // create pool a
        let signer_a = Signer::init(
            lightning_signer::Config {
                node_key_path: "../test-utils/keys/test_node.pem".into(),
                network_key_path: "../test-utils/keys/test_network.pem".into(),
            },
            query_runner.clone(),
        )
        .await
        .map_err(|e| anyhow!("{e:?}"))?;
        let mut pool_a = ConnectionPool::init(Config {}, &signer_a, query_runner.clone());
        pool_a.with_transport(global_transport.clone());
        pool_a.start().await;
        let (mut listener_a, _) = pool_a.bind::<TestFrame>(ServiceScope::Broadcast);

        let task = tokio::spawn(async move {
            // listen for a connection
            let (sender, mut receiver) = listener_a
                .accept()
                .await
                .expect("failed to accept a connection");

            // read a ping frame
            match receiver.recv().await.expect("connection closed") {
                TestFrame::Ping => {
                    // send a message
                    assert!(sender.send(TestFrame::Pong).await);
                },
                TestFrame::Pong => unreachable!(),
            }
        });

        let signer_b = Signer::init(
            lightning_signer::Config {
                node_key_path: "core/test-utils/keys/test_node2.pem".into(),
                network_key_path: "core/test-utils/keys/test_network2.pem".into(),
            },
            query_runner.clone(),
        )
        .await?;
        let mut pool_b = ConnectionPool::init(Config {}, &signer_b, query_runner);
        pool_b.with_transport(global_transport.clone());
        pool_b.start().await;
        let (_, connector_b) = pool_b.bind::<TestFrame>(ServiceScope::Broadcast);
        let (sender, mut receiver) = connector_b
            .connect(&signer_a.get_bls_pk())
            .await
            .expect("failed to connect to node a");

        assert!(sender.send(TestFrame::Ping).await);

        // read a pong frame
        match receiver.recv().await.expect("connection closed") {
            TestFrame::Pong => {},
            TestFrame::Ping => unreachable!(),
        }

        task.await?;
        Ok(())
    }
}
