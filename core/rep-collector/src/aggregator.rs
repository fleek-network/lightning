use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    NodeIndex,
    ReputationMeasurements,
    UpdateMethod,
    MAX_MEASUREMENTS_PER_TX,
};
use lightning_interfaces::Weight;
use tokio::pin;
use tracing::{error, info};
use types::{ExecuteTransactionRequest, ProtocolParamKey, ProtocolParamValue};

use crate::buffered_mpsc;
use crate::config::Config;
use crate::measurement_manager::MeasurementManager;

pub struct ReputationAggregator<C: NodeComponents> {
    reporter: MyReputationReporter,
    query: MyReputationQuery,
    measurement_manager: Mutex<MeasurementManager>,
    notifier: c![C::NotifierInterface],
    submit_tx: SignerSubmitTxSocket,
    report_rx: buffered_mpsc::BufferedReceiver<ReportMessage>,
}

impl<C: NodeComponents> ReputationAggregator<C> {
    /// Create a new reputation
    pub fn new(
        config: &C::ConfigProviderInterface,
        signer: &C::SignerInterface,
        fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();
        let submit_tx = signer.get_socket();

        let (report_tx, report_rx) =
            buffered_mpsc::buffered_channel(config.reporter_buffer_size, 2048);
        let measurement_manager = MeasurementManager::new();
        let local_reputation_ref = measurement_manager.get_local_reputation_ref();

        Ok(Self {
            reporter: MyReputationReporter::new(report_tx),
            query: MyReputationQuery::new(local_reputation_ref),
            measurement_manager: Mutex::new(measurement_manager),
            submit_tx,
            notifier,
            report_rx,
        })
    }

    pub async fn start(
        mut self,
        app_query: fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) {
        app_query.wait_for_genesis().await;

        // Calculate the amount of time before the next epoch change that the "before epoch change"
        // notification should be emitted by the notifier.
        // Default to 5 minutes, but if that exceeds the total epoch time then 3 seconds is used.
        let epoch_time = match app_query.get_protocol_param(&ProtocolParamKey::EpochTime) {
            Some(ProtocolParamValue::EpochTime(epoch_time)) => epoch_time,
            _ => unreachable!("invalid epoch time in protocol params"),
        };
        let mut time_before_epoch_change = Duration::from_secs(300);
        if time_before_epoch_change.as_millis() >= epoch_time as u128 {
            time_before_epoch_change = Duration::from_secs(3);
        }

        let shutdown_future = waiter.wait_for_shutdown();
        pin!(shutdown_future);

        let mut before_epoch_change_sub = self
            .notifier
            .subscribe_before_epoch_change(time_before_epoch_change);

        loop {
            tokio::select! {
                _ = &mut shutdown_future => {
                    break;
                }
                Some(report_msg) = self.report_rx.recv() => {
                    self.handle_report(report_msg);
                }
                Some(_) = before_epoch_change_sub.recv() => {
                    self.submit_aggregation().await;
                    self.measurement_manager.lock().unwrap().clear_measurements();
                }
                else => {
                    error!("Failed to receive message");

                }
            }
        }
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
                info!("Submitting reputation measurements");
                if let Err(e) = submit_tx
                    .enqueue(ExecuteTransactionRequest {
                        method: UpdateMethod::SubmitReputationMeasurements { measurements },
                        options: None,
                    })
                    .await
                {
                    error!("Submitting reputation measurements failed: {e:?}");
                }
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
                    .enqueue(ExecuteTransactionRequest {
                        method: UpdateMethod::SubmitReputationMeasurements {
                            measurements: measurements1,
                        },
                        options: None,
                    })
                    .await
                {
                    error!("Submitting reputation measurements failed: {e:?}");
                }

                info!("Submitting reputation measurements (2)");
                if let Err(e) = self
                    .submit_tx
                    .enqueue(ExecuteTransactionRequest {
                        method: UpdateMethod::SubmitReputationMeasurements {
                            measurements: measurements2,
                        },
                        options: None,
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

impl<C: NodeComponents> BuildGraph for ReputationAggregator<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(
            Self::new.with_event_handler(
                "start",
                Self::start
                    .bounded()
                    .wrap_with_spawn_named("REP-AGGREGATOR"),
            ),
        )
    }
}

impl<C: NodeComponents> ReputationAggregatorInterface<C> for ReputationAggregator<C> {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MyReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MyReputationQuery;

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter {
        self.reporter.clone()
    }

    /// Returns a reputation query that can be used to answer queries about the local
    /// reputation we have of another peer.
    fn get_query(&self) -> Self::ReputationQuery {
        self.query.clone()
    }
}

impl<C: NodeComponents> ConfigConsumer for ReputationAggregator<C> {
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
        spawn!(
            async move {
                // failure here means we're shutting down.
                let _ = tx.send(message).await;
            },
            "REP-AGGREGATOR: send message"
        );
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
