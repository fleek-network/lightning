use std::net::SocketAddr;

use async_trait::async_trait;
use derive_more::IsVariant;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::Topic;
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    ConfigConsumer,
    SignerInterface,
    WithStartAndShutdown,
};
use netkit::builder::Builder;
use netkit::endpoint::Endpoint;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::command::CommandSender;
use crate::config::Config;
use crate::db::Database;
use crate::ev::Context;
use crate::frame::Frame;
use crate::pubsub::PubSubI;

pub struct Broadcast<C: Collection> {
    command_sender: CommandSender,
    endpoint: Mutex<Option<Endpoint>>,
    // This is only ever `None` when the mutex is locked.
    status: Mutex<Option<Status<C>>>,
}

#[derive(IsVariant)]
enum Status<C: Collection> {
    Running {
        shutdown: Option<oneshot::Sender<()>>,
        shutdown_endpoint: Option<CancellationToken>,
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
        let shutdown_endpoint_token = CancellationToken::new();
        let next_state = if let Status::NotRunning { ctx } = state {
            let (shutdown, handle) = ctx.spawn();
            Status::Running {
                shutdown: Some(shutdown),
                shutdown_endpoint: Some(shutdown_endpoint_token.clone()),
                handle,
            }
        } else {
            state
        };

        *guard = Some(next_state);

        let mut endpoint = self
            .endpoint
            .lock()
            .await
            .take()
            .expect("There to be a netkit endpoint");
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_endpoint_token.cancelled() => {
                        break;
                    }
                    _ = endpoint.start() => {
                        break
                    }
                }
            }
        });
    }

    async fn shutdown(&self) {
        let mut guard = self.status.lock().await;

        let tmp: &mut Option<Status<C>> = &mut guard;
        let state = tmp.take().unwrap();
        let next_state = if let Status::Running {
            shutdown,
            shutdown_endpoint,
            handle,
        } = state
        {
            let _ = shutdown_endpoint.unwrap().cancel();
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
        config: Self::Config,
        sqr: c!(C::ApplicationInterface::SyncExecutor),
        topology: c!(C::TopologyInterface),
        signer: &c!(C::SignerInterface),
        notifier: c!(C::NotifierInterface),
    ) -> anyhow::Result<Self> {
        let (_, sk) = signer.get_sk();
        // Todo: Move this to config.
        let mut builder = Builder::new(sk.clone());
        builder.socket_address(config.address);
        let mut endpoint = builder.build()?;
        let network_event_rx = endpoint.network_event_receiver();
        let endpoint_tx = endpoint.request_sender();

        let ctx = Context::<C>::new(
            Database::default(),
            sqr,
            notifier,
            topology,
            network_event_rx,
            endpoint_tx,
            sk,
        );

        Ok(Self {
            command_sender: ctx.command_sender(),
            status: Some(Status::NotRunning { ctx }).into(),
            endpoint: Some(endpoint).into(),
        })
    }

    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T> {
        log::debug!("get_pubsub for topic {topic:?} was called.");
        PubSubI::new(topic, self.command_sender.clone())
    }
}
