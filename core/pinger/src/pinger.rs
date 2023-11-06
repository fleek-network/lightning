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
    inner: Arc<PingerInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection> PingerInterface<C> for Pinger<C> {
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> anyhow::Result<Self> {
        let shutdown_notify = Arc::new(Notify::new());
        let inner =
            PingerInner::<C>::new(config, query_runner, rep_reporter, shutdown_notify.clone());
        Ok(Self {
            inner: Arc::new(inner),
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
            let inner = self.inner.clone();
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                inner.start().await;
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
struct PingerInner<C: Collection> {
    config: Config,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingerInner<C> {
    fn new(
        config: Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            config,
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
