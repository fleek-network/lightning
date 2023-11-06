use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    PingerInterface,
    ReputationAggregatorInterface,
    WithStartAndShutdown,
};
use tokio::sync::Notify;
use tracing::error;

use crate::config::Config;

pub struct Pinger<C: Collection> {
    sender: Arc<PingSender<C>>,
    responder: Arc<PingResponder<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection> PingerInterface<C> for Pinger<C> {
    fn init(
        _config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> anyhow::Result<Self> {
        let shutdown_notify = Arc::new(Notify::new());
        let sender = PingSender::<C>::new(
            query_runner.clone(),
            rep_reporter.clone(),
            shutdown_notify.clone(),
        );
        let responder =
            PingResponder::<C>::new(query_runner, rep_reporter, shutdown_notify.clone());
        Ok(Self {
            sender: Arc::new(sender),
            responder: Arc::new(responder),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
            _marker: PhantomData,
        })
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Pinger<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        if !self.is_running() {
            let sender = self.sender.clone();
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                sender.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            let responder = self.responder.clone();
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                responder.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            self.is_running.store(true, Ordering::Relaxed);
        } else {
            error!("Cannot start pinger because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

#[allow(unused)]
struct PingSender<C: Collection> {
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingSender<C> {
    fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            query_runner,
            rep_reporter,
            shutdown_notify,
        }
    }

    async fn start(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }

            }
        }
    }
}

#[allow(unused)]
struct PingResponder<C: Collection> {
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingResponder<C> {
    fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            query_runner,
            rep_reporter,
            shutdown_notify,
        }
    }

    async fn start(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }

            }
        }
    }
}

impl<C: Collection> ConfigConsumer for Pinger<C> {
    const KEY: &'static str = "pinger";

    type Config = Config;
}
