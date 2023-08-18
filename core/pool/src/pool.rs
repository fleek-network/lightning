use std::{marker::PhantomData, net::SocketAddr, sync::Arc};

use affair::Worker;
use async_trait::async_trait;
use lightning_interfaces::{
    schema::LightningMessage, types::ServiceScope, ConfigConsumer, ConnectionPoolInterface,
    SignerInterface, SyncQueryRunnerInterface, WithStartAndShutdown,
};
use quinn::{Endpoint, ServerConfig};

use crate::{connector::Connector, listener::Listener, netkit, receiver::Receiver, sender::Sender};

pub struct ConnectionPool<Q, S> {
    endpoint: Endpoint,
    _marker: PhantomData<(Q, S)>,
}

impl<Q, S> ConfigConsumer for ConnectionPool<Q, S>
where
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
{
    const KEY: &'static str = "";
    type Config = ();
}

#[async_trait]
impl<Q, S> WithStartAndShutdown for ConnectionPool<Q, S>
where
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
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

impl<Q, S> ConnectionPoolInterface for ConnectionPool<Q, S>
where
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
{
    type QueryRunner = Q;
    type Signer = S;
    type Listener<T: LightningMessage> = Listener<Q, S, T>;
    type Connector<T: LightningMessage> = Connector<Q, S, T>;
    type Sender<T: LightningMessage> = Sender<T>;
    type Receiver<T: LightningMessage> = Receiver<T>;

    fn init(_: Self::Config, _: &Self::Signer, _: Self::QueryRunner) -> Self {
        let address: SocketAddr = "0.0.0.0".parse().unwrap();
        let tls_config = netkit::server_config();
        let server_config = ServerConfig::with_crypto(Arc::new(tls_config));
        let endpoint = Endpoint::server(server_config, address).unwrap();
        Self {
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
