use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::{
    schema::LightningMessage, types::ServiceScope, ConfigConsumer, ConnectionPoolInterface,
    SignerInterface, SyncQueryRunnerInterface, WithStartAndShutdown,
};

use crate::{connector::Connector, listener::Listener, receiver::Receiver, sender::Sender};

pub struct ConnectionPool<Q, S> {
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

    fn init(config: Self::Config, signer: &Self::Signer, query_runner: Self::QueryRunner) -> Self {
        todo!()
    }

    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage,
    {
        todo!()
    }
}
