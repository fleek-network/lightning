use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Result};
use fleek_crypto::NodePublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{
    Blake3Hash,
    Epoch,
    EpochInfo,
    NodeIndex,
    NodeInfo,
    Participation,
    ServerRequest,
};
use lightning_interfaces::{
    ApplicationInterface,
    BlockStoreServerInterface,
    BlockStoreServerSocket,
    ConfigConsumer,
    Notification,
    SignerInterface,
    SyncQueryRunnerInterface,
    SyncronizerInterface,
    WithStartAndShutdown,
};
use lightning_utils::application::QueryRunnerExt;
use rand::seq::SliceRandom;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::info;

use crate::config::Config;
use crate::rpc::{self, rpc_epoch, rpc_last_epoch_hash};
use crate::utils;

pub struct Syncronizer<C: Collection> {
    inner: Mutex<Option<SyncronizerInner<C>>>,
    rx_checkpoint_ready: Mutex<Option<oneshot::Receiver<Blake3Hash>>>,
    handle: Mutex<Option<JoinHandle<SyncronizerInner<C>>>>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

pub struct SyncronizerInner<C: Collection> {
    our_public_key: NodePublicKey,
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    blockstore_server_socket: BlockStoreServerSocket,
    rx_epoch_change: Receiver<Notification>,
    genesis_committee: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
    epoch_change_delta: Duration,
}

impl<C: Collection> WithStartAndShutdown for Syncronizer<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        self.handle.lock().unwrap().is_some()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if self.is_running() {
            info!("Syncronizer is not going to start because its already started");
            return;
        }

        let (tx_checkpoint_ready, rx_checkpoint_ready) = oneshot::channel();
        // We create a new oneshot channel everytime we start.
        let (tx_shutdown, rx_shutdown) = oneshot::channel();
        *self.rx_checkpoint_ready.lock().unwrap() = Some(rx_checkpoint_ready);
        *self.shutdown.lock().unwrap() = Some(tx_shutdown);

        let mut inner = self.inner.lock().unwrap().take().unwrap();

        let handle = tokio::task::spawn(async move {
            inner.run(tx_checkpoint_ready, rx_shutdown).await;

            inner
        });

        *self.handle.lock().unwrap() = Some(handle);
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        let handle = self.handle.lock().unwrap().take();
        let shutdown = self.shutdown.lock().unwrap().take();
        if let (Some(handle), Some(shutdown)) = (handle, shutdown) {
            let _ = shutdown.send(());

            *self.inner.lock().unwrap() = Some(handle.await.unwrap());
        }
    }
}

impl<C: Collection> SyncronizerInterface<C> for Syncronizer<C> {
    /// Create a syncronizer service for quickly syncronizing the node state with the chain
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore_server: &C::BlockStoreServerInterface,
        signer: &C::SignerInterface,
        rx_epoch_change: Receiver<Notification>,
    ) -> Result<Self> {
        let mut genesis_committee = query_runner.get_genesis_committee();
        // Shuffle this since we often hit this list in order until one responds. This will give our
        // network a bit of diversity on which bootstrap node they try first
        genesis_committee.shuffle(&mut rand::thread_rng());

        let our_public_key = signer.get_ed25519_pk();

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
            query_runner,
            blockstore_server,
            rx_epoch_change,
            config.epoch_change_delta,
            rpc_client,
        )?;

        Ok(Self {
            inner: Mutex::new(Some(inner)),
            rx_checkpoint_ready: Mutex::new(None),
            handle: Mutex::new(None),
            shutdown: Mutex::new(None),
        })
    }

    /// Returns a socket that will send accross the blake3hash of the checkpoint
    /// Will send it after it has already downloaded from the blockstore server
    fn checkpoint_socket(&self) -> oneshot::Receiver<Blake3Hash> {
        self.rx_checkpoint_ready.lock().unwrap().take().unwrap()
    }
}

impl<C: Collection> Syncronizer<C> {
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
        if !rpc::sync_call(rpc::check_is_valid_node(
            our_public_key,
            genesis_committee.clone(),
            rpc_client.clone(),
        ))
        .expect("Cannot reach bootstrap nodes")
        {
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
        .unwrap(); // safe unwrap because we check if the node is valid above

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
                println!(" after...");
            },
            _ => (),
        }
    }
}

impl<C: Collection> SyncronizerInner<C> {
    fn new(
        our_public_key: NodePublicKey,
        genesis_committee: Vec<(NodeIndex, NodeInfo)>,
        query_runner: c![C::ApplicationInterface::SyncExecutor],
        blockstore_server: &C::BlockStoreServerInterface,
        rx_epoch_change: Receiver<Notification>,
        epoch_change_delta: Duration,
        rpc_client: reqwest::Client,
    ) -> Result<Self> {
        Ok(Self {
            our_public_key,
            query_runner,
            blockstore_server_socket: blockstore_server.get_socket(),
            rx_epoch_change,
            genesis_committee,
            rpc_client,
            epoch_change_delta,
        })
    }

    async fn run(
        &mut self,
        tx_update_ready: oneshot::Sender<Blake3Hash>,
        mut shutdown: oneshot::Receiver<()>,
    ) {
        // When we first start we want to check if we should checkpoint
        if let Ok(checkpoint_hash) = self.try_sync().await {
            // Our blockstore succesfully downloaded the checkpoint lets send up the hash and return
            let _ = tx_update_ready.send(checkpoint_hash);
            return;
        }
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

            let epoch_change_future = self.rx_epoch_change.recv();

            tokio::select! {
                _ = time_to_check => {
                    if let Ok(checkpoint_hash) = self.try_sync().await{
                        // Our blockstore succesfully downloaded the checkpoint lets send up the hash and return
                        let _ = tx_update_ready.send(checkpoint_hash);
                        return;
                    }
                }

                notification = epoch_change_future => {
                    if notification.is_none() {
                        // We must be shutting down
                        return;
                    }
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

                _ = &mut shutdown => return
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
    async fn ask_bootstrap_nodes<T: DeserializeOwned>(&self, req: String) -> Result<T> {
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
        self.ask_bootstrap_nodes(rpc_last_epoch_hash().to_string())
            .await
    }

    /// Returns the epoch the bootstrap nodes are on
    async fn get_current_epoch(&self) -> Result<Epoch> {
        self.ask_bootstrap_nodes(rpc_epoch().to_string()).await
    }
}

impl<C: Collection> ConfigConsumer for Syncronizer<C> {
    const KEY: &'static str = "syncronizer";

    type Config = Config;
}
