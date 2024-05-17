use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::broadcast::Frame;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::Topic;
use lightning_interfaces::ServiceScope;
use tracing::debug;

use crate::backend::LightningBackend;
use crate::command::CommandSender;
use crate::db::Database;
use crate::ev::Context;
use crate::pubsub::PubSubI;

pub struct Broadcast<C: Collection> {
    command_sender: CommandSender,
    ctx: Option<Context<LightningBackend<C>>>,
}

impl<C: Collection> Broadcast<C> {
    pub fn new(
        keystore: &C::KeystoreInterface,
        rep_aggregator: &C::ReputationAggregatorInterface,
        pool: &c!(C::PoolInterface),
        fdi::Cloned(sqr): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Self {
        let sk = keystore.get_ed25519_sk();
        let event_handler = pool.open_event(ServiceScope::Broadcast);
        let rep_reporter = rep_aggregator.get_reporter();

        let backend = LightningBackend::new(sqr, rep_reporter, event_handler, sk);
        let ctx = Context::new(Database::default(), backend);

        Self {
            command_sender: ctx.get_command_sender(),
            ctx: Some(ctx),
        }
    }

    pub fn start(&mut self, fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>) {
        let ctx = self.ctx.take().expect("The context to be present.");
        ctx.spawn(waiter);
    }
}

impl<C: Collection> BuildGraph for Broadcast<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new()
            .with_infallible(Self::new.with_event_handler("start", Self::start))
    }
}

impl<C: Collection> BroadcastInterface<C> for Broadcast<C> {
    type Message = Frame;
    type PubSub<T: LightningMessage + Clone> = PubSubI<T>;

    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T> {
        debug!("get_pubsub for topic {topic:?} was called.");
        PubSubI::new(topic, self.command_sender.clone())
    }
}
