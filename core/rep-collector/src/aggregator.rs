use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lightning_interfaces::config::ConfigConsumer;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::notifier::{Notification, NotifierInterface};
use lightning_interfaces::reputation::ReputationAggregatorInterface;
use lightning_interfaces::signer::SubmitTxSocket;
use lightning_interfaces::types::{
    NodeIndex,
    ReputationMeasurements,
    UpdateMethod,
    MAX_MEASUREMENTS_PER_TX,
};
use lightning_interfaces::{
    ApplicationInterface,
    ReputationQueryInteface,
    ReputationReporterInterface,
    Weight,
    WithStartAndShutdown,
};
use tokio::sync::{mpsc, Notify};
use tracing::{error, info};

use crate::buffered_mpsc;
use crate::config::Config;
use crate::measurement_manager::MeasurementManager;

#[cfg(all(not(test), not(debug_assertions)))]
const BEFORE_EPOCH_CHANGE: Duration = Duration::from_secs(300);
#[cfg(any(test, debug_assertions))]
const BEFORE_EPOCH_CHANGE: Duration = Duration::from_secs(3);

pub struct ReputationAggregator<C: Collection> {
    inner: Arc<ReputationAggregatorInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> WithStartAndShutdown for ReputationAggregator<C> {
    /// Returns true if this system is running or not.
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
            error!("Cannot start reputation aggregator because it is already running");
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

impl<C: Collection> ReputationAggregatorInterface<C> for ReputationAggregator<C> {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MyReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MyReputationQuery;

    /// Create a new reputation
    fn init(
        config: Config,
        submit_tx: SubmitTxSocket,
        notifier: c!(C::NotifierInterface),
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        let (report_tx, report_rx) =
            buffered_mpsc::buffered_channel(config.reporter_buffer_size, 2048);
        let (notify_before_epoch_tx, notify_before_epoch_rx) = mpsc::channel(128);
        let (notify_new_epoch_tx, notify_new_epoch_rx) = mpsc::channel(128);
        let measurement_manager = MeasurementManager::new();
        let local_reputation_ref = measurement_manager.get_local_reputation_ref();

        let shutdown_notify = Arc::new(Notify::new());

        let inner = ReputationAggregatorInner {
            report_rx: Arc::new(Mutex::new(Some(report_rx))),
            reporter: MyReputationReporter::new(report_tx),
            query: MyReputationQuery::new(local_reputation_ref),
            measurement_manager: Arc::new(Mutex::new(measurement_manager)),
            submit_tx,
            notifier,
            notify_before_epoch_rx: Arc::new(Mutex::new(Some(notify_before_epoch_rx))),
            notify_before_epoch_tx,
            notify_new_epoch_rx: Arc::new(Mutex::new(Some(notify_new_epoch_rx))),
            notify_new_epoch_tx,
            _query_runner: query_runner,
            shutdown_notify: shutdown_notify.clone(),
            _config: config,
        };

        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
        })
    }

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter {
        self.inner.reporter.clone()
    }

    /// Returns a reputation query that can be used to answer queries about the local
    /// reputation we have of another peer.
    fn get_query(&self) -> Self::ReputationQuery {
        self.inner.query.clone()
    }
}

struct ReputationAggregatorInner<C: Collection> {
    report_rx: Arc<Mutex<Option<buffered_mpsc::BufferedReceiver<ReportMessage>>>>,
    reporter: MyReputationReporter,
    query: MyReputationQuery,
    measurement_manager: Arc<Mutex<MeasurementManager>>,
    submit_tx: SubmitTxSocket,
    notifier: c![C::NotifierInterface],
    notify_before_epoch_rx: Arc<Mutex<Option<mpsc::Receiver<Notification>>>>,
    notify_before_epoch_tx: mpsc::Sender<Notification>,
    notify_new_epoch_rx: Arc<Mutex<Option<mpsc::Receiver<Notification>>>>,
    notify_new_epoch_tx: mpsc::Sender<Notification>,
    _query_runner: c![C::ApplicationInterface::SyncExecutor],
    shutdown_notify: Arc<Notify>,
    _config: Config,
}

impl<C: Collection> ReputationAggregatorInner<C> {
    async fn start(&self) {
        self.notifier
            .notify_before_epoch_change(BEFORE_EPOCH_CHANGE, self.notify_before_epoch_tx.clone());
        self.notifier
            .notify_on_new_epoch(self.notify_new_epoch_tx.clone());
        let mut report_rx = self.report_rx.lock().unwrap().take().unwrap();
        let mut notify_before_epoch_rx =
            self.notify_before_epoch_rx.lock().unwrap().take().unwrap();
        let mut notify_new_epoch_rx = self.notify_new_epoch_rx.lock().unwrap().take().unwrap();
        let shutdown_notify = self.shutdown_notify.clone();
        loop {
            tokio::select! {
                _ = shutdown_notify.notified() => {
                    break;
                }
                report_msg = report_rx.recv() => {
                    if let Some(report_msg) = report_msg {
                        self.handle_report(report_msg);
                    } else {
                        error!("Failed to receive message");
                    }
                }
                notification = notify_before_epoch_rx.recv() => {
                    if let Some(Notification::BeforeEpochChange) = notification {
                        self.submit_aggregation().await;
                        self.measurement_manager.lock().unwrap().clear_measurements();
                    } else {
                        error!("Failed to receive message");
                    }
                }
                notification = notify_new_epoch_rx.recv() => {
                    if let Some(Notification::NewEpoch) = notification {
                        self.notifier
                            .notify_before_epoch_change(
                                BEFORE_EPOCH_CHANGE,
                                self.notify_before_epoch_tx.clone()
                            );
                    } else {
                        error!("Failed to receive message");
                    }
                }
            }
        }
        *self.report_rx.lock().unwrap() = Some(report_rx);
        *self.notify_before_epoch_rx.lock().unwrap() = Some(notify_before_epoch_rx);
        *self.notify_new_epoch_rx.lock().unwrap() = Some(notify_new_epoch_rx);
    }

    /// Called by the scheduler to notify that it is time to submit the aggregation, to do
    /// so one should use the [`notify_new_epoch_rx`] that is passed during the initialization
    /// to submit a transaction to the consensus.
    async fn submit_aggregation(&self) {
        let measurements: BTreeMap<NodeIndex, ReputationMeasurements> = self
            .measurement_manager
            .lock()
            .unwrap()
            .get_measurements()
            .into_iter()
            .collect();
        if !measurements.is_empty() {
            if measurements.len() <= MAX_MEASUREMENTS_PER_TX {
                let submit_tx = self.submit_tx.clone();
                tokio::spawn(async move {
                    info!("Submitting reputation measurements");
                    if let Err(e) = submit_tx
                        .run(UpdateMethod::SubmitReputationMeasurements { measurements })
                        .await
                    {
                        error!("Submitting reputation measurements failed: {e:?}");
                    }
                });
            } else {
                // Number of measurements exceeds maximum, we have to split the transaction into
                // two.
                let mut measurements1 = BTreeMap::new();
                let mut measurements2 = BTreeMap::new();
                for (node, m) in measurements.into_iter() {
                    if measurements1.len() < MAX_MEASUREMENTS_PER_TX {
                        measurements1.insert(node, m);
                    } else if measurements2.len() < MAX_MEASUREMENTS_PER_TX {
                        measurements2.insert(node, m);
                    } else {
                        // If both hashmaps exceed the maximum, we will drop the remaining
                        // measurements. Enough is enough.
                        break;
                    }
                }

                info!("Submitting reputation measurements (1)");
                if let Err(e) = self
                    .submit_tx
                    .enqueue(UpdateMethod::SubmitReputationMeasurements {
                        measurements: measurements1,
                    })
                    .await
                {
                    error!("Submitting reputation measurements failed: {e:?}");
                }

                info!("Submitting reputation measurements (2)");
                if let Err(e) = self
                    .submit_tx
                    .enqueue(UpdateMethod::SubmitReputationMeasurements {
                        measurements: measurements2,
                    })
                    .await
                {
                    error!("Submitting reputation measurements failed: {e:?}");
                }
            }
        } else {
            info!("No reputation measurements to submit");
        }
    }

    fn handle_report(&self, report_msg: ReportMessage) {
        match report_msg {
            ReportMessage::Sat { peer, weight } => {
                self.measurement_manager
                    .lock()
                    .unwrap()
                    .report_sat(peer, weight);
            },
            ReportMessage::Unsat { peer, weight } => {
                self.measurement_manager
                    .lock()
                    .unwrap()
                    .report_unsat(peer, weight);
            },
            ReportMessage::Ping { peer, latency } => match latency {
                Some(latency) => {
                    let mut manager = self.measurement_manager.lock().unwrap();
                    manager.report_latency(peer, latency);
                    manager.report_ping(peer, true);
                },
                None => {
                    self.measurement_manager
                        .lock()
                        .unwrap()
                        .report_ping(peer, false);
                },
            },
            ReportMessage::BytesReceived {
                peer,
                bytes,
                duration,
            } => {
                self.measurement_manager
                    .lock()
                    .unwrap()
                    .report_bytes_received(peer, bytes, duration);
            },
            ReportMessage::BytesSent {
                peer,
                bytes,
                duration,
            } => {
                self.measurement_manager
                    .lock()
                    .unwrap()
                    .report_bytes_sent(peer, bytes, duration);
            },
            ReportMessage::Hops { peer, hops } => {
                self.measurement_manager
                    .lock()
                    .unwrap()
                    .report_hops(peer, hops);
            },
        }
    }
}

impl<C: Collection> ConfigConsumer for ReputationAggregator<C> {
    const KEY: &'static str = "rep-collector";

    type Config = Config;
}

#[derive(Clone)]
pub struct MyReputationQuery {
    local_reputation: Arc<scc::HashMap<NodeIndex, u8>>,
}

impl MyReputationQuery {
    fn new(local_reputation: Arc<scc::HashMap<NodeIndex, u8>>) -> Self {
        Self { local_reputation }
    }
}

impl ReputationQueryInteface for MyReputationQuery {
    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, peer: &NodeIndex) -> Option<u8> {
        self.local_reputation.get(peer).map(|entry| *entry.get())
    }
}

#[derive(Clone)]
pub struct MyReputationReporter {
    tx: buffered_mpsc::BufferedSender<ReportMessage>,
}

impl MyReputationReporter {
    fn new(tx: buffered_mpsc::BufferedSender<ReportMessage>) -> Self {
        Self { tx }
    }

    fn send_message(&self, message: ReportMessage) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(message).await.unwrap();
        });
    }
}

impl ReputationReporterInterface for MyReputationReporter {
    /// Report a satisfactory (happy) interaction with the given peer.
    fn report_sat(&self, peer: NodeIndex, weight: Weight) {
        let message = ReportMessage::Sat { peer, weight };
        self.send_message(message);
    }

    /// Report a unsatisfactory (happy) interaction with the given peer.
    fn report_unsat(&self, peer: NodeIndex, weight: Weight) {
        let message = ReportMessage::Unsat { peer, weight };
        self.send_message(message);
    }

    /// Report a ping interaction with another peer and the latency if the peer responded.
    /// `None` indicates that the peer did not respond.
    fn report_ping(&self, peer: NodeIndex, latency: Option<Duration>) {
        let message = ReportMessage::Ping { peer, latency };
        self.send_message(message);
    }

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, peer: NodeIndex, bytes: u64, duration: Option<Duration>) {
        let message = ReportMessage::BytesReceived {
            peer,
            bytes,
            duration,
        };
        self.send_message(message);
    }

    /// Report the number of (healthy) bytes which we sent from another peer.
    fn report_bytes_sent(&self, peer: NodeIndex, bytes: u64, duration: Option<Duration>) {
        let message = ReportMessage::BytesSent {
            peer,
            bytes,
            duration,
        };
        self.send_message(message);
    }

    /// Report the number of hops we have witnessed to the given peer.
    fn report_hops(&self, peer: NodeIndex, hops: u8) {
        let message = ReportMessage::Hops { peer, hops };
        self.send_message(message);
    }
}

#[derive(Debug)]
enum ReportMessage {
    Sat {
        peer: NodeIndex,
        weight: Weight,
    },
    Unsat {
        peer: NodeIndex,
        weight: Weight,
    },
    Ping {
        peer: NodeIndex,
        latency: Option<Duration>,
    },
    BytesReceived {
        peer: NodeIndex,
        bytes: u64,
        duration: Option<Duration>,
    },
    BytesSent {
        peer: NodeIndex,
        bytes: u64,
        duration: Option<Duration>,
    },
    Hops {
        peer: NodeIndex,
        hops: u8,
    },
}
