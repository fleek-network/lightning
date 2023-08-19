use std::collections::HashSet;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::Arc;

use affair::Worker;
use async_trait::async_trait;
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
use quinn::{Endpoint, ServerConfig};
use tokio::sync::{mpsc, Mutex};

use crate::connection::RegisterEvent;
use crate::connector::{ConnectEvent, Connector};
use crate::listener::Listener;
use crate::netkit;
use crate::receiver::Receiver;
use crate::sender::Sender;

pub struct ConnectionPool<C> {
    connector_tx: mpsc::Sender<ConnectEvent>,
    register_tx: mpsc::Sender<RegisterEvent>,
    connector_rx: Arc<Mutex<Option<mpsc::Receiver<ConnectEvent>>>>,
    register_rx: Arc<Mutex<Option<mpsc::Receiver<RegisterEvent>>>>,
    endpoint: Endpoint,
    _marker: PhantomData<C>,
}

impl<C> ConfigConsumer for ConnectionPool<C> {
    const KEY: &'static str = "";
    type Config = ();
}

#[async_trait]
impl<C> WithStartAndShutdown for ConnectionPool<C>
where
    C: Collection,
{
    fn is_running(&self) -> bool {
        todo!()
    }

    async fn start(&self) {
        todo!()
    }

    async fn shutdown(&self) {
        todo!()
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
        signer: &c!(C::SignerInterface),
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        let address: SocketAddr = "0.0.0.0".parse().unwrap();
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
            endpoint,
            _marker: PhantomData::default(),
        }
    }

    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage,
    {
        todo!()
    }
}
