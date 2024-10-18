use std::sync::{Arc, Mutex};

use lightning_broadcast::Broadcast;
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::Topic;

pub struct SyncBroadcaster<C: NodeComponents> {
    inner: Arc<Mutex<Broadcast<C>>>,
}

impl<C: NodeComponents> BroadcastInterface<C> for SyncBroadcaster<C> {
    type Message = <Broadcast<C> as BroadcastInterface<C>>::Message;
    type PubSub<T: LightningMessage + Clone> = <Broadcast<C> as BroadcastInterface<C>>::PubSub<T>;

    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T> {
        self.inner.lock().unwrap().get_pubsub(topic)
    }
}

impl<C: NodeComponents> BuildGraph for SyncBroadcaster<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new()
            .with_infallible(Self::new.with_event_handler("start", Self::start))
    }
}

impl<C: NodeComponents> SyncBroadcaster<C> {
    pub fn new(
        keystore: &C::KeystoreInterface,
        rep_aggregator: &C::ReputationAggregatorInterface,
        pool: &c!(C::PoolInterface),
        fdi::Cloned(sqr): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Broadcast::new(
                keystore,
                rep_aggregator,
                pool,
                fdi::Cloned(sqr),
            ))),
        }
    }

    pub fn start(&mut self, fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>) {
        self.inner.lock().unwrap().start(fdi::Cloned(waiter));
    }
}
