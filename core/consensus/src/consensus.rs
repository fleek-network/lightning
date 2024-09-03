use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use derive_more::{From, IsVariant, TryInto};
use fastcrypto::bls12381::min_sig::BLS12381PrivateKey;
use fastcrypto::ed25519::Ed25519PrivateKey;
use fleek_crypto::{ConsensusPublicKey, NodePublicKey, SecretKey};
use gethostname::gethostname;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Epoch, EpochInfo, Topic, UpdateMethod};
use lightning_interfaces::Events;
use lightning_utils::application::QueryRunnerExt;
use mysten_metrics::RegistryService;
use mysten_network::Multiaddr;
use narwhal_config::{Committee, CommitteeBuilder, WorkerCache, WorkerIndex, WorkerInfo};
use narwhal_crypto::traits::{KeyPair as _, ToFromBytes};
use narwhal_crypto::{KeyPair, NetworkKeyPair, NetworkPublicKey, PublicKey};
use narwhal_node::NodeStorage;
use prometheus::Registry;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::{pin, select, task, time};
use tracing::{error, info};
use typed_store::DBMetrics;

use crate::config::Config;
use crate::execution::parcel::{AuthenticStampedParcel, CommitteeAttestation, Digest};
use crate::execution::state::FilteredConsensusOutput;
use crate::execution::worker::ExecutionWorker;
use crate::narwhal::{NarwhalArgs, NarwhalService};

pub struct Consensus<C: Collection> {
    /// Inner state of the consensus
    #[allow(clippy::type_complexity)]
    epoch_state: Option<
        EpochState<
            c![C::ApplicationInterface::SyncExecutor],
            c![C::BroadcastInterface::PubSub<PubSubMsg>],
            c![C::NotifierInterface::Emitter],
        >,
    >,
    /// To send the event sender to the execution worker.
    event_tx_tx: Option<oneshot::Sender<Events>>,
    /// Timestamp of the narwhal certificate that caused an epoch change
    /// is sent through this channel to notify that epoch chould change.
    reconfigure_notify: Arc<Notify>,
    /// To notify the epoch state when consensus is shutting down
    shutdown_notify_epoch_state: Arc<Notify>,
}

/// This struct contains mutable state only for the current epoch.
struct EpochState<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg> + 'static, NE: Emitter> {
    /// Execute transactions.
    executor: ExecutionEngineSocket,
    /// The node public key of the node.
    node_public_key: NodePublicKey,
    /// The consensus public key of the node.
    consensus_public_key: ConsensusPublicKey,
    /// The Narwhal service for the current epoch.
    consensus: Option<NarwhalService>,
    /// Used to query the application data
    query_runner: Q,
    /// This narwhal node data
    narwhal_args: NarwhalArgs,
    /// Notifications emitter
    notifier: NE,
    /// Path to the database used by the narwhal implementation
    pub store_path: ResolvedPathBuf,
    /// Used to send transactions to consensus
    /// We still use this socket on consensus struct because a node is not always on the committee,
    /// so its not always sending     a transaction to its own mempool. The signer interface
    /// also takes care of nonce bookkeeping and retry logic
    txn_socket: SubmitTxSocket,
    /// Interface for sending messages through the gossip layer
    pub_sub: P,
    /// Send consensus output from the execution state to the execution worker.
    consensus_output_tx: Sender<FilteredConsensusOutput>,
    /// Receive consensus output in the execution worker from the execution state.
    consensus_output_rx: Option<Receiver<FilteredConsensusOutput>>,
    /// Receive the rpc event sender in the execution worker.
    event_tx_rx: Option<oneshot::Receiver<Events>>,
    /// To notify when consensus is shutting down.
    shutdown_notify: Arc<Notify>,
}

#[allow(clippy::too_many_arguments)]
impl<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg> + 'static, NE: Emitter>
    EpochState<Q, P, NE>
{
    fn new(
        executor: ExecutionEngineSocket,
        node_public_key: NodePublicKey,
        consensus_public_key: ConsensusPublicKey,
        query_runner: Q,
        narwhal_args: NarwhalArgs,
        store_path: ResolvedPathBuf,
        txn_socket: SubmitTxSocket,
        notifier: NE,
        pub_sub: P,
        consensus_output_tx: Sender<FilteredConsensusOutput>,
        consensus_output_rx: Receiver<FilteredConsensusOutput>,
        event_tx_rx: oneshot::Receiver<Events>,
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            executor,
            node_public_key,
            consensus_public_key,
            consensus: None,
            query_runner,
            narwhal_args,
            notifier,
            store_path,
            txn_socket,
            pub_sub,
            consensus_output_tx,
            consensus_output_rx: Some(consensus_output_rx),
            event_tx_rx: Some(event_tx_rx),
            shutdown_notify,
        }
    }

    fn spawn_execution_worker(&mut self, reconfigure_notify: Arc<Notify>) -> ExecutionWorker {
        ExecutionWorker::spawn::<P, Q, NE>(
            self.executor.clone(),
            self.consensus_output_rx
                .take()
                .expect("consensus_output_rx is missing"),
            self.pub_sub.clone(),
            self.query_runner.clone(),
            self.narwhal_args
                .primary_network_keypair
                .public()
                .to_owned()
                .into(),
            reconfigure_notify,
            self.notifier.clone(),
            self.event_tx_rx.take().expect("event_tx_rx is missing"),
        )
    }

    async fn start_current_epoch(&mut self) {
        // Get current epoch information
        let (committee, worker_cache, epoch, epoch_end) = self.get_epoch_info();

        if committee
            .authority_by_key(self.narwhal_args.primary_keypair.public())
            .is_some()
        {
            self.run_narwhal(epoch_end, epoch, committee, worker_cache)
                .await
        }
    }

    async fn move_to_next_epoch(&mut self) {
        if let Some(state) = self.consensus.take() {
            state.shutdown().await
        }

        self.start_current_epoch().await
    }

    fn get_epoch_info(&self) -> (Committee, WorkerCache, u64, u64) {
        let EpochInfo {
            committee,
            epoch,
            epoch_end,
        } = self.query_runner.get_epoch_info();

        let mut committee_builder = CommitteeBuilder::new(epoch);

        for node in &committee {
            // TODO(dalton) This check should be done at application before adding it to state. So
            // it should never not be Ok so even an unwrap should be safe here
            if let (Ok(address), Ok(consensus_key), Ok(public_key)) = (
                Multiaddr::try_from(format!("/ip4/{}/udp/{}", node.domain, node.ports.primary)),
                PublicKey::from_bytes(&node.consensus_key.0),
                NetworkPublicKey::from_bytes(&node.public_key.0),
            ) {
                committee_builder = committee_builder.add_authority(
                    consensus_key,
                    1,
                    address,
                    public_key,
                    gethostname().to_string_lossy().into_owned(),
                );
            }
        }
        let narwhal_committee = committee_builder.build();

        // TODO(dalton): We need to handle an ip6 scenario when parsing these multiaddrs
        let worker_cache = WorkerCache {
            epoch,
            workers: committee
                .iter()
                .filter_map(|node| {
                    let mut worker_index = BTreeMap::new();

                    if let (Ok(node_key), Ok(key), Ok(address), Ok(mempool)) = (
                        PublicKey::from_bytes(&node.consensus_key.0),
                        NetworkPublicKey::from_bytes(&node.worker_public_key.0),
                        Multiaddr::try_from(format!(
                            "/ip4/{}/udp/{}/http",
                            node.worker_domain, node.ports.worker
                        )),
                        Multiaddr::try_from(format!(
                            "/ip4/{}/tcp/{}/http",
                            node.worker_domain, node.ports.mempool
                        )),
                    ) {
                        worker_index.insert(
                            0u32,
                            WorkerInfo {
                                name: key,
                                transactions: mempool,
                                worker_address: address,
                            },
                        );
                        Some((node_key, WorkerIndex(worker_index)))
                    } else {
                        None
                    }
                })
                .collect(),
        };

        (narwhal_committee, worker_cache, epoch, epoch_end)
    }

    fn wait_to_signal_epoch_change(&self, mut time_until_change: Duration, epoch: Epoch) {
        let txn_socket = self.txn_socket.clone();
        let query_runner = self.query_runner.clone();

        let shutdown = self.shutdown_notify.clone();
        task::spawn(async move {
            let shutdown_fut = shutdown.notified();
            pin!(shutdown_fut);
            loop {
                let time_to_sleep = time::sleep(time_until_change);

                tokio::select! {
                    biased;
                    _ = &mut shutdown_fut => {
                        tracing::debug!("shutdown signal received, stopping");
                        break;
                    }
                    _ = time_to_sleep => {
                        let new_epoch = query_runner.get_current_epoch();
                        if new_epoch != epoch {
                            break;
                        }

                        info!("Narwhal: Signalling ready to change epoch");

                        if let Err(e) = txn_socket
                        .enqueue(UpdateMethod::ChangeEpoch { epoch })
                        .await {
                            error!("Error sending change epoch signal to socket {}", e);
                        }

                        time_until_change = Duration::from_secs(120);
                    },
                }
            }
        });
    }

    async fn run_narwhal(
        &mut self,
        epoch_end: u64,
        epoch: u64,
        committee: Committee,
        worker_cache: WorkerCache,
    ) {
        info!("Node is on current committee, starting narwhal.");

        let store = self.get_narwhal_store_and_garbage_collect(epoch);

        // Create the narwhal service
        let service = NarwhalService::new(
            self.node_public_key,
            self.consensus_public_key,
            self.narwhal_args.clone(),
            store,
            committee,
            worker_cache,
        );

        service
            .start(self.consensus_output_tx.clone(), self.query_runner.clone())
            .await;

        // start the timer to signal when your node thinks its ready to change epochs
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let until_epoch_ends: u64 = (epoch_end as u128).saturating_sub(now).try_into().unwrap();
        let time_until_epoch_change = Duration::from_millis(until_epoch_ends);

        self.wait_to_signal_epoch_change(time_until_epoch_change, epoch);

        self.consensus = Some(service)
    }

    pub fn shutdown(&self) {
        self.executor.downgrade();
    }

    /// Creates or reopens a narwhal store specific to current epoch. Also garbage collects stores
    /// that are older than 2 epochs
    fn get_narwhal_store_and_garbage_collect(&self, current_epoch: u64) -> NodeStorage {
        let mut store_path = self.store_path.to_path_buf();

        // Delete any directories that are from more than 2 epochs back
        garbage_collect_old_stores(&current_epoch, &store_path, 2);

        store_path.push(format!("{current_epoch}"));
        // TODO(dalton): This store takes an optional cache metrics struct that can give us metrics
        // on hits/miss
        NodeStorage::reopen(store_path, None)
    }
}

impl<C: Collection> Consensus<C> {
    /// Start the system, should not do anything if the system is already
    /// started.
    fn start(
        &mut self,
        app_query: fdi::Cloned<c![C::ApplicationInterface::SyncExecutor]>,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) {
        let reconfigure_notify = self.reconfigure_notify.clone();
        let shutdown_notify_epoch_state = self.shutdown_notify_epoch_state.clone();

        let mut epoch_state = self
            .epoch_state
            .take()
            .expect("Consensus was tried to start before initialization");

        let panic_waiter = waiter.clone();
        let app_query = app_query.clone();
        spawn!(
            async move {
                // Wait for genesis to be applied before starting the consensus.
                if !app_query.wait_for_genesis().await {
                    return;
                }

                let execution_worker =
                    epoch_state.spawn_execution_worker(reconfigure_notify.clone());
                epoch_state.start_current_epoch().await;

                let shutdown_future = waiter.wait_for_shutdown();
                pin!(shutdown_future);

                loop {
                    let reconfigure_future = reconfigure_notify.notified();

                    select! {
                        biased;
                        _ = &mut shutdown_future => {
                            if let Some(consensus) = epoch_state.consensus.take() {
                                consensus.shutdown().await;
                            }
                            execution_worker.shutdown().await;
                            epoch_state.shutdown();
                            break
                        }
                        _ = reconfigure_future => {
                            epoch_state.move_to_next_epoch().await;
                            continue
                        }
                    }
                }

                // Notify the epoch state that it is time to shutdown.
                shutdown_notify_epoch_state.notify_waiters();
            },
            "CONSENSUS",
            crucial(panic_waiter)
        );
    }
}

impl<C: Collection> ConfigConsumer for Consensus<C> {
    const KEY: &'static str = "consensus";
    type Config = Config;
}

impl<C: Collection> BuildGraph for Consensus<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(
            Self::init
                .with_event_handler("_post", Self::post_init)
                .with_event_handler("start", Self::start),
        )
    }
}

impl<C: Collection> ConsensusInterface<C> for Consensus<C> {
    type Certificate = PubSubMsg;
}

impl<C: Collection> Consensus<C> {
    /// Create a new consensus service with the provided config and executor.
    fn init(
        config_provider: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        signer: &C::SignerInterface,
        app: &C::ApplicationInterface,
        broadcast: &C::BroadcastInterface,
        notifier: &C::NotifierInterface,
    ) -> anyhow::Result<Self> {
        let config = config_provider.get::<Self>();
        let executor = app.transaction_executor();
        let query_runner = app.sync_query();
        let pubsub = broadcast.get_pubsub(Topic::Consensus);

        // Spawn the registry for narwhal
        let registry = Registry::new();
        // Init the metrics for narwhal
        DBMetrics::init(&registry);

        let (consensus_sk, primary_sk) = (keystore.get_bls_sk(), keystore.get_ed25519_sk());
        let (consensus_pk, primary_pk) = (consensus_sk.to_pk(), primary_sk.to_pk());
        let reconfigure_notify = Arc::new(Notify::new());
        let networking_keypair = NetworkKeyPair::from(Ed25519PrivateKey::from(&primary_sk));
        let primary_keypair = KeyPair::from(BLS12381PrivateKey::from(&consensus_sk));
        let narwhal_args = NarwhalArgs {
            primary_keypair,
            primary_network_keypair: networking_keypair.copy(),
            worker_keypair: networking_keypair,
            registry_service: RegistryService::new(registry),
        };

        // Todo(dalton): Figure out better default channel size
        let (consensus_output_tx, consensus_output_rx) = mpsc::channel(1000);
        let (event_tx_tx, event_tx_rx) = oneshot::channel();

        let shutdown_notify_epoch_state = Arc::new(Notify::new());

        let epoch_state = EpochState::new(
            executor,
            primary_pk,
            consensus_pk,
            query_runner,
            narwhal_args,
            config.store_path,
            signer.get_socket(),
            notifier.get_emitter(),
            pubsub,
            consensus_output_tx,
            consensus_output_rx,
            event_tx_rx,
            shutdown_notify_epoch_state.clone(),
        );

        Ok(Self {
            epoch_state: Some(epoch_state),
            event_tx_tx: Some(event_tx_tx),
            reconfigure_notify,
            shutdown_notify_epoch_state,
        })
    }

    fn post_init(&mut self, rpc: &C::RpcInterface) {
        self.event_tx_tx
            .take()
            .expect("Tried to start consensus before initialization")
            .send(rpc.event_tx())
            .expect("Failed to send events sender to execution worker");
    }
}

/// Delete any epoch directories that are more than `retention` epochs old
/// We dont want to panic if this fails but we should print an error
fn garbage_collect_old_stores(current_epoch: &u64, store_location: &PathBuf, retention: u64) {
    if current_epoch < &retention {
        return;
    }
    if let Ok(files) = fs::read_dir(store_location) {
        for file in files.flatten() {
            // Every narwhal db is store in this directory with the number of the epoch as its
            // name
            if let Some(epoch_num) = file
                .file_name()
                .into_string()
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
            {
                if epoch_num < current_epoch - retention {
                    if let Err(e) = fs::remove_dir_all(file.path()) {
                        error!("Unable to remove garbage collected Narwhal epoch: {e}");
                    }
                }
            }
        }
    } else {
        error!("Unable to read narwhal store directory to garbage collect old databases");
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, IsVariant, From, TryInto)]
pub enum PubSubMsg {
    Transactions(AuthenticStampedParcel),
    Attestation(CommitteeAttestation),
    RequestTransactions(Digest),
}

impl AutoImplSerde for PubSubMsg {}

// Tests to make sure the right directories are being deleted when running the
// garbage_collect_old_stores function.
#[cfg(test)]
mod test_garbage_collect {
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;

    fn create_epoch_directory(path: &Path, epoch: u64) {
        let mut path = path.to_path_buf();
        path.push(format!("{epoch}"));
        path.push("directory1");
        fs::create_dir_all(path.clone()).unwrap();
        path.set_file_name("file1.txt");
        fs::File::create(path.clone()).unwrap();
        path.set_file_name("file2.txt");
        fs::File::create(path.clone()).unwrap();
        path.set_file_name("file3.txt");
        fs::File::create(path).unwrap();
    }

    #[test]
    fn with_0_epochs_2_retention() {
        let temp_dir = tempdir().unwrap();
        let store_path = temp_dir.into_path().to_path_buf();

        garbage_collect_old_stores(&0, &store_path.to_path_buf(), 2);

        assert_eq!(fs::read_dir(store_path).unwrap().count(), 0);
    }

    #[test]
    fn with_1_epoch_2_retention() {
        let temp_dir = tempdir().unwrap();
        let store_path = temp_dir.into_path();
        create_epoch_directory(&store_path, 0);

        garbage_collect_old_stores(&1, &store_path.to_path_buf(), 2);

        assert_eq!(fs::read_dir(&store_path).unwrap().count(), 1);
        assert!(store_path.join("0").exists());
    }

    #[test]
    fn with_2_epochs_2_retention() {
        let temp_dir = tempdir().unwrap();
        let store_path = temp_dir.into_path();
        create_epoch_directory(&store_path, 0);
        create_epoch_directory(&store_path, 1);

        garbage_collect_old_stores(&2, &store_path.to_path_buf(), 2);

        assert_eq!(fs::read_dir(&store_path).unwrap().count(), 2);
        assert!(store_path.join("0").exists());
        assert!(store_path.join("1").exists());
    }

    #[test]
    fn with_3_epochs_2_retention() {
        let temp_dir = tempdir().unwrap();
        let store_path = temp_dir.into_path();
        create_epoch_directory(&store_path, 0);
        create_epoch_directory(&store_path, 1);
        create_epoch_directory(&store_path, 2);

        garbage_collect_old_stores(&3, &store_path.to_path_buf(), 2);

        assert_eq!(fs::read_dir(&store_path).unwrap().count(), 2);
        assert!(!store_path.join("0").exists());
        assert!(store_path.join("1").exists());
        assert!(store_path.join("2").exists());
    }

    #[test]
    fn with_10_epochs_2_retention() {
        // Create a fake store directory and fill it with 10 directories to simulate 10 epoch
        // directories
        let temp_dir = tempdir().unwrap();
        let store_path = temp_dir.into_path();
        for i in 0..=10 {
            create_epoch_directory(&store_path, i);
        }

        // Garbage collect the directory
        garbage_collect_old_stores(&10, &store_path.to_path_buf(), 2);

        // Ensure that that there is only 3 directories left. One for current epoch and for the
        // previous 2 epochs
        for i in 0..=10 {
            let mut dir_path = store_path.to_path_buf();
            dir_path.push(format!("{i}"));
            if i < 8 {
                assert!(!dir_path.exists());
            } else {
                assert!(dir_path.exists());
            }
        }
    }
}
