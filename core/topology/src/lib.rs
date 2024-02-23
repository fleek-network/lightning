pub mod clustering;
pub mod config;
mod core;
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::anyhow;
pub use config::Config;
use divisive::DivisiveHierarchy;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::Participation;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    Notification,
    NotifierInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_utils::application::QueryRunnerExt;
use ndarray::{Array, Array2};
use rand::SeedableRng;
use tokio::sync::{mpsc, watch, Notify};
use tracing::{error, info};

#[derive(Clone)]
pub struct Topology<C: Collection> {
    inner: Arc<TopologyInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

struct TopologyInner<C: Collection> {
    query: c!(C::ApplicationInterface::SyncExecutor),
    #[allow(unused)]
    notifier_rx: Arc<Mutex<Option<mpsc::Receiver<Notification>>>>,
    topology_tx: watch::Sender<Vec<Vec<NodePublicKey>>>,
    topology_rx: watch::Receiver<Vec<Vec<NodePublicKey>>>,
    our_public_key: NodePublicKey,
    // TODO(qti3e): Use ArcSwap instead.
    current_peers: Mutex<Arc<Vec<Vec<NodePublicKey>>>>,
    // TODO(qti3e): Use a single AtomicU64 instead.
    current_epoch: Mutex<u64>,
    target_k: usize,
    min_nodes: usize,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> TopologyInner<C> {
    /// Build a latency matrix according to the current application state.
    /// Returns the matrix, a map of node ids to public keys, and an optional node index for
    /// ourselves if we're included in the topology.
    fn build_latency_matrix(&self) -> (Array2<i32>, HashMap<usize, NodePublicKey>, Option<usize>) {
        let latencies = self.query.get_current_latencies();
        let valid_pubkeys: BTreeSet<NodePublicKey> = self
            .query
            .get_node_registry(None)
            .into_iter()
            .filter(|node_info| node_info.info.participation == Participation::True)
            .map(|node_info| node_info.info.public_key)
            .collect();

        let mut max_latency = Duration::ZERO;
        latencies
            .values()
            .for_each(|latency| max_latency = max_latency.max(*latency));
        let max_latency: i32 = max_latency.as_millis().try_into().unwrap_or(i32::MAX);

        let mut matrix = Array::zeros((valid_pubkeys.len(), valid_pubkeys.len()));
        let pubkeys: Vec<(usize, NodePublicKey)> =
            valid_pubkeys.iter().copied().enumerate().collect();

        let mut our_index = None;
        let mut index_to_pubkey = HashMap::new();
        for (index_lhs, pubkey_lhs) in pubkeys.iter() {
            index_to_pubkey.insert(*index_lhs, *pubkey_lhs);
            if *pubkey_lhs == self.our_public_key {
                our_index = Some(*index_lhs);
            }
            for (index_rhs, pubkey_rhs) in pubkeys[index_lhs + 1..].iter() {
                if let Some(latency) = latencies.get(&(*pubkey_lhs, *pubkey_rhs)) {
                    let latency: i32 = latency.as_millis().try_into().unwrap_or(i32::MAX);
                    matrix[[*index_lhs, *index_rhs]] = latency;
                    matrix[[*index_rhs, *index_lhs]] = latency;
                } else {
                    matrix[[*index_lhs, *index_rhs]] = max_latency;
                    matrix[[*index_rhs, *index_lhs]] = max_latency;
                }
            }
        }

        (matrix, index_to_pubkey, our_index)
    }

    async fn suggest_connections_new(&self) -> anyhow::Result<Vec<Vec<NodePublicKey>>> {
        let epoch = self.query.get_current_epoch();
        let our_public_key = self.our_public_key;
        let latencies = self.query.get_current_latencies();
        let valid_pubkeys: BTreeSet<NodePublicKey> = self
            .query
            .get_node_registry(None)
            .into_iter()
            .filter(|node_info| node_info.info.participation == Participation::True)
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

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        let epoch = self.query.get_current_epoch();
        let mut current = self.current_peers.lock().expect("failed to acquire lock");
        let mut current_epoch = self.current_epoch.lock().expect("failed to acquire lock");

        // TODO(qti3e): This computation can be heavy. Right now only broadcast is calling this
        // function and we can put the call to `suggest_connections` in a blocking thread on the
        // caller side. But I regret having the topology poll based. It was a mistake.
        //
        // We should have the topology as a reactive source of data so we can put this heavy
        // computation on a different thread here from within the origin of the computation.

        // if the epoch has changed, or the object has been newly initialized
        if *current_epoch < epoch || *current_epoch == u64::MAX {
            let (matrix, mappings, our_index) = self.build_latency_matrix();

            *current = if let Some(our_index) = our_index {
                // Included in the topology: collect assignments and build output
                if mappings.len() < self.min_nodes {
                    // Fallback to returning all nodes, since we're less than the minimum
                    info!("All nodes connect to each other");
                    vec![mappings.into_values().collect()]
                } else {
                    info!("Form hierarchical clusters");
                    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(epoch);
                    let hierarchy = DivisiveHierarchy::new(&mut rng, &matrix, self.target_k);

                    hierarchy.connections()[our_index]
                        .iter()
                        .map(|ids| ids.iter().map(|idx| mappings[idx]).collect())
                        .collect()
                }
            } else {
                // Not in the topology: return all nodes to bootstrap from
                vec![mappings.into_values().collect()]
            }
            .into();

            *current_epoch = epoch;
        }

        current.clone()
    }

    async fn start(&self) {
        let mut notifier_rx = self.notifier_rx.lock().unwrap().take().unwrap();
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                notify = notifier_rx.recv() => {
                    let Some(notify) = notify else {
                        info!("Notifier was dropped");
                        break;
                    };
                    if let Notification::NewEpoch = notify {
                        // This only fails if joining the blocking task fails, which only
                        // happens if something is already wrong.
                        let conns = self.suggest_connections_new().await.expect("Failed to compute topology");
                        if let Err(e) = self.topology_tx.send(conns) {
                            error!("All receivers have been dropped: {e:?}");
                        }
                    }
                }
            }
        }
        *self.notifier_rx.lock().unwrap() = Some(notifier_rx);
    }
}

impl<C: Collection> TopologyInterface<C> for Topology<C> {
    fn init(
        config: Self::Config,
        our_public_key: NodePublicKey,
        notifier: C::NotifierInterface,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        let (notifier_tx, notifier_rx) = mpsc::channel(16);
        notifier.notify_on_new_epoch(notifier_tx);

        let (topology_tx, topology_rx) = watch::channel(Vec::new());
        let shutdown_notify = Arc::new(Notify::new());
        let inner = TopologyInner {
            target_k: config.testing_target_k,
            min_nodes: config.testing_min_nodes,
            query: query_runner,
            notifier_rx: Arc::new(Mutex::new(Some(notifier_rx))),
            topology_tx,
            topology_rx,
            current_epoch: Mutex::new(u64::MAX),
            current_peers: Mutex::new(Arc::new(Vec::new())),
            our_public_key,
            shutdown_notify: shutdown_notify.clone(),
        };
        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
        })
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        self.inner.suggest_connections()
    }

    fn get_receiver(&self) -> watch::Receiver<Vec<Vec<NodePublicKey>>> {
        self.inner.topology_rx.clone()
    }
}

impl<C: Collection> WithStartAndShutdown for Topology<C> {
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
            error!("Cannot start topology because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

impl<C: Collection> ConfigConsumer for Topology<C> {
    type Config = Config;

    const KEY: &'static str = "topology";
}
