use std::time::Duration;

use fleek_crypto::AccountOwnerSecretKey;
use futures::Future;
use lightning_blockstore::blockstore::Blockstore;
use lightning_interfaces::fdi::MultiThreadedProvider;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{NodePorts, Staking};
use lightning_node::ContainedNode;
use lightning_node_bindings::FullNodeComponents;
use lightning_rpc::api::FleekApiClient;
use lightning_rpc::{Rpc, RpcClient};
use lightning_utils::config::TomlConfigProvider;
use lightning_utils::poll::{poll_until, PollUntilError};
use types::{Epoch, NodeIndex};

use crate::error::SwarmError;

pub struct ContainerizedNode {
    config: TomlConfigProvider<FullNodeComponents>,
    owner_secret_key: AccountOwnerSecretKey,
    node: ContainedNode<FullNodeComponents>,
    index: NodeIndex,
    genesis_stake: Staking,
    ports: NodePorts,
    is_genesis_committee: bool,
    started: bool,
}

impl ContainerizedNode {
    pub fn new(
        config: TomlConfigProvider<FullNodeComponents>,
        owner_secret_key: AccountOwnerSecretKey,
        ports: NodePorts,
        index: NodeIndex,
        is_genesis_committee: bool,
        genesis_stake: Staking,
    ) -> Self {
        let provider = MultiThreadedProvider::default();
        provider.insert(config.clone());
        let node =
            ContainedNode::<FullNodeComponents>::new(provider, Some(format!("NODE-{index}")));
        Self {
            config,
            owner_secret_key,
            node,
            index,
            genesis_stake,
            ports,
            is_genesis_committee,
            started: false,
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        // This function has to return a result in order to use try_join_all in swarm.rs
        let handle = self.node.spawn();
        handle.await.unwrap()?;

        self.started = true;

        Ok(())
    }

    pub fn shutdown(self) -> impl Future<Output = ()> {
        self.node.shutdown()
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn get_ports(&self) -> &NodePorts {
        &self.ports
    }

    pub fn get_rpc_address(&self) -> String {
        let config = self.config.get::<Rpc<FullNodeComponents>>();
        format!("http://{}", config.addr())
    }

    pub fn get_rpc_client(&self) -> RpcClient {
        RpcClient::new_no_auth(&self.get_rpc_address()).unwrap()
    }

    pub fn get_owner_secret_key(&self) -> AccountOwnerSecretKey {
        self.owner_secret_key.clone()
    }

    pub fn get_index(&self) -> NodeIndex {
        self.index
    }

    pub fn get_genesis_stake(&self) -> Staking {
        self.genesis_stake.clone()
    }

    pub fn take_syncronizer(&self) -> fdi::Ref<c!(FullNodeComponents::SyncronizerInterface)> {
        self.node
            .provider()
            .get::<<FullNodeComponents as NodeComponents>::SyncronizerInterface>()
    }

    pub fn take_blockstore(&self) -> Blockstore<FullNodeComponents> {
        self.node
            .provider()
            .get::<<FullNodeComponents as NodeComponents>::BlockstoreInterface>()
            .clone()
    }

    pub fn take_blockstore_server_socket(&self) -> BlockstoreServerSocket {
        self.node
            .provider()
            .get::<<FullNodeComponents as NodeComponents>::BlockstoreServerInterface>()
            .get_socket()
    }

    pub fn take_fetcher_server_socket(&self) -> FetcherSocket {
        self.node
            .provider()
            .get::<<FullNodeComponents as NodeComponents>::FetcherInterface>()
            .get_socket()
    }

    pub fn is_genesis_committee(&self) -> bool {
        self.is_genesis_committee
    }

    pub fn provider(&self) -> &MultiThreadedProvider {
        self.node.provider()
    }

    pub async fn wait_for_rpc_ready(&self) {
        self.node.wait_for_rpc_ready().await
    }

    pub async fn wait_for_epoch_change(
        &self,
        new_epoch: Epoch,
        timeout: Duration,
    ) -> Result<(), SwarmError> {
        poll_until(
            || async {
                let client = self.get_rpc_client();
                let epoch = client
                    .get_epoch()
                    .await
                    .map_err(|_| PollUntilError::ConditionNotSatisfied)?;

                (epoch == new_epoch)
                    .then_some(())
                    .ok_or(PollUntilError::ConditionNotSatisfied)
            },
            timeout,
            Duration::from_millis(100),
        )
        .await
        .map_err(|e| SwarmError::Internal(e.to_string()))
    }
}
