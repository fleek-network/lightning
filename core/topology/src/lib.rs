pub mod clustering;
pub mod config;
mod core;
pub use core::{build_latency_matrix, suggest_connections_from_latency_matrix, Connections};
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
pub use config::Config;
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
use tokio::sync::{mpsc, watch, Notify};
use tracing::{error, info};

pub struct Topology<C: Collection> {
    inner: Arc<TopologyInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

struct TopologyInner<C: Collection> {
    query: c!(C::ApplicationInterface::SyncExecutor),
    notifier_rx: Arc<Mutex<Option<mpsc::Receiver<Notification>>>>,
    topology_tx: watch::Sender<Arc<Vec<Vec<NodePublicKey>>>>,
    topology_rx: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    our_public_key: NodePublicKey,
    target_k: usize,
    min_nodes: usize,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> TopologyInner<C> {
    async fn suggest_connections(&self) -> anyhow::Result<Vec<Vec<NodePublicKey>>> {
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

    async fn start(&self) {
        let conns = self
            .suggest_connections()
            .await
            .expect("Failed to compute topology");
        if let Err(e) = self.topology_tx.send(Arc::new(conns)) {
            error!("All receivers have been dropped: {e:?}");
        }
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
                        let conns = self.suggest_connections().await.expect("Failed to compute topology");
                        if let Err(e) = self.topology_tx.send(Arc::new(conns)) {
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

        let (topology_tx, topology_rx) = watch::channel(Arc::new(Vec::new()));
        let shutdown_notify = Arc::new(Notify::new());
        let inner = TopologyInner {
            target_k: config.testing_target_k,
            min_nodes: config.testing_min_nodes,
            query: query_runner,
            notifier_rx: Arc::new(Mutex::new(Some(notifier_rx))),
            topology_tx,
            topology_rx,
            our_public_key,
            shutdown_notify: shutdown_notify.clone(),
        };
        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
        })
    }

    fn get_receiver(&self) -> watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>> {
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
