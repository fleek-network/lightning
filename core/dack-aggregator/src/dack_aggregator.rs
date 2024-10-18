use std::collections::HashMap;
use std::marker::PhantomData;

use affair::{Socket, Task};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    DeliveryAcknowledgment,
    DeliveryAcknowledgmentProof,
    UpdateMethod,
    MAX_DELIVERY_ACKNOWLEDGMENTS,
};
use lightning_metrics::increment_counter_by;
use queue_file::QueueFile;
use tokio::sync::mpsc;
use tracing::error;
use types::ExecuteTransactionRequest;

use crate::config::Config;

pub struct DeliveryAcknowledgmentAggregator<C: NodeComponents> {
    inner: Option<AggregatorInner>,
    socket: DeliveryAcknowledgmentSocket,
    _marker: PhantomData<C>,
}

impl<C: NodeComponents> DeliveryAcknowledgmentAggregator<C> {
    /// Initialize a new delivery acknowledgment aggregator.
    fn init(
        config: &C::ConfigProviderInterface,
        signer: &C::SignerInterface,
    ) -> anyhow::Result<Self> {
        let (socket, socket_rx) = Socket::raw_bounded(2048);
        let inner = AggregatorInner::new(config.get::<Self>(), signer.get_socket(), socket_rx)?;

        Ok(Self {
            inner: Some(inner),
            socket,
            _marker: PhantomData,
        })
    }

    async fn start(
        mut this: fdi::RefMut<Self>,
        fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>,
    ) {
        let inner = this.inner.take().expect("can only call start once");
        drop(this);

        shutdown
            .run_until_shutdown(async move { inner.start().await })
            .await;
    }
}

impl<C: NodeComponents> BuildGraph for DeliveryAcknowledgmentAggregator<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::init.with_event_handler(
            "start",
            Self::start.wrap_with_spawn_named("DACK_AGGREGATOR"),
        ))
    }
}

impl<C: NodeComponents> DeliveryAcknowledgmentAggregatorInterface<C>
    for DeliveryAcknowledgmentAggregator<C>
{
    fn socket(&self) -> DeliveryAcknowledgmentSocket {
        self.socket.clone()
    }
}

struct AggregatorInner {
    config: Config,
    #[allow(unused)]
    submit_tx: SignerSubmitTxSocket,
    #[allow(clippy::type_complexity)]
    socket_rx: mpsc::Receiver<Task<DeliveryAcknowledgment, ()>>,
    queue: QueueFile,
}

impl AggregatorInner {
    fn new(
        config: Config,
        submit_tx: SignerSubmitTxSocket,
        socket_rx: mpsc::Receiver<Task<DeliveryAcknowledgment, ()>>,
    ) -> anyhow::Result<Self> {
        let queue = QueueFile::open(&config.db_path)?;
        Ok(Self {
            config,
            submit_tx,
            socket_rx,
            queue,
        })
    }

    async fn start(mut self) {
        let mut interval = tokio::time::interval(self.config.submit_interval);
        loop {
            tokio::select! {
                task = self.socket_rx.recv() => {
                    if let Some(task) = task {
                        match bincode::serialize(&task.request) {
                            Ok(dack_bytes) => {
                                task.respond(());
                                if let Err(e) = self.queue.add(&dack_bytes) {
                                    // TODO(matthias): this should be a telemetry event log
                                    error!("Failed to write DACK to disk: {e:?}");
                                }
                            }
                            Err(e) => {
                                task.respond(());
                                error!("Failed to serialize DACK: {e:?}");
                            }
                        }
                    } else {
                        break;
                    }
                }
                _ = interval.tick() => {
                    let mut proofs: HashMap<u32, Vec<DeliveryAcknowledgmentProof>> = HashMap::new();
                    let mut metadata = HashMap::new();
                    let mut commodity = HashMap::new();
                    let mut num_dacks_taken = 0;
                    for dack_bytes in self.queue.iter() {
                        match bincode::deserialize::<DeliveryAcknowledgment>(&dack_bytes) {
                            Ok(dack) => {
                                let num_dacks = proofs.get(&dack.service_id).map_or(0, |p| p.len());
                                if num_dacks >= MAX_DELIVERY_ACKNOWLEDGMENTS {
                                    break;
                                }
                                proofs.entry(dack.service_id).or_default().push(dack.proof);
                                *commodity.entry(dack.service_id).or_insert(0) += dack.commodity;
                                if let Some(data) = &dack.metadata {
                                    metadata
                                        .entry(dack.service_id)
                                        .or_insert(Vec::new())
                                        .extend(data);
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize DACK: {e:?}");
                            }
                        }
                        num_dacks_taken += 1;
                    }

                    for (service_id, service_proofs) in proofs {
                        // This unwrap is safe because commodity and proofs are inserted together
                        let service_commodity = commodity.get(&service_id).unwrap();
                        let update = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                            commodity: *service_commodity,
                            service_id,
                            proofs: service_proofs,
                            metadata: metadata.remove(&service_id),
                        };
                        let submit_tx = self.submit_tx.clone();
                        spawn!(async move {
                            if let Err(e) = submit_tx
                                .run(ExecuteTransactionRequest {
                                    method: update,
                                    options: None,
                                })
                                .await
                            {
                                error!("Failed to submit DACK to signer: {e:?}");
                            }
                        },
                        "DACK_AGGREGATOR: submit tx");
                    }

                    increment_counter_by!(
                        num_dacks_taken,
                        "dack_aggregator_processed",
                        Some("Number of delivery acknowledgments aggregated")
                    );

                    // After sending the transaction, we remove the DACKs we sent from the queue.
                    (0..num_dacks_taken).for_each(|_| {
                        // TODO(matthias): should we only remove them after we verified that the transaction was
                        // ordered?
                        if let Err(e) = self.queue.remove() {
                            error!("Failed to remove DACK from queue: {e:?}");
                        }
                    });
                }
            }
        }
    }
}

impl<C: NodeComponents> ConfigConsumer for DeliveryAcknowledgmentAggregator<C> {
    const KEY: &'static str = "dack-aggregator";

    type Config = Config;
}
