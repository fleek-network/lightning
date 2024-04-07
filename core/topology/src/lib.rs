pub mod clustering;
pub mod config;
mod core;
pub use core::{build_latency_matrix, suggest_connections_from_latency_matrix, Connections};
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
pub use config::Config;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::fdi::{self, BuildGraph, DependencyGraph, MethodExt};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    KeystoreInterface,
    Notification,
    ShutdownWaiter,
    TopologyInterface,
};
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::{mpsc, watch};
use tracing::error;

pub struct Topology<C: Collection> {
    inner: Arc<TopologyInner<C>>,
}

struct TopologyInner<C: Collection> {
    query: c!(C::ApplicationInterface::SyncExecutor),
    notifier_rx: Arc<Mutex<Option<mpsc::Receiver<Notification>>>>,
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

        let mut notifier_rx = self.notifier_rx.lock().unwrap().take().unwrap();
        while let Some(notify) = notifier_rx.recv().await {
            if let Notification::NewEpoch = notify {
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

        *self.notifier_rx.lock().unwrap() = Some(notifier_rx);
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
        _notifier: &C::NotifierInterface,
        fdi::Cloned(query): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();
        let (_notifier_tx, notifier_rx) = mpsc::channel(16);
        // TODO(qti3e): Use the new notifier.
        // notifier.notify_on_new_epoch(notifier_tx);

        let (topology_tx, topology_rx) = watch::channel(Arc::new(Vec::new()));

        let inner = TopologyInner {
            target_k: config.testing_target_k,
            min_nodes: config.testing_min_nodes,
            query,
            notifier_rx: Arc::new(Mutex::new(Some(notifier_rx))),
            topology_tx,
            topology_rx,
            our_public_key: signer.get_ed25519_pk(),
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    async fn start(this: fdi::Ref<Self>, waiter: fdi::Cloned<ShutdownWaiter>) {
        let inner = this.inner.clone();
        drop(this);

        waiter.run_until_shutdown(inner.start()).await;
    }
}

impl<C: Collection> BuildGraph for Topology<C> {
    fn build_graph() -> DependencyGraph {
        DependencyGraph::default().with(Self::init.on("start", Self::start.spawn()))
    }
}

impl<C: Collection> ConfigConsumer for Topology<C> {
    type Config = Config;

    const KEY: &'static str = "topology";
}
