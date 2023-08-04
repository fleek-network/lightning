use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use affair::{Executor, TokioSpawn};
use async_trait::async_trait;
use derive_more::{From, IsVariant, TryInto};
use lightning_interfaces::{
    application::ExecutionEngineSocket,
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    consensus::{ConsensusInterface, MempoolSocket},
    signer::{SignerInterface, SubmitTxSocket},
    types::{Epoch, EpochInfo, UpdateMethod},
    PubSub, SyncQueryRunnerInterface,
};
use lightning_schema::AutoImplSerde;
use log::{error, info};
use mysten_metrics::RegistryService;
use mysten_network::Multiaddr;
use narwhal_config::{Committee, CommitteeBuilder, WorkerCache, WorkerIndex, WorkerInfo};
use narwhal_crypto::{
    traits::{KeyPair as _, ToFromBytes},
    KeyPair, NetworkKeyPair, NetworkPublicKey, PublicKey,
};
use narwhal_node::NodeStorage;
use prometheus::Registry;
use serde::{Deserialize, Serialize};
use tokio::{
    pin, select,
    sync::{Mutex, Notify},
    task, time,
};

use crate::{
    config::Config,
    edge_node::EdgeService,
    execution::Execution,
    forwarder::Forwarder,
    narwhal::{NarwhalArgs, NarwhalService},
};

pub struct Consensus<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg> + 'static> {
    /// Inner state of the consensus
    /// todo(dalton): We can probably get a little more effecient then a mutex here
    /// maybe a once box
    epoch_state: Mutex<Option<EpochState<Q, P>>>,
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
    consensus: Option<ConsensusService<P>>,
    /// Used to query the application data
    query_runner: Q,
    /// This narwhal node data
    narwhal_args: NarwhalArgs,
    /// Path to the database used by the narwhal implementation
    pub store_path: PathBuf,
    /// Narwhal execution state.
    execution_state: Arc<Execution<P>>,
    /// Used to send transactions to consensus
    /// We still use this socket on consensus struct because a node is not always on the committee,
    /// so its not always sending     a transaction to its own mempool. The signer interface
    /// also takes care of nonce bookkeeping and retry logic
    txn_socket: SubmitTxSocket,
    /// Interface for sending messages through the gossip layer
    pub_sub: P,
}

#[derive(From)]
enum ConsensusService<P: PubSub<PubSubMsg> + 'static> {
    /// A node that is on the narwhal committee
    Narwhal(NarwhalService),
    /// A node that is not running narwhal because its not on the committee
    EdgeNode(EdgeService<P>),
}

impl<P: PubSub<PubSubMsg>> ConsensusService<P> {
    async fn shutdown(&mut self) {
        match self {
            ConsensusService::Narwhal(narwhal) => narwhal.shutdown().await,
            ConsensusService::EdgeNode(edge) => edge.shutdown().await,
        }
    }
}

impl<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg> + 'static> EpochState<Q, P> {
    fn new(
        query_runner: Q,
        narwhal_args: NarwhalArgs,
        store_path: PathBuf,
        execution_state: Arc<Execution<P>>,
        txn_socket: SubmitTxSocket,
        pub_sub: P,
    ) -> Self {
        Self {
            consensus: None,
            query_runner,
            narwhal_args,
            store_path,
            execution_state,
            txn_socket,
            pub_sub,
        }
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
        } else {
            self.run_edge_node(store, committee, worker_cache).await
        }
    }

    async fn move_to_next_epoch(&mut self) {
        if let Some(mut state) = self.consensus.take() {
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
            if let (Ok(address), Ok(protocol_key), Ok(network_key)) = (
                Multiaddr::try_from(node.domain.to_string()),
                PublicKey::from_bytes(&node.public_key.0),
                NetworkPublicKey::from_bytes(&node.network_key.0),
            ) {
                committee_builder =
                    committee_builder.add_authority(protocol_key, 1, address, network_key);
            }
        }
        let narwhal_committee = committee_builder.build();

        let worker_cache = WorkerCache {
            epoch,
            workers: committee
                .iter()
                .filter_map(|node| {
                    if let Ok(public_key) = PublicKey::from_bytes(&node.public_key.0) {
                        let mut worker_index = BTreeMap::new();
                        node.workers
                            .iter()
                            .filter_map(|worker| {
                                if let (Ok(name), Ok(address), Ok(mempool)) = (
                                    NetworkPublicKey::from_bytes(&worker.public_key.0),
                                    Multiaddr::try_from(worker.address.to_string()),
                                    Multiaddr::try_from(worker.mempool.to_string()),
                                ) {
                                    Some(WorkerInfo {
                                        name,
                                        transactions: mempool,
                                        worker_address: address,
                                    })
                                } else {
                                    None
                                }
                            })
                            .enumerate()
                            .for_each(|(index, worker)| {
                                worker_index.insert(index as u32, worker);
                            });
                        Some((public_key, WorkerIndex(worker_index)))
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

    async fn run_edge_node(
        &mut self,
        store: NodeStorage,
        committee: Committee,
        worker_cache: WorkerCache,
    ) {
        info!("Not on narwhal committee running edge node service");

        // Let the execution state know you are not on committee
        self.execution_state.set_committee_status(false);

        let mut edge_service =
            EdgeService::new(store, committee, worker_cache, self.pub_sub.clone());

        edge_service.start(self.execution_state.clone()).await;

        self.consensus = Some(edge_service.into());
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
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let until_epoch_ends: u64 = (epoch_end as u128).saturating_sub(now).try_into().unwrap();
        let time_until_epoch_change = Duration::from_millis(until_epoch_ends);

        self.wait_to_signal_epoch_change(time_until_epoch_change, epoch)
            .await;

        // Create the narwhal service
        let service =
            NarwhalService::new(self.narwhal_args.clone(), store, committee, worker_cache);

        service.start(self.execution_state.clone()).await;

        self.consensus = Some(service.into())
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg>> WithStartAndShutdown for Consensus<Q, P> {
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
            epoch_state.start_current_epoch().await;
            loop {
                let reconfigure_future = reconfigure_notify.notified();
                let shutdown_future = shutdown_notify.notified();
                pin!(shutdown_future);
                pin!(reconfigure_future);

                select! {
                    _ = shutdown_future => {
                        if let Some(mut consensus) = epoch_state.consensus.take() {
                            consensus.shutdown().await;
                        }
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

impl<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg>> ConfigConsumer for Consensus<Q, P> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, P: PubSub<PubSubMsg>> ConsensusInterface for Consensus<Q, P> {
    type QueryRunner = Q;
    type Certificate = PubSubMsg;
    type PubSub = P;

    /// Create a new consensus service with the provided config and executor.
    async fn init<S: SignerInterface>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: Q,
        pubsub: P,
    ) -> anyhow::Result<Self> {
        let (networking_sk, primary_sk) = signer.get_sk();
        let reconfigure_notify = Arc::new(Notify::new());
        let new_block_notify = Arc::new(Notify::new());
        let networking_keypair = NetworkKeyPair::from(networking_sk);
        let primary_keypair = KeyPair::from(primary_sk);
        let forwarder = Forwarder::new(query_runner.clone(), primary_keypair.public().clone());
        let narwhal_args = NarwhalArgs {
            primary_keypair,
            primary_network_keypair: networking_keypair.copy(),
            worker_keypair: networking_keypair,
            primary_address: config.address,
            worker_address: config.worker_address,
            worker_mempool: config.mempool_address,
            registry_service: RegistryService::new(Registry::new()),
        };
        let execution_state = Arc::new(Execution::new(
            executor,
            reconfigure_notify.clone(),
            new_block_notify.clone(),
            pubsub.clone(),
        ));
        let epoch_state = EpochState::new(
            query_runner,
            narwhal_args,
            config.store_path,
            execution_state,
            signer.get_socket(),
            pubsub,
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
    Certificate(narwhal_types::Certificate),
    Batch(narwhal_types::Batch),
}
impl AutoImplSerde for PubSubMsg {}
