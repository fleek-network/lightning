pub mod clustering;
pub mod config;
mod core;
pub use core::{build_latency_matrix, suggest_connections_from_latency_matrix, Connections};
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::anyhow;
pub use config::Config;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::watch;
use tracing::error;

pub struct Topology<C: Collection> {
    inner: Arc<TopologyInner<C>>,
}

struct TopologyInner<C: Collection> {
    query: c!(C::ApplicationInterface::SyncExecutor),
    notifier: C::NotifierInterface,
    topology_tx: watch::Sender<Arc<Vec<Vec<NodePublicKey>>>>,
    topology_rx: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    our_public_key: NodePublicKey,
    target_k: usize,
    min_nodes: usize,
}

impl<C: Collection> TopologyInner<C> {
    async fn suggest_connections(&self) -> anyhow::Result<Vec<Vec<NodePublicKey>>> {
        let epoch = self.query.get_current_epoch();
        let our_public_key = self.our_public_key;
        let latencies = self.query.get_current_latencies();
        let valid_pubkeys: BTreeSet<NodePublicKey> = self
            .query
            .get_active_nodes()
            .into_iter()
            .map(|node_info| node_info.info.public_key)
            .collect();
        let min_nodes = self.min_nodes;
        let target_k = self.target_k;

        // TODO(matthias): use rayon?
        tokio::task::spawn_blocking(move || {
            core::suggest_connections(
                epoch,
                our_public_key,
                latencies,
                valid_pubkeys,
                min_nodes,
                target_k,
            )
        })
        .await
        .map_err(|e| anyhow!("Failed to join blocking task: {e:?}"))
    }

    async fn start(&self) {
        let conns = self
            .suggest_connections()
            .await
            .expect("Failed to compute topology");

        if let Err(e) = self.topology_tx.send(Arc::new(conns)) {
            error!("All receivers have been dropped: {e:?}");
        }

        let mut epoch_changed_sub = self.notifier.subscribe_epoch_changed();

        while epoch_changed_sub.recv().await.is_some() {
            // This only fails if joining the blocking task fails, which only
            // happens if something is already wrong.
            let conns = self
                .suggest_connections()
                .await
                .expect("Failed to compute topology");

            if let Err(e) = self.topology_tx.send(Arc::new(conns)) {
                error!("All receivers have been dropped: {e:?}");
            }
        }
    }
}

impl<C: Collection> TopologyInterface<C> for Topology<C> {
    fn get_receiver(&self) -> watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>> {
        self.inner.topology_rx.clone()
    }
}

impl<C: Collection> Topology<C> {
    fn init(
        config: &C::ConfigProviderInterface,
        signer: &C::KeystoreInterface,
        fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
        fdi::Cloned(query): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();
        let (topology_tx, topology_rx) = watch::channel(Arc::new(Vec::new()));
        let inner = TopologyInner {
            target_k: config.testing_target_k,
            notifier,
            min_nodes: config.testing_min_nodes,
            query,
            topology_tx,
            topology_rx,
            our_public_key: signer.get_ed25519_pk(),
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    async fn start(this: fdi::Ref<Self>, waiter: fdi::Cloned<ShutdownWaiter>) {
        this.inner.query.wait_for_genesis().await;

        let inner = this.inner.clone();
        drop(this);

        waiter.run_until_shutdown(inner.start()).await;
    }
}

impl<C: Collection> BuildGraph for Topology<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(
            Self::init.with_event_handler("start", Self::start.wrap_with_spawn_named("TOPOLOGY")),
        )
    }
}

impl<C: Collection> ConfigConsumer for Topology<C> {
    type Config = Config;

    const KEY: &'static str = "topology";
}
