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
use serde::de::DeserializeOwned;

use crate::config::Config;
use crate::rpc::{self, rpc_epoch};
use crate::utils;

pub struct Syncronizer<C: Collection> {
    state: State<C>,
}

enum State<C: Collection> {
    Initialized(SyncronizerInner<C>),
    Running(async_channel::Receiver<Blake3Hash>),
}

struct SyncronizerInner<C: Collection> {
    our_public_key: NodePublicKey,
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    notifier: C::NotifierInterface,
    blockstore_server_socket: BlockstoreServerSocket,
    genesis_committee: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
    epoch_change_delta: Duration,
}

impl<C: Collection> Syncronizer<C> {
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

        let rpc_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(5))
            .build()?;

        if !cfg!(debug_assertions) {
            // We only run the prelude in prod mode to avoid interfering with tests.
            Syncronizer::<C>::prelude(our_public_key, &genesis_committee, &rpc_client);
        }

        let inner = SyncronizerInner::new(
            our_public_key,
            genesis_committee,
            query_runner.clone(),
            notifier.clone(),
            blockstore_server,
            config.epoch_change_delta,
            rpc_client,
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
        tokio::spawn(async move { shutdown.run_until_shutdown(inner.run(tx)).await });
    }

    fn prelude(
        our_public_key: NodePublicKey,
        genesis_committee: &Vec<(NodeIndex, NodeInfo)>,
        rpc_client: &reqwest::Client,
    ) {
        // Check if node is on genesis committee.
        for (_, node_info) in genesis_committee {
            if our_public_key == node_info.public_key {
                // If the node is a member of the genesis committee, we skip the prelude.
                return;
            }
        }

        // The rpc calls are using a lot of clones. This is necessary because the futures are
        // passed into a thread, and thus need to satisfy the 'static lifetime.

        // Check if node is staked.
        let is_valid = rpc::sync_call(rpc::check_is_valid_node(
            our_public_key,
            genesis_committee.clone(),
            rpc_client.clone(),
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
            genesis_committee.clone(),
            rpc_client.clone(),
        ))
        .expect("Cannot reach bootstrap nodes")
        .unwrap(); // we unwrap here because we already checked if the node is valid above

        let epoch_info = rpc::sync_call(rpc::get_epoch_info(
            genesis_committee.clone(),
            rpc_client.clone(),
        ))
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
                    genesis_committee.clone(),
                    rpc_client.clone(),
                ));
            },
            _ => (),
        }
    }
}

impl<C: Collection> fdi::BuildGraph for Syncronizer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(Self::init.on("start", Self::start))
    }
}

impl<C: Collection> SyncronizerInterface<C> for Syncronizer<C> {
    /// Returns a socket that will send accross the blake3hash of the checkpoint
    /// Will send it after it has already downloaded from the blockstore server
    async fn next_checkpoint_hash(&self) -> Option<Blake3Hash> {
        let State::Running(rx) = &self.state else {
            panic!("syncronizer must be started");
        };
        rx.recv().await.ok()
    }
}

impl<C: Collection> SyncronizerInner<C> {
    fn new(
        our_public_key: NodePublicKey,
        genesis_committee: Vec<(NodeIndex, NodeInfo)>,
        query_runner: c![C::ApplicationInterface::SyncExecutor],
        notifier: C::NotifierInterface,
        blockstore_server: &C::BlockstoreServerInterface,
        epoch_change_delta: Duration,
        rpc_client: reqwest::Client,
    ) -> Result<Self> {
        Ok(Self {
            our_public_key,
            query_runner,
            blockstore_server_socket: blockstore_server.get_socket(),
            notifier,
            genesis_committee,
            rpc_client,
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
                    if !cfg!(debug_assertions) {
                        // We only run the prelude in prod mode to avoid interfering with tests.
                        if !self.query_runner.is_valid_node(&self.our_public_key) {
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

                        if node_info.participation == Participation::False {
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

        //let bootstrap_epoch = self.ask_bootstrap_nodes(rpc_epoch().to_string()).await?;

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
            Err(anyhow!("Unable to download checkpoint"))
        }
    }

    /// This function will rpc request genesis nodes in sequence and stop when one of them responds
    async fn ask_bootstrap_nodes<T: DeserializeOwned>(&self, req: String) -> Result<Vec<T>> {
        rpc::ask_nodes(req, &self.genesis_committee, &self.rpc_client).await
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

            if let Ok(Ok(response)) = res.recv().await {
                return Ok(response);
            }
        }
        Err(anyhow!(
            "Unable to download checkpoint from any bootstrap nodes"
        ))
    }

    // This function will hit the bootstrap nodes(Genesis committee) to ask what epoch they are on
    // who the current committee is
    async fn get_latest_checkpoint_hash(&self) -> Result<[u8; 32]> {
        rpc::last_epoch_hash(&self.genesis_committee, &self.rpc_client).await
    }

    /// Returns the epoch the bootstrap nodes are on
    async fn get_current_epoch(&self) -> Result<Epoch> {
        let epochs = self.ask_bootstrap_nodes(rpc_epoch().to_string()).await?;
        let epoch = epochs
            .into_iter()
            .max()
            .context("Failed to get epoch from bootstrap nodes")?;
        Ok(epoch)
    }
}

impl<C: Collection> ConfigConsumer for Syncronizer<C> {
    const KEY: &'static str = "syncronizer";

    type Config = Config;
}
