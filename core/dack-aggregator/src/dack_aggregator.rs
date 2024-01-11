use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use affair::{Socket, Task};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    DeliveryAcknowledgment,
    UpdateMethod,
    MAX_DELIVERY_ACKNOWLEDGMENTS,
};
use lightning_interfaces::{
    ConfigConsumer,
    DeliveryAcknowledgmentAggregatorInterface,
    DeliveryAcknowledgmentSocket,
    SubmitTxSocket,
    WithStartAndShutdown,
};
use queue_file::QueueFile;
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
    queue: Arc<Mutex<Option<QueueFile>>>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> DeliveryAcknowledgmentAggregatorInterface<C>
    for DeliveryAcknowledgmentAggregator<C>
{
    fn init(config: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        let (socket, socket_rx) = Socket::raw_bounded(2048);
        let shutdown_notify = Arc::new(Notify::new());
        let inner = AggregatorInner::new(config, submit_tx, socket_rx, shutdown_notify.clone())?;

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
    ) -> anyhow::Result<Self> {
        let queue = QueueFile::open(&config.db_path)?;
        Ok(Self {
            config,
            submit_tx,
            socket_rx: Arc::new(Mutex::new(Some(socket_rx))),
            queue: Arc::new(Mutex::new(Some(queue))),
            shutdown_notify,
        })
    }

    async fn start(&self) {
        let mut socket_rx = self.socket_rx.lock().unwrap().take().unwrap();
        let mut queue = self.queue.lock().unwrap().take().unwrap();
        let mut interval = tokio::time::interval(self.config.submit_interval);
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                task = socket_rx.recv() => {
                    if let Some(task) = task {
                        match bincode::serialize(&task.request) {
                            Ok(dack_bytes) => {
                                task.respond(());
                                if let Err(e) = queue.add(&dack_bytes) {
                                    // TODO(matthias): this should be a telemetry event log
                                    error!("Failed to write DACK to disk: {e:?}");
                                }
                            }
                            Err(e) => {
                                task.respond(());
                                error!("Failed to serialize DACK: {e:?}");
                            }
                        }

                    }
                }
                _ = interval.tick() => {
                    let mut dacks = Vec::new();
                    let mut num_dacks_taken = 0;
                    for dack_bytes in queue.iter() {
                        if dacks.len() == MAX_DELIVERY_ACKNOWLEDGMENTS {
                            break;
                        }
                        match bincode::deserialize::<DeliveryAcknowledgment>(&dack_bytes) {
                            Ok(dack) => {
                                dacks.push(dack);
                            }
                            Err(e) => {
                                error!("Failed to deserialize DACK: {e:?}");
                            }
                        }
                        num_dacks_taken += 1;
                    }

                    let submit_tx = self.submit_tx.clone();
                    // TODO(matthias): fill this information in. This information has
                    // has to be sent through the aggregator socket, along with the DACK.
                    let update = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                        commodity: 1, // dummy
                        service_id: 0, // dummy
                        proofs: dacks,
                        metadata: None // dummy
                    };
                    tokio::spawn(async move {
                        if let Err(e) = submit_tx
                            .run(update)
                            .await
                        {
                            error!("Failed to submit DACK to signer: {e:?}");
                        }
                    });
                    // After sending the transaction, we remove the DACKs we sent from the queue.
                    (0..num_dacks_taken).for_each(|_| {
                        // TODO(matthias): should we only remove them after we verified that the transaction was
                        // ordered?
                        if let Err(e) = queue.remove() {
                            error!("Failed to remove DACK from queue: {e:?}");
                        }
                    });
                }
            }
        }
        *self.socket_rx.lock().unwrap() = Some(socket_rx);
        *self.queue.lock().unwrap() = Some(queue);
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
