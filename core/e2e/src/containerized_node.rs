use fleek_crypto::AccountOwnerSecretKey;
use futures::Future;
use lightning_blockstore::blockstore::Blockstore;
use lightning_final_bindings::FinalTypes;
use lightning_interfaces::fdi::MultiThreadedProvider;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::Staking;
use lightning_node::ContainedNode;
use lightning_rpc::Rpc;
use lightning_utils::config::TomlConfigProvider;

pub struct ContainerizedNode {
    config: TomlConfigProvider<FinalTypes>,
    owner_secret_key: AccountOwnerSecretKey,
    node: ContainedNode<FinalTypes>,
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
        let provider = MultiThreadedProvider::default();
        provider.insert(config.clone());
        let node = ContainedNode::<FinalTypes>::new(provider, None);
        Self {
            config,
            owner_secret_key,
            node,
            index,
            genesis_stake,
            is_genesis_committee,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        // This function has to return a result in order to use try_join_all in swarm.rs
        let handle = self.node.spawn();
        handle.await.unwrap()?;

        Ok(())
    }

    pub fn shutdown(self) -> impl Future<Output = ()> {
        self.node.shutdown()
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

    pub fn take_syncronizer(&self) -> fdi::Ref<c!(FinalTypes::SyncronizerInterface)> {
        self.node
            .provider()
            .get::<<FinalTypes as Collection>::SyncronizerInterface>()
    }

    pub fn take_blockstore(&self) -> Blockstore<FinalTypes> {
        self.node
            .provider()
            .get::<<FinalTypes as Collection>::BlockstoreInterface>()
            .clone()
    }

    pub fn is_genesis_committee(&self) -> bool {
        self.is_genesis_committee
    }
}
