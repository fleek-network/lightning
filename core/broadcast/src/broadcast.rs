use std::marker::PhantomData;
use std::sync::Mutex;

use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::Topic;
use lightning_interfaces::{
    BroadcastInterface,
    ConfigConsumer,
    ListenerConnector,
    WithStartAndShutdown,
};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::frame::Frame;
use crate::pubsub::PubSubI;

pub struct Broadcast<C: Collection> {
    // This is only used upon life-cycle events. And never during the execution
    // of the node. A sync std mutex is sufficient.
    task_handle: Mutex<Option<JoinHandle<()>>>,
    collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Broadcast<C> {
    const KEY: &'static str = "broadcast";
    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Broadcast<C> {
    fn is_running(&self) -> bool {
        let guard = self.task_handle.lock().expect("Unexpected poisioned lock.");
        guard.is_some()
    }

    async fn start(&self) {
        todo!()
    }

    async fn shutdown(&self) {
        todo!()
    }
}

impl<C: Collection> BroadcastInterface<C> for Broadcast<C> {
    type Message = Frame;

    type PubSub<T: LightningMessage + Clone> = PubSubI<T>;

    fn init(
        _config: Self::Config,
        _listener_connector: ListenerConnector<C, c![C::ConnectionPoolInterface], Self::Message>,
        _topology: c!(C::TopologyInterface),
        _signer: &c!(C::SignerInterface),
        _notifier: c!(C::NotifierInterface),
    ) -> anyhow::Result<Self> {
        todo!()
    }

    fn get_pubsub<T: LightningMessage + Clone>(&self, _topic: Topic) -> Self::PubSub<T> {
        todo!()
    }
}
