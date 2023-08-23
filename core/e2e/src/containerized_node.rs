use std::sync::Mutex;

use fleek_crypto::AccountOwnerSecretKey;
use lightning_interfaces::ConfigProviderInterface;
use lightning_node::config::TomlConfigProvider;
use lightning_node::node::FinalTypes;
use lightning_rpc::server::Rpc;

use crate::container::Container;

pub struct ContainerizedNode {
    config: TomlConfigProvider<FinalTypes>,
    owner_secret_key: AccountOwnerSecretKey,
    container: Mutex<Option<Container<FinalTypes>>>,
    runtime_type: RuntimeType,
    index: usize,
}

impl ContainerizedNode {
    pub fn new(
        config: TomlConfigProvider<FinalTypes>,
        owner_secret_key: AccountOwnerSecretKey,
        index: usize,
    ) -> Self {
        Self {
            config,
            owner_secret_key,
            container: Default::default(),
            runtime_type: RuntimeType::MultiThreaded,
            index,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        // This function has to return a result in order to use try_join_all in swarm.rs
        *self.container.lock().unwrap() =
            Some(Container::spawn(self.index, self.config.clone(), self.runtime_type).await);
        Ok(())
    }

    pub fn shutdown(&self) {
        *self.container.lock().unwrap() = None;
    }

    pub fn is_running(&self) -> bool {
        self.container.lock().unwrap().is_some()
    }

    pub fn get_rpc_address(&self) -> String {
        let config = self.config.get::<Rpc<FinalTypes>>();
        format!("http://{}:{}/rpc/v0", config.addr, config.port)
    }

    pub fn get_owner_secret_key(&self) -> AccountOwnerSecretKey {
        self.owner_secret_key.clone()
    }

    pub fn get_index(&self) -> usize {
        self.index
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RuntimeType {
    SingleThreaded,
    MultiThreaded,
}
