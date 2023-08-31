use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use affair::{Executor, TokioSpawn};
use async_trait::async_trait;
use derive_more::{From, IsVariant, TryInto};
use lightning_interfaces::application::ExecutionEngineSocket;
use lightning_interfaces::common::WithStartAndShutdown;
use lightning_interfaces::config::ConfigConsumer;
use lightning_interfaces::consensus::{ConsensusInterface, MempoolSocket};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::signer::{SignerInterface, SubmitTxSocket};
use lightning_interfaces::types::{Epoch, EpochInfo, UpdateMethod};
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    PubSub,
    SyncQueryRunnerInterface,
};
use lightning_schema::AutoImplSerde;
use log::{error, info};
use mysten_metrics::RegistryService;
use mysten_network::Multiaddr;
use narwhal_config::{Committee, CommitteeBuilder, WorkerCache, WorkerIndex, WorkerInfo};
use narwhal_crypto::traits::{KeyPair as _, ToFromBytes};
use narwhal_crypto::{KeyPair, NetworkKeyPair, NetworkPublicKey, PublicKey};
use narwhal_node::NodeStorage;
use prometheus::Registry;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::{pin, select, task, time};
use typed_store::DBMetrics;

use crate::config::Config;
use crate::edge_node::consensus::EdgeConsensus;
use crate::execution::{AuthenticStampedParcel, CommitteeAttestation, Execution};
use crate::forwarder::Forwarder;
use crate::narwhal::{NarwhalArgs, NarwhalService};

pub struct Consensus<C: Collection> {
    /// Inner state of the consensus
    /// todo(dalton): We can probably get a little more effecient then a mutex here
    /// maybe a once box
    #[allow(clippy::type_complexity)]
    epoch_state: Mutex<
        Option<
            EpochState<
                c![C::ApplicationInterface::SyncExecutor],
                c![C::BroadcastInterface::PubSub<PubSubMsg>],
            >,
        >,
    >,
    /// This socket recieves signed transactions and forwards them to an active committee member to
    /// be ordered
    mempool_socket: MempoolSocket,
    /// Timestamp of the narwhal certificate that caused an epoch change
    /// is sent through this channel to notify that epoch chould change.
    reconfigure_notify: Arc<Notify>,
    /// A notifier that is notified everytime a new block is proccessed
    new_block_notify: Arc<Notify>,
    /// Called from the shutdown function to notify the start event loop to
    /// exit.
    shutdown_notify: Arc<Notify>,
    /// bool indicating if narwhal is running
    is_running: AtomicBool,
}

/// This struct contains mutable state only for the current epoch.
struct EpochState<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg> + 'static> {
    /// The Narwhal service for the current epoch.
    consensus: Option<NarwhalService>,
    /// Used to query the application data
    query_runner: Q,
    /// This narwhal node data
    narwhal_args: NarwhalArgs,
    /// Path to the database used by the narwhal implementation
    pub store_path: ResolvedPathBuf,
    /// Narwhal execution state.
    execution_state: Arc<Execution>,
    /// Used to send transactions to consensus
    /// We still use this socket on consensus struct because a node is not always on the committee,
    /// so its not always sending     a transaction to its own mempool. The signer interface
    /// also takes care of nonce bookkeeping and retry logic
    txn_socket: SubmitTxSocket,
    /// Interface for sending messages through the gossip layer
    pub_sub: P,
    /// Narhwal sends payloads ready for broadcast to this reciever
    rx_narwhal_batches: Option<mpsc::Receiver<(AuthenticStampedParcel, bool)>>,
}

impl<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg> + 'static> EpochState<Q, P> {
    fn new(
        query_runner: Q,
        narwhal_args: NarwhalArgs,
        store_path: ResolvedPathBuf,
        execution_state: Arc<Execution>,
        txn_socket: SubmitTxSocket,
        pub_sub: P,
        rx_narwhal_batches: mpsc::Receiver<(AuthenticStampedParcel, bool)>,
    ) -> Self {
        Self {
            consensus: None,
            query_runner,
            narwhal_args,
            store_path,
            execution_state,
            txn_socket,
            pub_sub,
            rx_narwhal_batches: Some(rx_narwhal_batches),
        }
    }

    fn spawn_edge_consensus(&mut self) -> EdgeConsensus {
        EdgeConsensus::spawn(
            self.pub_sub.clone(),
            self.execution_state.clone(),
            self.query_runner.clone(),
            self.narwhal_args
                .primary_network_keypair
                .public()
                .to_owned()
                .into(),
            self.rx_narwhal_batches
                .take()
                .expect("rx_narwhal_batches missing from EpochState"),
        )
    }

    async fn start_current_epoch(&mut self) {
        // Get current epoch information
        let (committee, worker_cache, epoch, epoch_end) = self.get_epoch_info();

        // Make or open store specific to current epoch
        let mut store_path = self.store_path.clone();
        store_path.push(format!("{epoch}"));
        // TODO(dalton): This store takes an optional cache metrics struct that can give us metrics
        // on hits/miss
        let store = NodeStorage::reopen(store_path, None);

        if committee
            .authority_by_key(self.narwhal_args.primary_keypair.public())
            .is_some()
        {
            self.run_narwhal(store, epoch_end, epoch, committee, worker_cache)
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
                Multiaddr::try_from(format!(
                    "/ip4/{}/udp/{}",
                    node.domain.to_string(),
                    node.ports.primary
                )),
                PublicKey::from_bytes(&node.consensus_key.0),
                NetworkPublicKey::from_bytes(&node.public_key.0),
            ) {
                committee_builder =
                    committee_builder.add_authority(consensus_key, 1, address, public_key);
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

    async fn wait_to_signal_epoch_change(&self, time_until_change: Duration, epoch: Epoch) {
        let txn_socket = self.txn_socket.clone();
        task::spawn(async move {
            time::sleep(time_until_change).await;
            info!("Narwhal: Signalling ready to change epoch");
            // We shouldnt panic here lets repeatedly try.
            while txn_socket
                .run(UpdateMethod::ChangeEpoch { epoch })
                .await
                .is_err()
            {
                error!(
                    "Error sending change epoch transaction to signer interface, trying again..."
                );
                time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    async fn run_narwhal(
        &mut self,
        store: NodeStorage,
        epoch_end: u64,
        epoch: u64,
        committee: Committee,
        worker_cache: WorkerCache,
    ) {
        info!("Node is on current committee, starting narwhal.");
        // If you are on the committee start the timer to signal when your node thinks its ready
        // to change epochs.

        // Let the execution state know you are on the committee
        self.execution_state.set_committee_status(true);

        // Create the narwhal service
        let service =
            NarwhalService::new(self.narwhal_args.clone(), store, committee, worker_cache);

        service.start(self.execution_state.clone()).await;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let until_epoch_ends: u64 = (epoch_end as u128).saturating_sub(now).try_into().unwrap();
        let time_until_epoch_change = Duration::from_millis(until_epoch_ends);

        self.wait_to_signal_epoch_change(time_until_epoch_change, epoch)
            .await;

        self.consensus = Some(service)
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Consensus<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        let reconfigure_notify = self.reconfigure_notify.clone();
        let shutdown_notify = self.shutdown_notify.clone();

        let mut epoch_state = self
            .epoch_state
            .lock()
            .await
            .take()
            .expect("Consensus was tried to start before initialization");
        self.is_running.store(true, Ordering::Relaxed);

        task::spawn(async move {
            let edge_node = epoch_state.spawn_edge_consensus();
            epoch_state.start_current_epoch().await;
            loop {
                let reconfigure_future = reconfigure_notify.notified();
                let shutdown_future = shutdown_notify.notified();
                pin!(shutdown_future);
                pin!(reconfigure_future);

                select! {
                    _ = shutdown_future => {
                        if let Some(consensus) = epoch_state.consensus.take() {
                            consensus.shutdown().await;
                        }
                        edge_node.shutdown().await;
                        break
                    }
                    _ = reconfigure_future => {
                        epoch_state.move_to_next_epoch().await;
                        continue
                    }
                }
            }
        });
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
        self.is_running.store(false, Ordering::Relaxed);
    }
}

impl<C: Collection> ConfigConsumer for Consensus<C> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

#[async_trait]
impl<C: Collection> ConsensusInterface<C> for Consensus<C> {
    type Certificate = PubSubMsg;

    /// Create a new consensus service with the provided config and executor.
    fn init<S: SignerInterface<C>>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
    ) -> anyhow::Result<Self> {
        // Spawn the registry for narwhal
        let registry = Registry::new();
        // Init the metrics for narwhal
        DBMetrics::init(&registry);

        let (consensus_sk, primary_sk) = signer.get_sk();
        let reconfigure_notify = Arc::new(Notify::new());
        let new_block_notify = Arc::new(Notify::new());
        let networking_keypair = NetworkKeyPair::from(primary_sk);
        let primary_keypair = KeyPair::from(consensus_sk);
        let forwarder = Forwarder::new(query_runner.clone(), primary_keypair.public().clone());
        let narwhal_args = NarwhalArgs {
            primary_keypair,
            primary_network_keypair: networking_keypair.copy(),
            worker_keypair: networking_keypair,
            registry_service: RegistryService::new(registry),
        };

        // Todo(dalton): Figure out better default channel size
        let (tx_narwhal_batches, rx_narwhal_batches) = mpsc::channel(1000);

        let execution_state = Arc::new(Execution::new(
            executor,
            reconfigure_notify.clone(),
            new_block_notify.clone(),
            tx_narwhal_batches,
        ));
        let epoch_state = EpochState::new(
            query_runner,
            narwhal_args,
            config.store_path,
            execution_state,
            signer.get_socket(),
            pubsub,
            rx_narwhal_batches,
        );

        Ok(Self {
            epoch_state: Mutex::new(Some(epoch_state)),
            mempool_socket: TokioSpawn::spawn_async(forwarder),
            reconfigure_notify,
            new_block_notify,
            shutdown_notify: Arc::new(Notify::new()),
            is_running: AtomicBool::new(false),
        })
    }

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolSocket {
        self.mempool_socket.clone()
    }

    fn new_block_notifier(&self) -> Arc<Notify> {
        self.new_block_notify.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, IsVariant, From, TryInto)]
pub enum PubSubMsg {
    Transactions(AuthenticStampedParcel),
    Attestation(CommitteeAttestation),
}
impl AutoImplSerde for PubSubMsg {}
