use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use affair::{Socket, Task};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::DeliveryAcknowledgment;
use lightning_interfaces::{
    ConfigConsumer,
    DeliveryAcknowledgmentAggregatorInterface,
    DeliveryAcknowledgmentSocket,
    SubmitTxSocket,
    WithStartAndShutdown,
};
use tokio::sync::{mpsc, Notify};
use tracing::error;

use crate::config::Config;

pub struct DeliveryAcknowledgmentAggregator<C: Collection> {
    inner: Arc<AggregatorInner>,
    socket: DeliveryAcknowledgmentSocket,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
    _marker: PhantomData<C>,
}

struct AggregatorInner {
    config: Config,
    #[allow(unused)]
    submit_tx: SubmitTxSocket,
    #[allow(clippy::type_complexity)]
    socket_rx: Arc<Mutex<Option<mpsc::Receiver<Task<DeliveryAcknowledgment, ()>>>>>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> DeliveryAcknowledgmentAggregatorInterface<C>
    for DeliveryAcknowledgmentAggregator<C>
{
    fn init(config: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        let (socket, socket_rx) = Socket::raw_bounded(2048);
        let shutdown_notify = Arc::new(Notify::new());
        let inner = AggregatorInner::new(config, submit_tx, socket_rx, shutdown_notify.clone());

        Ok(Self {
            inner: Arc::new(inner),
            socket,
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
            _marker: PhantomData,
        })
    }

    fn socket(&self) -> DeliveryAcknowledgmentSocket {
        self.socket.clone()
    }
}

impl AggregatorInner {
    fn new(
        config: Config,
        submit_tx: SubmitTxSocket,
        socket_rx: mpsc::Receiver<Task<DeliveryAcknowledgment, ()>>,
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            config,
            submit_tx,
            socket_rx: Arc::new(Mutex::new(Some(socket_rx))),
            shutdown_notify,
        }
    }

    async fn start(&self) {
        let mut socket_rx = self.socket_rx.lock().unwrap().take().unwrap();
        let mut interval = tokio::time::interval(self.config.submit_interval);
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                task = socket_rx.recv() => {
                    if let Some(_task) = task {
                    }
                }
                _ = interval.tick() => {

                }
            }
        }
        *self.socket_rx.lock().unwrap() = Some(socket_rx);
    }
}

impl<C: Collection> WithStartAndShutdown for DeliveryAcknowledgmentAggregator<C> {
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
            error!("Cannot start delivery aggregator because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

impl<C: Collection> ConfigConsumer for DeliveryAcknowledgmentAggregator<C> {
    const KEY: &'static str = "dack-aggregator";

    type Config = Config;
}
