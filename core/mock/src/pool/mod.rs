#![allow(clippy::type_complexity)]
//! Mocked connection pool, backed by tcp streams

pub mod connection;
pub mod schema;
pub mod scope;
pub mod transport;

use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use affair::{Executor, TokioSpawn};
use async_trait::async_trait;
use connection::Sender;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    ConnectionPoolInterface,
    ReputationAggregatorInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use scope::{Connector, Listener};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::channel;

use self::schema::ScopedFrame;
use self::scope::ConnectorWorker;
use self::transport::{GlobalMemoryTransport, MemoryConnection};

pub const CHANNEL_BUFFER_LEN: usize = 64;

pub struct ConnectionPool<C: Collection> {
    _p: PhantomData<C>,
    pubkey: NodePublicKey,
    shutdown_signal: Arc<Mutex<Option<Sender<()>>>>,
    transport: Arc<Mutex<Option<GlobalMemoryTransport<ScopedFrame>>>>,
    /// Senders for incoming scope connections
    incoming: Arc<DashMap<ServiceScope, tokio::sync::mpsc::Sender<NodePublicKey>>>,
    /// Senders for outgoing messages, used by individual [`connection::Sender`]s.
    senders: Arc<DashMap<NodePublicKey, tokio::sync::mpsc::Sender<ScopedFrame>>>,
    /// Senders for communicating with [`connection::Receiver`]s for incoming messages
    receivers: Arc<DashMap<(NodePublicKey, ServiceScope), tokio::sync::mpsc::Sender<Vec<u8>>>>,
    _rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {}

impl<C: Collection> ConfigConsumer for ConnectionPool<C> {
    const KEY: &'static str = "connection-pool";

    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for ConnectionPool<C> {
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

impl<C: Collection> ConnectionPool<C> {
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

impl<C: Collection> ConnectionPoolInterface<C> for ConnectionPool<C> {
    // Bounded Types
    type Connector<T: LightningMessage> = Connector<C, T>;
    type Listener<T: LightningMessage> = Listener<C, T>;

    fn init(
        _config: Self::Config,
        signer: &c!(C::SignerInterface),
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    ) -> Self {
        let pubkey = signer.get_ed25519_pk();
        Self {
            pubkey,
            _p: PhantomData,
            transport: Mutex::new(None).into(),
            incoming: DashMap::new().into(),
            shutdown_signal: Mutex::new(None).into(),
            senders: DashMap::new().into(),
            receivers: DashMap::new().into(),
            _rep_reporter: rep_reporter,
        }
    }

    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage + 'static,
    {
        let transport = self
            .transport
            .lock()
            .expect("failed to aquire lock on transport")
            .clone()
            .expect("transport not found, has `with_transport` been called?");

        let worker = ConnectorWorker::<C, T> {
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
        let connector = Connector::<C, T>::new(socket);

        // create a channel to notify the listener about new scoped connections
        let (tx, rx) = channel(CHANNEL_BUFFER_LEN);
        self.incoming.insert(scope, tx);
        let listener =
            Listener::<C, T>::new(scope, rx, self.senders.clone(), self.receivers.clone());

        (listener, connector)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::anyhow;
    use lightning_application::app::Application;
    use lightning_application::config::{Mode, StorageConfig};
    use lightning_interfaces::infu_collection::Collection;
    use lightning_interfaces::types::ServiceScope;
    use lightning_interfaces::{
        partial,
        ApplicationInterface,
        ConnectionPoolInterface,
        ConnectorInterface,
        ListenerInterface,
        ReceiverInterface,
        SenderInterface,
        SignerInterface,
        WithStartAndShutdown,
    };
    use lightning_schema::AutoImplSerde;
    use lightning_signer::Signer;
    use serde::{Deserialize, Serialize};

    use crate::pool::transport::GlobalMemoryTransport;
    use crate::pool::{Config, ConnectionPool};

    partial!(TestBinding {
        ApplicationInterface = Application<Self>;
        SignerInterface = Signer<Self>;
    });

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

        let application = Application::<TestBinding>::init(
            lightning_application::config::Config {
                mode: Mode::Test,
                genesis: None,
                storage: StorageConfig::InMemory,
                db_path: None,
                db_options: None,
            },
            Default::default(),
        )?;
        let query_runner = application.sync_query();

        // create pool a
        let signer_a = Signer::<TestBinding>::init(
            lightning_signer::Config {
                node_key_path: PathBuf::from("../test-utils/keys/test_node.pem")
                    .try_into()
                    .expect("Failed to resolve path"),
                consensus_key_path: PathBuf::from("../test-utils/keys/test_consensus.pem")
                    .try_into()
                    .expect("Failed to resolve path"),
            },
            query_runner.clone(),
        )
        .map_err(|e| anyhow!("{e:?}"))?;

        let mut pool_a = ConnectionPool::<TestBinding>::init(
            Config {},
            &signer_a,
            query_runner.clone(),
            Default::default(),
        );

        pool_a.with_transport(global_transport.clone());
        pool_a.start().await;
        let (mut listener_a, _) = pool_a.bind::<TestFrame>(ServiceScope::Debug);

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

        let signer_b = Signer::<TestBinding>::init(
            lightning_signer::Config {
                node_key_path: PathBuf::from("../test-utils/keys/test_node2.pem")
                    .try_into()
                    .expect("Failed to resolve path"),
                consensus_key_path: PathBuf::from("../test-utils/keys/test_consensus2.pem")
                    .try_into()
                    .expect("Failed to resolve path"),
            },
            query_runner.clone(),
        )?;

        let mut pool_b = ConnectionPool::<TestBinding>::init(
            Config {},
            &signer_b,
            query_runner,
            Default::default(),
        );
        pool_b.with_transport(global_transport.clone());
        pool_b.start().await;
        let (_, connector_b) = pool_b.bind::<TestFrame>(ServiceScope::Debug);
        let (sender, mut receiver) = connector_b
            .connect(&signer_a.get_ed25519_pk())
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
