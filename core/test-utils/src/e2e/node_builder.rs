use std::path::PathBuf;

use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::state::QueryRunner;
use lightning_application::{Application, ApplicationConfig};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_checkpointer::{Checkpointer, CheckpointerConfig, CheckpointerDatabaseConfig};
use lightning_interfaces::prelude::*;
use lightning_node::ContainedNode;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rpc::config::Config as RpcConfig;
use lightning_rpc::Rpc;
use lightning_utils::config::TomlConfigProvider;
use tempfile::{tempdir, TempDir};

use super::{BoxedTestNode, TestFullNode};
use crate::consensus::{MockConsensus, MockConsensusGroup};
use crate::keys::EphemeralKeystore;

pub struct TestNodeBuilder {
    _temp_dir: TempDir,
    home_dir: PathBuf,
    use_mock_consensus: bool,
    mock_consensus_group: Option<MockConsensusGroup>,
    is_genesis_committee: Option<bool>,
}

impl Default for TestNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestNodeBuilder {
    pub fn new() -> Self {
        let temp_dir = tempdir().unwrap();
        let home_dir = temp_dir.path().to_path_buf();
        Self {
            _temp_dir: temp_dir,
            home_dir,
            use_mock_consensus: true,
            mock_consensus_group: None,
            is_genesis_committee: None,
        }
    }

    pub fn with_mock_consensus(mut self, mock_consensus_group: MockConsensusGroup) -> Self {
        self.use_mock_consensus = true;
        self.mock_consensus_group = Some(mock_consensus_group);
        self
    }

    pub fn without_mock_consensus(mut self) -> Self {
        self.use_mock_consensus = false;
        self.mock_consensus_group = None;
        self
    }

    pub fn with_is_genesis_committee(mut self, is_genesis_committee: bool) -> Self {
        self.is_genesis_committee = Some(is_genesis_committee);
        self
    }

    pub async fn build<C: NodeComponents>(self) -> Result<BoxedTestNode>
    where
        C::ApplicationInterface: ApplicationInterface<C, SyncExecutor = QueryRunner>,
    {
        let config = TomlConfigProvider::<C>::new();

        // Configure application component.
        config.inject::<Application<C>>(ApplicationConfig {
            genesis_path: None,
            db_path: Some(self.home_dir.join("app").try_into().unwrap()),
            ..Default::default()
        });

        // Configure blockstore component.
        config.inject::<Blockstore<C>>(BlockstoreConfig {
            root: self.home_dir.join("blockstore").try_into().unwrap(),
        });

        // Configure checkpointer component.
        config.inject::<Checkpointer<C>>(CheckpointerConfig {
            database: CheckpointerDatabaseConfig {
                path: self.home_dir.join("checkpointer").try_into().unwrap(),
            },
        });

        // Configure consensus component.
        if self.use_mock_consensus {
            config.inject::<MockConsensus<C>>(
                self.mock_consensus_group
                    .as_ref()
                    .map(|group| group.config.clone())
                    .unwrap_or_default(),
            );
        }

        // Configure pool component.
        config.inject::<PoolProvider<C>>(PoolConfig {
            // Specify port 0 to get a random available port.
            address: "0.0.0.0:0".parse().unwrap(),
            ..Default::default()
        });

        // Configure RPC component.
        config.inject::<Rpc<C>>(RpcConfig {
            // Specify port 0 to get a random available port.
            addr: "0.0.0.0:0".parse().unwrap(),
            hmac_secret_dir: Some(self.home_dir.clone()),
            ..Default::default()
        });

        // Configure keystore component.
        config.inject::<EphemeralKeystore<C>>(Default::default());

        // Initialize the node.
        let mut provider = fdi::MultiThreadedProvider::default().with(config);
        if let Some(mock_consensus_group) = self.mock_consensus_group {
            provider = provider.with(mock_consensus_group);
        }
        let node = ContainedNode::<C>::new(provider, None);

        Ok(Box::new(TestFullNode {
            inner: node,
            home_dir: self.home_dir.clone(),
            owner_secret_key: AccountOwnerSecretKey::generate(),
            is_genesis_committee: self.is_genesis_committee.unwrap_or(true),
            pool_listen_address: None,
            rpc_listen_address: None,
        }))
    }
}
