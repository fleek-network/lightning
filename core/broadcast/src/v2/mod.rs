use std::marker::PhantomData;

use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::{BroadcastInterface, ListenerConnector};

pub struct Broadcast<C: Collection> {
    collection: PhantomData<C>,
}

impl<C: Collection> BroadcastInterface<C> for Broadcast<C> {
    type Message = ();

    type PubSub<T: LightningMessage + Clone> = ();

    fn init(
        config: Self::Config,
        listener_connector: ListenerConnector<C, c![C::ConnectionPoolInterface], Self::Message>,
        topology: c!(C::TopologyInterface),
        signer: &c!(C::SignerInterface),
        notifier: c!(C::NotifierInterface),
    ) -> Result<Self> {
        todo!()
    }

    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T> {
        todo!()
    }
}
