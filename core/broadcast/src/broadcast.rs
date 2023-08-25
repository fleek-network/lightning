use std::marker::PhantomData;

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

use crate::config::Config;
use crate::frame::Frame;
use crate::pubsub::PubSubI;

pub struct Broadcast<C: Collection> {
    collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Broadcast<C> {
    const KEY: &'static str = "broadcast";
    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Broadcast<C> {
    fn is_running(&self) -> bool {
        true
    }

    async fn start(&self) {}

    async fn shutdown(&self) {}
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
