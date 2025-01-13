use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use fleek_crypto::NodePublicKey;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Blake3Hash,
    Epoch,
    EpochInfo,
    NodeIndex,
    NodeInfo,
    Participation,
    ServerRequest,
};
use lightning_metrics::increment_counter;
use lightning_utils::application::QueryRunnerExt;
use rand::seq::SliceRandom;
use tracing::error;

use crate::config::Config;
use crate::{rpc, utils};

pub struct Syncronizer<C: NodeComponents> {
    state: State<C>,
}

enum State<C: NodeComponents> {
    Initialized(SyncronizerInner<C>),
    Running(async_channel::Receiver<Blake3Hash>),
}

struct SyncronizerInner<C: NodeComponents> {
    our_public_key: NodePublicKey,
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    notifier: C::NotifierInterface,
    blockstore_server_socket: BlockstoreServerSocket,
    genesis_committee: Vec<(NodeIndex, NodeInfo)>,
    epoch_change_delta: Duration,
}

impl<C: NodeComponents> Syncronizer<C> {
    /// Create a syncronizer service for quickly syncronizing the node state with the chain
    fn init(
        config: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        blockstore_server: &C::BlockstoreServerInterface,
        notifier: &C::NotifierInterface,
        query_runner: fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Result<Self> {
        let config = config.get::<Self>();

        let mut genesis_committee = query_runner.get_genesis_committee();
        // Shuffle this since we often hit this list in order until one responds. This will give our
        // network a bit of diversity on which bootstrap node they try first
        genesis_committee.shuffle(&mut rand::thread_rng());

        let our_public_key = keystore.get_ed25519_pk();

        // We only run the prelude in prod mode to avoid interfering with tests, and if genesis
        // has already been applied, otherwise we skip it.
        if !cfg!(debug_assertions) && query_runner.has_genesis() {
            Syncronizer::<C>::prelude(our_public_key, &genesis_committee);
        }

        let inner = SyncronizerInner::new(
            our_public_key,
            genesis_committee,
            query_runner.clone(),
            notifier.clone(),
            blockstore_server,
            config.epoch_change_delta,
        )?;

        Ok(Self {
            state: State::Initialized(inner),
        })
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    fn start(this: &mut Self, fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>) {
        let (tx, rx) = async_channel::bounded(1);
        let state = std::mem::replace(&mut this.state, State::Running(rx));
        let State::Initialized(mut inner) = state else {
            panic!("Syncronizer can only be started once");
        };
        let panic_waiter = shutdown.clone();
        spawn!(
            async move { shutdown.run_until_shutdown(inner.run(tx)).await },
            "SYNCRONIZER",
            crucial(panic_waiter)
        );
    }

    fn prelude(our_public_key: NodePublicKey, genesis_committee: &[(NodeIndex, NodeInfo)]) {
        // Skip prelude if node is on the genesis committee.
        if genesis_committee
            .iter()
            .any(|(_, info)| info.public_key == our_public_key)
        {
            return;
        }

        // The rpc calls are using a lot of clones. This is necessary because the futures are
        // passed into a thread, and thus need to satisfy the 'static lifetime.

        // Check if node has sufficient stake.
        let is_valid = rpc::sync_call(rpc::check_if_node_has_sufficient_stake(
            our_public_key,
            genesis_committee.to_vec(),
        ))
        .expect("Cannot reach bootstrap nodes");
        if !is_valid {
            // TODO(matthias): print this message in a loop?
            println!(
                "The node is not staked. Only staked nodes can participate in the network. Submit a stake transaction and try again."
            );
            std::process::exit(0);
        }
        let node_info = rpc::sync_call(rpc::get_node_info(
            our_public_key,
            genesis_committee.to_vec(),
        ))
        .expect("Cannot reach bootstrap nodes")
        .unwrap(); // we unwrap here because we already checked if the node is valid above

        let epoch_info = rpc::sync_call(rpc::get_epoch_info(genesis_committee.to_vec()))
            .expect("Cannot reach bootstrap nodes");

        // Check participation status.
        match node_info.participation {
            Participation::False => {
                // TODO(matthias): print this message in a loop?
                println!(
                    "The node is currently not participating in the network. Submit an OptIn transaction using the CLI to participate and try again."
                );
                std::process::exit(0);
            },
            Participation::OptedIn => {
                rpc::sync_call(utils::wait_to_next_epoch(
                    epoch_info,
                    genesis_committee.to_vec(),
                ));
            },
            _ => (),
        }
    }
}

impl<C: NodeComponents> fdi::BuildGraph for Syncronizer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(Self::init.with_event_handler("start", Self::start))
    }
}

impl<C: NodeComponents> SyncronizerInterface<C> for Syncronizer<C> {
    /// Returns a socket that will send accross the blake3hash of the checkpoint
    /// Will send it after it has already downloaded from the blockstore server
    async fn next_checkpoint_hash(&self) -> Option<Blake3Hash> {
        let State::Running(rx) = &self.state else {
            panic!("syncronizer must be started");
        };
        rx.recv().await.ok()
    }
}

impl<C: NodeComponents> SyncronizerInner<C> {
    fn new(
        our_public_key: NodePublicKey,
        genesis_committee: Vec<(NodeIndex, NodeInfo)>,
        query_runner: c![C::ApplicationInterface::SyncExecutor],
        notifier: C::NotifierInterface,
        blockstore_server: &C::BlockstoreServerInterface,
        epoch_change_delta: Duration,
    ) -> Result<Self> {
        Ok(Self {
            our_public_key,
            query_runner,
            blockstore_server_socket: blockstore_server.get_socket(),
            notifier,
            genesis_committee,
            epoch_change_delta,
        })
    }

    async fn run(&mut self, tx_update_ready: async_channel::Sender<Blake3Hash>) {
        // When we first start we want to check if we should checkpoint
        if let Ok(checkpoint_hash) = self.try_sync().await {
            // Our blockstore succesfully downloaded the checkpoint lets send up the hash and return
            let _ = tx_update_ready.send(checkpoint_hash).await;
            return;
        }

        let mut epoch_changed_sub = self.notifier.subscribe_epoch_changed();

        loop {
            let EpochInfo { epoch_end, .. } = self.query_runner.get_epoch_info();

            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let until_epoch_ends: u64 = (epoch_end as u128).saturating_sub(now).try_into().unwrap();
            let time_until_epoch_change = Duration::from_millis(until_epoch_ends);

            let time_to_check =
                tokio::time::sleep(time_until_epoch_change + self.epoch_change_delta);

            tokio::select! {
                _ = time_to_check => {
                    if let Ok(checkpoint_hash) = self.try_sync().await{
                        // Our blockstore succesfully downloaded the checkpoint lets send up the hash and return

                        increment_counter!(
                            "epoch_change_by_checkpoint",
                            Some("Counter for the number of times the node restarted from a new checkpoint instead of changing epochs naturally")
                        );

                        let _ = tx_update_ready.send(checkpoint_hash).await;
                        return;
                    }
                }

                Some(_notification) = epoch_changed_sub.recv() => {
                    // We only run the prelude in prod mode to avoid interfering with tests, and if genesis
                    // has already been applied, otherwise we skip it.
                    // At this point the genesis should always already have been applied, but we keep the check anyway.
                    if !cfg!(debug_assertions) && self.query_runner.has_genesis() {
                        if !self.query_runner.has_sufficient_stake(&self.our_public_key) {
                            println!("The node doesn't have enough stake to participate in the network.");
                            std::process::exit(1);
                        }
                        let node_idx = self
                            .query_runner
                            .pubkey_to_index(&self.our_public_key)
                            .unwrap();

                        let node_info = self
                            .query_runner
                            .get_node_info::<NodeInfo>(&node_idx, |n| n)
                            .unwrap();

                        // Exit if a non-genesis node is not participating
                        if node_info.participation == Participation::False
                            && !self.genesis_committee
                                .iter()
                                .any(|(_, info)| info.public_key == self.our_public_key)
                        {
                            println!("The node is currently not participating in the network. You either submitted a OptOut transaction, or the node did not respond to enough pings.");
                            std::process::exit(1);
                        }
                    }
                }

                else => {
                    // We must be shutting down
                    return;
                }
            }
        }
    }

    async fn try_sync(&self) -> Result<[u8; 32]> {
        // Get the epoch this edge node is on
        let current_epoch = self.query_runner.get_current_epoch();

        // Get the epoch the bootstrap nodes are at
        let bootstrap_epoch = self.get_current_epoch().await?;

        if bootstrap_epoch <= current_epoch {
            bail!("Bootstrap nodes are on the same epoch");
        }

        // Try to get the latest checkpoint hash
        let latest_checkpoint_hash = self.get_latest_checkpoint_hash().await?;

        // Attempt to download to our blockstore the latest checkpoint and if that is succesfully
        // alert the node that it is ready to load the checkpoint
        if self
            .download_checkpoint_from_bootstrap(latest_checkpoint_hash)
            .await
            .is_ok()
        {
            Ok(latest_checkpoint_hash)
        } else {
            error!("Unable to download checkpoint");
            Err(anyhow!("Unable to download checkpoint"))
        }
    }

    async fn download_checkpoint_from_bootstrap(&self, checkpoint_hash: [u8; 32]) -> Result<()> {
        for (node_index, _) in &self.genesis_committee {
            let mut res = self
                .blockstore_server_socket
                .run(ServerRequest {
                    hash: checkpoint_hash,
                    peer: *node_index,
                })
                .await
                .expect("Failed to send blockstore server request");

            if let Ok(Ok(_)) = res.recv().await {
                // TODO: Ask here about checkpoints
                return Ok(());
            }
        }
        Err(anyhow!(
            "Unable to download checkpoint from any bootstrap nodes"
        ))
    }

    // This function will hit the bootstrap nodes(Genesis committee) to ask what epoch they are on
    // who the current committee is
    async fn get_latest_checkpoint_hash(&self) -> Result<[u8; 32]> {
        rpc::last_epoch_hash(&self.genesis_committee).await
    }

    /// Returns the epoch the bootstrap nodes are on
    async fn get_current_epoch(&self) -> Result<Epoch> {
        let epochs = rpc::get_epoch(self.genesis_committee.clone()).await?;
        let epoch = epochs
            .into_iter()
            .max()
            .context("Failed to get epoch from bootstrap nodes")?;
        Ok(epoch)
    }
}

impl<C: NodeComponents> ConfigConsumer for Syncronizer<C> {
    const KEY: &'static str = "syncronizer";

    type Config = Config;
}
