use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use draco_application::query_runner::QueryRunner;
use draco_interfaces::{
    config::ConfigConsumer, reputation::ReputationAggregatorInterface, signer::SubmitTxSocket,
    ReputationQueryInteface, ReputationReporterInterface, Weight,
};
use fleek_crypto::NodePublicKey;

use crate::{buffered_mpsc, config::Config, measurement_manager::MeasurementManager};

#[allow(dead_code)]
pub struct ReputationAggregator {
    report_rx: buffered_mpsc::BufferedReceiver<ReportMessage>,
    reporter: MyReputationReporter,
    query: MyReputationQuery,
    measurement_manager: MeasurementManager,
    submit_tx: SubmitTxSocket,
    config: Config,
}

#[allow(dead_code)]
impl ReputationAggregator {
    async fn start(mut self) -> anyhow::Result<()> {
        loop {
            let report_msg = self
                .report_rx
                .recv()
                .await
                .expect("Failed to receive report message.");
            self.handle_report(report_msg);
        }
    }

    fn handle_report(&mut self, report_msg: ReportMessage) {
        match report_msg {
            ReportMessage::Sat { peer, weight } => {
                self.measurement_manager.report_sat(peer, weight);
            },
            ReportMessage::Unsat { peer, weight } => {
                self.measurement_manager.report_unsat(peer, weight);
            },
            ReportMessage::Latency { peer, latency } => {
                self.measurement_manager.report_latency(peer, latency);
            },
            ReportMessage::BytesReceived {
                peer,
                bytes,
                duration,
            } => {
                self.measurement_manager
                    .report_bytes_received(peer, bytes, duration);
            },
            ReportMessage::BytesSent {
                peer,
                bytes,
                duration,
            } => {
                self.measurement_manager
                    .report_bytes_sent(peer, bytes, duration);
            },
            ReportMessage::Hops { peer, hops } => {
                self.measurement_manager.report_hops(peer, hops);
            },
        }
    }
}

#[async_trait]
impl ReputationAggregatorInterface for ReputationAggregator {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MyReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MyReputationQuery;

    /// Create a new reputation
    async fn init(config: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        let (report_tx, report_rx) = buffered_mpsc::buffered_channel(100, 2048);
        let measurement_manager = MeasurementManager::new();
        let local_reputation_ref = measurement_manager.get_local_reputation_ref();
        Ok(Self {
            report_rx,
            reporter: MyReputationReporter::new(report_tx),
            query: MyReputationQuery::new(local_reputation_ref),
            measurement_manager: MeasurementManager::new(),
            submit_tx,
            config,
        })
    }

    /// Called by the scheduler to notify that it is time to submit the aggregation, to do
    /// so one should use the [`SubmitTxSocket`] that is passed during the initialization
    /// to submit a transaction to the consensus.
    fn submit_aggregation(&self) {
        todo!()
    }

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

impl ConfigConsumer for ReputationAggregator {
    const KEY: &'static str = "rep-collector";

    type Config = Config;
}

#[derive(Clone)]
pub struct MyReputationQuery {
    local_reputation: Arc<scc::HashMap<NodePublicKey, u128>>,
}

impl MyReputationQuery {
    fn new(local_reputation: Arc<scc::HashMap<NodePublicKey, u128>>) -> Self {
        Self { local_reputation }
    }
}

impl ReputationQueryInteface for MyReputationQuery {
    /// The application layer's synchronize query runner.
    type SyncQuery = QueryRunner;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, peer: &NodePublicKey) -> Option<u128> {
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
    fn report_sat(&self, peer: &NodePublicKey, weight: Weight) {
        let message = ReportMessage::Sat {
            peer: *peer,
            weight,
        };
        self.send_message(message);
    }

    /// Report a unsatisfactory (happy) interaction with the given peer.
    fn report_unsat(&self, peer: &NodePublicKey, weight: Weight) {
        let message = ReportMessage::Unsat {
            peer: *peer,
            weight,
        };
        self.send_message(message);
    }

    /// Report a latency which we witnessed from another peer.
    fn report_latency(&self, peer: &NodePublicKey, latency: Duration) {
        let message = ReportMessage::Latency {
            peer: *peer,
            latency,
        };
        self.send_message(message);
    }

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, peer: &NodePublicKey, bytes: u64, duration: Option<Duration>) {
        let message = ReportMessage::BytesReceived {
            peer: *peer,
            bytes,
            duration,
        };
        self.send_message(message);
    }

    /// Report the number of (healthy) bytes which we sent from another peer.
    fn report_bytes_sent(&self, peer: &NodePublicKey, bytes: u64, duration: Option<Duration>) {
        let message = ReportMessage::BytesSent {
            peer: *peer,
            bytes,
            duration,
        };
        self.send_message(message);
    }

    /// Report the number of hops we have witnessed to the given peer.
    fn report_hops(&self, peer: &NodePublicKey, hops: u8) {
        let message = ReportMessage::Hops { peer: *peer, hops };
        self.send_message(message);
    }
}

#[derive(Debug)]
enum ReportMessage {
    Sat {
        peer: NodePublicKey,
        weight: Weight,
    },
    Unsat {
        peer: NodePublicKey,
        weight: Weight,
    },
    Latency {
        peer: NodePublicKey,
        latency: Duration,
    },
    BytesReceived {
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    },
    BytesSent {
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    },
    Hops {
        peer: NodePublicKey,
        hops: u8,
    },
}
