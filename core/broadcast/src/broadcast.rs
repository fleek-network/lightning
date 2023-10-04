use async_trait::async_trait;
use derive_more::IsVariant;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::Topic;
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    ConfigConsumer,
    PoolInterface,
    ReputationAggregatorInterface,
    ServiceScope,
    SignerInterface,
    WithStartAndShutdown,
};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::command::CommandSender;
use crate::config::Config;
use crate::db::Database;
use crate::ev::Context;
use crate::frame::Frame;
use crate::pubsub::PubSubI;

pub struct Broadcast<C: Collection> {
    command_sender: CommandSender,
    // This is only ever `None` when the mutex is locked.
    status: Mutex<Option<Status<C>>>,
}

#[derive(IsVariant)]
enum Status<C: Collection> {
    Running {
        shutdown: Option<oneshot::Sender<()>>,
        handle: JoinHandle<Context<C>>,
    },
    NotRunning {
        ctx: Context<C>,
    },
}

impl<C: Collection> ConfigConsumer for Broadcast<C> {
    const KEY: &'static str = "broadcast";
    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Broadcast<C> {
    fn is_running(&self) -> bool {
        let guard = self.status.blocking_lock();
        guard.as_ref().unwrap().is_running()
    }

    async fn start(&self) {
        let mut guard = self.status.lock().await;

        // This is an unneeded binding. But my rust-analyzer (not rustc) assumes
        // take on `guard.take()` is Iterator::take and then complains that there
        // is one param (i.e size) required.
        let tmp: &mut Option<Status<C>> = &mut guard;
        let state = tmp.take().unwrap();
        let next_state = if let Status::NotRunning { ctx } = state {
            let (shutdown, handle) = ctx.spawn();
            Status::Running {
                shutdown: Some(shutdown),
                handle,
            }
        } else {
            state
        };

        *guard = Some(next_state);
    }

    async fn shutdown(&self) {
        let mut guard = self.status.lock().await;

        let tmp: &mut Option<Status<C>> = &mut guard;
        let state = tmp.take().unwrap();
        let next_state = if let Status::Running { shutdown, handle } = state {
            let _ = shutdown.unwrap().send(());
            let ctx = match handle.await {
                Ok(ctx) => ctx,
                Err(e) => {
                    if e.is_panic() {
                        std::panic::resume_unwind(e.into_panic());
                    }
                    panic!("Failed to shutdown.");
                },
            };
            Status::NotRunning { ctx }
        } else {
            state
        };

        *guard = Some(next_state);
    }
}

impl<C: Collection> BroadcastInterface<C> for Broadcast<C> {
    type Message = Frame;
    type PubSub<T: LightningMessage + Clone> = PubSubI<T>;

    fn init(
        _config: Self::Config,
        sqr: c!(C::ApplicationInterface::SyncExecutor),
        signer: &c!(C::SignerInterface),
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
        pool: &c!(C::PoolInterface),
    ) -> anyhow::Result<Self> {
        let (_, sk) = signer.get_sk();
        let event_handler = pool.open_event(ServiceScope::Broadcast);

        let ctx = Context::<C>::new(Database::default(), sqr, rep_reporter, event_handler, sk);

        Ok(Self {
            command_sender: ctx.get_command_sender(),
            status: Some(Status::NotRunning { ctx }).into(),
        })
    }

    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T> {
        log::debug!("get_pubsub for topic {topic:?} was called.");
        PubSubI::new(topic, self.command_sender.clone())
    }
}
