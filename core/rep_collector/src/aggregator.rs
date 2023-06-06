use std::time::Duration;

use async_trait::async_trait;
use draco_application::query_runner::QueryRunner;
use draco_interfaces::{
    config::ConfigConsumer, reputation::ReputationAggregatorInterface, signer::SubmitTxSocket,
    ReputationQueryInteface, ReputationReporterInterface, Weight,
};
use fleek_crypto::NodePublicKey;
use tokio::sync::mpsc;

use crate::config::Config;

#[allow(dead_code)]
pub struct ReputationAggregator {
    report_rx: mpsc::Receiver<ReportMessage>,
    reporter: MyReputationReporter,
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
            ReportMessage::Sat {
                peer: _peer,
                weight: _weight,
            } => {},
            ReportMessage::Unsat {
                peer: _peer,
                weight: _weight,
            } => {},
            ReportMessage::Latency {
                peer: _peer,
                latency: _latency,
            } => {},
            ReportMessage::BytesReceived {
                peer: _peer,
                bytes: _bytes,
                duration: _duration,
            } => {},
            ReportMessage::BytesSent {
                peer: _peer,
                bytes: _bytes,
                duration: _duration,
            } => {},
            ReportMessage::Hops {
                peer: _peer,
                hops: _hops,
            } => {},
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
    async fn init(_config: Self::Config, _submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        let (report_tx, report_rx) = mpsc::channel(2048);
        Ok(Self {
            report_rx,
            reporter: MyReputationReporter::new(report_tx),
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
        todo!()
    }
}

impl ConfigConsumer for ReputationAggregator {
    const KEY: &'static str = "reputation";

    type Config = Config;
}

#[derive(Clone)]
pub struct MyReputationQuery {}

impl ReputationQueryInteface for MyReputationQuery {
    /// The application layer's synchronize query runner.
    type SyncQuery = QueryRunner;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, _peer: &NodePublicKey) -> Option<u128> {
        todo!()
    }
}

#[derive(Clone)]
pub struct MyReputationReporter {
    tx: mpsc::Sender<ReportMessage>,
}

impl MyReputationReporter {
    fn new(tx: mpsc::Sender<ReportMessage>) -> Self {
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

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
