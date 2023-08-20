use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};

use affair::Worker;
use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    ConnectionPoolInterface,
    SignerInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use quinn::{Endpoint, RecvStream, SendStream, ServerConfig};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::connection::{ConnectorDriver, ListenerDriver, RegisterEvent};
use crate::connector::{ConnectEvent, Connector};
use crate::listener::Listener;
use crate::receiver::Receiver;
use crate::sender::Sender;
use crate::{connection, netkit};

pub struct ConnectionPool<C> {
    connector_tx: mpsc::Sender<ConnectEvent>,
    register_tx: mpsc::Sender<RegisterEvent>,
    connector_rx: Arc<Mutex<Option<mpsc::Receiver<ConnectEvent>>>>,
    register_rx: Arc<Mutex<Option<mpsc::Receiver<RegisterEvent>>>>,
    scope_handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    endpoint: Endpoint,
    is_running: Arc<Mutex<bool>>,
    drivers: Mutex<JoinSet<()>>,
    _marker: PhantomData<C>,
}

impl<C> ConnectionPool<C> {
    pub fn new(config: PoolConfig) -> Self {
        let address: SocketAddr = config.address;
        let tls_config = netkit::server_config();
        let server_config = ServerConfig::with_crypto(Arc::new(tls_config));
        let endpoint = Endpoint::server(server_config, address).unwrap();

        let (register_tx, register_rx) = mpsc::channel(256);
        let (connector_tx, connector_rx) = mpsc::channel(256);

        Self {
            connector_tx,
            connector_rx: Arc::new(Mutex::new(Some(connector_rx))),
            register_tx,
            register_rx: Arc::new(Mutex::new(Some(register_rx))),
            scope_handles: Arc::new(DashMap::new()),
            endpoint,
            is_running: Arc::new(Mutex::new(false)),
            drivers: Mutex::new(JoinSet::new()),
            _marker: PhantomData::default(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PoolConfig {
    address: SocketAddr,
}

impl PoolConfig {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:0".parse().unwrap(),
        }
    }
}

impl<C> ConfigConsumer for ConnectionPool<C> {
    const KEY: &'static str = "";
    type Config = PoolConfig;
}

#[async_trait]
impl<C> WithStartAndShutdown for ConnectionPool<C>
where
    C: Collection,
{
    fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    async fn start(&self) {
        let register_rx = self.register_rx.lock().unwrap().take().unwrap();
        let listener_driver = ListenerDriver::new(
            self.scope_handles.clone(),
            register_rx,
            self.endpoint.clone(),
        );
        self.drivers
            .lock()
            .unwrap()
            .spawn(connection::start_listener_driver(listener_driver));

        let connector_rx = self.connector_rx.lock().unwrap().take().unwrap();
        let listener_driver = ConnectorDriver::new(connector_rx, self.endpoint.clone());
        self.drivers
            .lock()
            .unwrap()
            .spawn(connection::start_connector_driver(listener_driver));

        *self.is_running.lock().unwrap() = true;
    }

    async fn shutdown(&self) {
        self.drivers.lock().unwrap().abort_all();
        *self.is_running.lock().unwrap() = false;
    }
}

impl<C> ConnectionPoolInterface<C> for ConnectionPool<C>
where
    C: Collection,
{
    type Listener<T: LightningMessage> = Listener<T>;
    type Connector<T: LightningMessage> = Connector<T>;

    fn init(
        config: Self::Config,
        _signer: &c!(C::SignerInterface),
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        ConnectionPool::new(config)
    }

    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage,
    {
        if let Some(handle) = self.scope_handles.get(&scope) {
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
        self.scope_handles.insert(scope, new_handle);

        (
            Listener::new(
                self.register_tx.clone(),
                connection_event_rx,
                self.scope_handles.clone(),
            ),
            Connector::new(scope, self.connector_tx.clone(), self.scope_handles.clone()),
        )
    }
}

pub struct ScopeHandle {
    /// Indicates whether connector is active.
    pub connector_active: bool,
    /// Used to send new connection events to Listener.
    pub listener_tx: mpsc::Sender<(NodePublicKey, SendStream, RecvStream)>,
}
