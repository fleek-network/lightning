use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
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
use quinn::{Endpoint, RecvStream, SendStream, ServerConfig, TransportConfig};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::connector::{ConnectEvent, Connector};
use crate::driver::{ConnectorDriver, ListenerDriver};
use crate::listener::Listener;
use crate::{driver, tls};

/// QUIC connection pool.
pub struct ConnectionPool<C: Collection> {
    /// Sender of connect events for connector.
    connector_tx: mpsc::Sender<ConnectEvent>,
    /// Receiver of connect events for connector.
    connector_rx: Arc<Mutex<Option<mpsc::Receiver<ConnectEvent>>>>,
    /// The scopes currently active.
    active_scopes: Arc<DashMap<ServiceScope, ScopeHandle>>,
    /// QUIC endpoint.
    endpoint: Mutex<Option<Endpoint>>,
    /// Flag to indicate if pool is running.
    is_running: Arc<Mutex<bool>>,
    /// Pool config.
    config: PoolConfig,
    /// Set of spawned driver workers.
    drivers: Mutex<JoinSet<()>>,
    /// Query runner.
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    /// Signer provider.
    node_secret_key: NodeSecretKey,
    /// Reputation reporter.
    _rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    _marker: PhantomData<C>,
}

impl<C: Collection> ConnectionPool<C> {
    pub fn new(
        config: PoolConfig,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        node_secret_key: NodeSecretKey,
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> Self {
        let (connector_tx, connector_rx) = mpsc::channel(256);

        Self {
            connector_tx,
            connector_rx: Arc::new(Mutex::new(Some(connector_rx))),
            active_scopes: Arc::new(DashMap::new()),
            endpoint: Mutex::new(None),
            is_running: Arc::new(Mutex::new(false)),
            config,
            drivers: Mutex::new(JoinSet::new()),
            query_runner,
            node_secret_key,
            _rep_reporter: rep_reporter,
            _marker: PhantomData,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PoolConfig {
    pub address: SocketAddr,
}

impl PoolConfig {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:4200".parse().unwrap(),
        }
    }
}

impl<C: Collection> ConfigConsumer for ConnectionPool<C> {
    const KEY: &'static str = "pool";
    type Config = PoolConfig;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for ConnectionPool<C> {
    fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    async fn start(&self) {
        let tls_config =
            tls::make_server_config(&self.node_secret_key).expect("Secret key to be valid");
        let mut server_config = ServerConfig::with_crypto(Arc::new(tls_config));
        let mut transport_config = TransportConfig::default();
        transport_config.max_idle_timeout(Duration::from_secs(30).try_into().ok());
        transport_config.keep_alive_interval(Some(Duration::from_secs(10)));
        server_config.transport_config(Arc::new(transport_config));
        let endpoint = Endpoint::server(server_config, self.config.address).unwrap();
        self.endpoint.lock().unwrap().replace(endpoint.clone());

        let listener_driver = ListenerDriver::new(self.active_scopes.clone(), endpoint.clone());
        self.drivers
            .lock()
            .unwrap()
            .spawn(driver::start_listener_driver(listener_driver));

        let connector_rx = self.connector_rx.lock().unwrap().take().unwrap();
        let listener_driver =
            ConnectorDriver::new(connector_rx, endpoint, self.node_secret_key.clone());
        self.drivers
            .lock()
            .unwrap()
            .spawn(driver::start_connector_driver(listener_driver));

        *self.is_running.lock().unwrap() = true;
    }

    async fn shutdown(&self) {
        self.drivers.lock().unwrap().abort_all();
        *self.is_running.lock().unwrap() = false;
    }
}

impl<C: Collection> ConnectionPoolInterface<C> for ConnectionPool<C> {
    type Listener<T: LightningMessage> = Listener<T>;
    type Connector<T: LightningMessage> = Connector<c![C::ApplicationInterface::SyncExecutor], T>;

    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> Self {
        let (_, secret_key) = signer.get_sk();
        ConnectionPool::new(config, query_runner, secret_key, rep_reporter)
    }

    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage,
    {
        if let Some(handle) = self.active_scopes.get(&scope) {
            if handle.connector_active || !handle.listener_tx.is_closed() {
                panic!("{scope:?} is already active");
            }
        }

        // If we didn't panic, we are safe to create new listeners and connectors.
        let (connection_event_tx, connection_event_rx) = mpsc::channel(256);
        let new_handle = ScopeHandle {
            connector_active: true,
            listener_tx: connection_event_tx,
        };
        self.active_scopes.insert(scope, new_handle);

        (
            Listener::new(connection_event_rx),
            Connector::new(
                scope,
                self.connector_tx.clone(),
                self.active_scopes.clone(),
                self.query_runner.clone(),
            ),
        )
    }
}

/// State for the scope.
pub struct ScopeHandle {
    /// Indicates whether connector is active.
    pub connector_active: bool,
    /// Used to send new connection events to Listener.
    /// If this is closed, the listener was dropped.
    pub listener_tx: mpsc::Sender<(NodePublicKey, SendStream, RecvStream)>,
}
