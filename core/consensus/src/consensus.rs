use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use affair::{Executor, TokioSpawn};
use async_trait::async_trait;
use draco_interfaces::{
    application::ExecutionEngineSocket,
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    consensus::{ConsensusInterface, MempoolSocket},
    signer::{SignerInterface, SubmitTxSocket},
    types::{Epoch, EpochInfo, UpdateMethod},
    GossipInterface, SyncQueryRunnerInterface,
};
use mysten_metrics::RegistryService;
use mysten_network::Multiaddr;
use narwhal_config::{Committee, CommitteeBuilder, WorkerCache, WorkerIndex, WorkerInfo};
use narwhal_crypto::{
    traits::{KeyPair as _, ToFromBytes},
    KeyPair, NetworkKeyPair, NetworkPublicKey, PublicKey,
};
use narwhal_node::NodeStorage;
use prometheus::Registry;
use tokio::{
    pin, select,
    sync::{Mutex, Notify},
    task, time,
};

use crate::{
    config::Config,
    execution::Execution,
    forwarder::Forwarder,
    narwhal::{NarwhalArgs, NarwhalService},
};

pub struct Consensus<Q: SyncQueryRunnerInterface, G: GossipInterface> {
    /// Used to query the application data
    pub query_runner: Q,
    /// Interface for sending messages through the gossip layer
    _gossip: Arc<G>,
    /// Path to the database used by the narwhal implementation
    pub store_path: PathBuf,
    /// This narwhal node data
    narwhal_args: NarwhalArgs,
    /// The state of the current Narwhal epoch.
    epoch_state: Mutex<Option<EpochState>>,
    /// Used to send transactions to consensus
    /// We still use this socket on consensus struct because a node is not always on the committee,
    /// so its not always sending     a transaction to its own mempool. The signer interface
    /// also takes care of nonce bookkeeping and retry logic
    txn_socket: SubmitTxSocket,
    /// This socket recieves signed transactions and forwards them to an active committee member to
    /// be ordered
    mempool_socket: MempoolSocket,
    /// Narwhal execution state.
    execution_state: Arc<Execution>,
    /// Timestamp of the narwhal certificate that caused an epoch change
    /// is sent through this channel to notify that epoch chould change.
    reconfigure_notify: Arc<Notify>,
    /// A notifier that is notified everytime a new block is proccessed
    new_block_notify: Arc<Notify>,
    /// Called from the shutdown function to notify the start event loop to
    /// exit.
    shutdown_notify: Notify,
}

/// This struct contains mutable state only for the current epoch.
struct EpochState {
    /// The Narwhal service for the current epoch.
    narwhal: NarwhalService,
}

impl<Q: SyncQueryRunnerInterface, G: GossipInterface> Consensus<Q, G> {
    async fn start_current_epoch(&self) {
        // Get current epoch information
        let (committee, worker_cache, epoch, epoch_end) = self.get_epoch_info();

        if committee
            .authority_by_key(self.narwhal_args.primary_keypair.public())
            .is_none()
        {
            self.run_edge_node();
            return;
        }
        // Make or open store specific to current epoch
        let mut store_path = self.store_path.clone();
        store_path.push(format!("{epoch}"));
        // TODO(dalton): This store takes an optional cache metrics struct that can give us metrics
        // on hits/miss
        let store = NodeStorage::reopen(store_path, None);

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

        *self.epoch_state.lock().await = Some(EpochState { narwhal: service })
    }

    async fn move_to_next_epoch(&self) {
        {
            let epoch_state_mut = self.epoch_state.lock().await.take();
            if let Some(state) = epoch_state_mut {
                state.narwhal.shutdown().await
            }
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
            // We shouldnt panic here lets repeatedly try.
            while txn_socket
                .run(UpdateMethod::ChangeEpoch { epoch })
                .await
                .is_err()
            {
                // If for some reason there is an error sending to the signer interface try again
                println!(
                    "Error sending change epoch transaction to signer interface, trying again..."
                );
                time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    fn run_edge_node(&self) {
        // todo
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, G: GossipInterface> WithStartAndShutdown for Consensus<Q, G> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        self.start_current_epoch().await;
        // todo(dalton): Use affair to await:
        //      A:) shutdown_future
        //      B:) Reconfigure_notifier
        loop {
            let reconfigure_future = self.reconfigure_notify.notified();
            let shutdown_future = self.shutdown_notify.notified();
            pin!(shutdown_future);
            pin!(reconfigure_future);

            select! {
                _ = shutdown_future => {
                    break
                }
                _ = reconfigure_future => {
                    self.move_to_next_epoch().await;
                    continue
                }
            }
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }
}

impl<Q: SyncQueryRunnerInterface, G: GossipInterface> ConfigConsumer for Consensus<Q, G> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, G: GossipInterface> ConsensusInterface for Consensus<Q, G> {
    type QueryRunner = Q;
    type Gossip = G;
    /// Create a new consensus service with the provided config and executor.
    async fn init<S: SignerInterface>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: Q,
        gossip: Arc<Self::Gossip>,
    ) -> anyhow::Result<Self> {
        let (networking_sk, primary_sk) = signer.get_sk();
        let reconfigure_notify = Arc::new(Notify::new());
        let new_block_notify = Arc::new(Notify::new());
        let networking_keypair = NetworkKeyPair::from(networking_sk);
        let primary_keypair = KeyPair::from(primary_sk);
        let forwarder = Forwarder::new(query_runner.clone(), primary_keypair.public().clone());

        Ok(Self {
            query_runner,
            _gossip: gossip,
            store_path: config.store_path,
            narwhal_args: NarwhalArgs {
                primary_keypair,
                primary_network_keypair: networking_keypair.copy(),
                worker_keypair: networking_keypair,
                primary_address: config.address,
                worker_address: config.worker_address,
                worker_mempool: config.mempool_address,
                registry_service: RegistryService::new(Registry::new()),
            },
            epoch_state: Mutex::new(None),
            execution_state: Arc::new(Execution::new(
                executor,
                reconfigure_notify.clone(),
                new_block_notify.clone(),
            )),
            txn_socket: signer.get_socket(),
            mempool_socket: TokioSpawn::spawn_async(forwarder),
            reconfigure_notify,
            new_block_notify,
            shutdown_notify: Notify::new(),
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
