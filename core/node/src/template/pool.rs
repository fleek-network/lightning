use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::{
    schema::LightningMessage, ConfigConsumer, ConnectionPoolInterface, ConnectorInterface,
    ListenerInterface, ReceiverInterface, SenderInterface, SenderReceiver,
    SyncQueryRunnerInterface, WithStartAndShutdown,
};

use super::config::Config;

pub struct ConnectionPool<Q: SyncQueryRunnerInterface> {
    _q: PhantomData<Q>,
}

#[derive(Debug)]
pub struct Connector<Q: SyncQueryRunnerInterface, T> {
    _q: PhantomData<Q>,
    _x: PhantomData<T>,
}

impl<Q: SyncQueryRunnerInterface, T> Clone for Connector<Q, T> {
    fn clone(&self) -> Self {
        Self {
            _q: self._q,
            _x: self._x,
        }
    }
}

pub struct Listener<Q: SyncQueryRunnerInterface, T> {
    _q: PhantomData<Q>,
    _x: PhantomData<T>,
}

pub struct Receiver<T> {
    _x: PhantomData<T>,
}

pub struct Sender<T> {
    _x: PhantomData<T>,
}

impl<Q: SyncQueryRunnerInterface> ConnectionPoolInterface for ConnectionPool<Q> {
    type QueryRunner = Q;

    type Connector<T: LightningMessage> = Connector<Q, T>;

    type Listener<T: LightningMessage> = Listener<Q, T>;

    /// The sender struct used across the sender and connector.
    type Sender<T: LightningMessage> = Sender<T>;

    /// The receiver struct used across the sender and connector.
    type Receiver<T: LightningMessage> = Receiver<T>;

    fn init(_config: Self::Config) -> Self {
        Self { _q: PhantomData }
    }

    fn bind<T>(
        &self,
        _scope: lightning_interfaces::ServiceScope,
    ) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage,
    {
        (
            Listener {
                _q: PhantomData,
                _x: PhantomData,
            },
            Connector {
                _q: PhantomData,
                _x: PhantomData,
            },
        )
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for ConnectionPool<Q> {
    const KEY: &'static str = "connection-pool";

    type Config = Config;
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> WithStartAndShutdown for ConnectionPool<Q> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

impl<Q: SyncQueryRunnerInterface, T: LightningMessage> ConnectorInterface<T> for Connector<Q, T> {
    type ConnectionPool = ConnectionPool<Q>;

    fn connect(
        &self,
        _to: &fleek_crypto::NodePublicKey,
    ) -> Option<lightning_interfaces::SenderReceiver<Self::ConnectionPool, T>> {
        None
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, T: LightningMessage> ListenerInterface<T> for Listener<Q, T> {
    type ConnectionPool = ConnectionPool<Q>;

    async fn accept(&mut self) -> Option<SenderReceiver<Self::ConnectionPool, T>> {
        None
    }
}

#[async_trait]
impl<T: LightningMessage> SenderInterface<T> for Sender<T> {
    fn pk(&self) -> &fleek_crypto::NodePublicKey {
        &fleek_crypto::NodePublicKey([0; 96])
    }

    async fn send(&self, _msg: &T) -> bool {
        false
    }
}

#[async_trait]
impl<T: LightningMessage> ReceiverInterface<T> for Receiver<T> {
    fn pk(&self) -> &fleek_crypto::NodePublicKey {
        &fleek_crypto::NodePublicKey([0; 96])
    }

    async fn recv(&mut self) -> Option<T> {
        None
    }
}
