use std::sync::Mutex;

use fleek_crypto::AccountOwnerSecretKey;
use lightning_blockstore::blockstore::Blockstore;
use lightning_interfaces::types::{Blake3Hash, Staking};
use lightning_interfaces::ConfigProviderInterface;
use lightning_node::config::TomlConfigProvider;
use lightning_node::FinalTypes;
use lightning_rpc::Rpc;
use tokio::sync::oneshot;

use crate::container::Container;

pub struct ContainerizedNode {
    config: TomlConfigProvider<FinalTypes>,
    owner_secret_key: AccountOwnerSecretKey,
    container: Mutex<Option<Container<FinalTypes>>>,
    runtime_type: RuntimeType,
    index: usize,
    genesis_stake: Staking,
    is_genesis_committee: bool,
}

impl ContainerizedNode {
    pub fn new(
        config: TomlConfigProvider<FinalTypes>,
        owner_secret_key: AccountOwnerSecretKey,
        index: usize,
        is_genesis_committee: bool,
        genesis_stake: Staking,
    ) -> Self {
        Self {
            config,
            owner_secret_key,
            container: Default::default(),
            runtime_type: RuntimeType::MultiThreaded,
            index,
            genesis_stake,
            is_genesis_committee,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        // This function has to return a result in order to use try_join_all in swarm.rs
        *self.container.lock().unwrap() =
            Some(Container::spawn(self.index, self.config.clone(), self.runtime_type).await);
        Ok(())
    }

    pub fn shutdown(&self) {
        if let Some(container) = &mut self.container.lock().unwrap().take() {
            container.shutdown();
        }
    }

    pub fn is_running(&self) -> bool {
        self.container.lock().unwrap().is_some()
    }

    pub fn get_rpc_address(&self) -> String {
        let config = self.config.get::<Rpc<FinalTypes>>();
        format!("http://{}", config.addr())
    }

    pub fn get_owner_secret_key(&self) -> AccountOwnerSecretKey {
        self.owner_secret_key.clone()
    }

    pub fn get_index(&self) -> usize {
        self.index
    }

    pub fn get_genesis_stake(&self) -> Staking {
        self.genesis_stake.clone()
    }

    pub fn take_ckpt_rx(&self) -> Option<oneshot::Receiver<Blake3Hash>> {
        let container = self.container.lock().unwrap().take();
        if let Some(mut container) = container {
            let ckpt_rx = container.take_ckpt_rx();
            *self.container.lock().unwrap() = Some(container);
            ckpt_rx
        } else {
            *self.container.lock().unwrap() = None;
            None
        }
    }

    pub fn take_blockstore(&self) -> Option<Blockstore<FinalTypes>> {
        let container = self.container.lock().unwrap().take();
        if let Some(mut container) = container {
            let blockstore = container.take_blockstore();
            *self.container.lock().unwrap() = Some(container);
            blockstore
        } else {
            *self.container.lock().unwrap() = None;
            None
        }
    }

    pub fn is_genesis_committee(&self) -> bool {
        self.is_genesis_committee
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RuntimeType {
    SingleThreaded,
    MultiThreaded,
}
