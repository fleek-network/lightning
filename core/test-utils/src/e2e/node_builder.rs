use std::path::PathBuf;

use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::{Application, ApplicationConfig};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_broadcast::Broadcast;
use lightning_checkpointer::{Checkpointer, CheckpointerConfig, CheckpointerDatabaseConfig};
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::MyReputationReporter;
use lightning_rpc::config::Config as RpcConfig;
use lightning_rpc::Rpc;
use lightning_utils::config::TomlConfigProvider;
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;

use super::{TestNode, TestNodeBeforeGenesisReadyState, TestNodeComponents};
use crate::consensus::{MockConsensus, MockConsensusGroup, MockForwarder};
use crate::keys::EphemeralKeystore;

pub struct TestNodeBuilder {
    home_dir: PathBuf,
    use_mock_consensus: bool,
    mock_consensus_group: Option<MockConsensusGroup>,
}

impl TestNodeBuilder {
    pub fn new(home_dir: PathBuf) -> Self {
        Self {
            home_dir,
            use_mock_consensus: true,
            mock_consensus_group: None,
        }
    }

    pub fn with_mock_consensus(mut self, mock_consensus_group: Option<MockConsensusGroup>) -> Self {
        self.use_mock_consensus = true;
        self.mock_consensus_group = mock_consensus_group;
        self
    }

    pub fn without_mock_consensus(mut self) -> Self {
        self.use_mock_consensus = false;
        self.mock_consensus_group = None;
        self
    }

    pub async fn build(self) -> Result<TestNode> {
        let config = TomlConfigProvider::<TestNodeComponents>::new();

        // Configure application component.
        config.inject::<Application<TestNodeComponents>>(ApplicationConfig {
            genesis_path: None,
            db_path: Some(self.home_dir.join("app").try_into().unwrap()),
            ..Default::default()
        });

        // Configure blockstore component.
        config.inject::<Blockstore<TestNodeComponents>>(BlockstoreConfig {
            root: self.home_dir.join("blockstore").try_into().unwrap(),
        });

        // Configure checkpointer component.
        config.inject::<Checkpointer<TestNodeComponents>>(CheckpointerConfig {
            database: CheckpointerDatabaseConfig {
                path: self.home_dir.join("checkpointer").try_into().unwrap(),
            },
        });

        // Configure consensus component.
        if self.use_mock_consensus {
            config.inject::<MockConsensus<TestNodeComponents>>(
                self.mock_consensus_group
                    .as_ref()
                    .map(|group| group.config.clone())
                    .unwrap_or_default(),
            );
        }

        // Configure pool component.
        config.inject::<PoolProvider<TestNodeComponents>>(PoolConfig {
            // Specify port 0 to get a random available port.
            address: "0.0.0.0:0".parse().unwrap(),
            ..Default::default()
        });

        // Configure RPC component.
        config.inject::<Rpc<TestNodeComponents>>(RpcConfig {
            // Specify port 0 to get a random available port.
            addr: "0.0.0.0:0".parse().unwrap(),
            hmac_secret_dir: Some(self.home_dir.clone()),
            ..Default::default()
        });

        // Configure keystore component.
        config.inject::<EphemeralKeystore<TestNodeComponents>>(Default::default());

        // Initialize the node.
        let mut provider = fdi::Provider::default().with(config);
        if let Some(mock_consensus_group) = self.mock_consensus_group {
            provider = provider.with(mock_consensus_group);
        }
        let node = Node::<TestNodeComponents>::init_with_provider(provider)?;

        // Start the node.
        node.start().await;

        // Wait for the node to be ready.
        let shutdown = node
            .shutdown_waiter()
            .expect("node missing shutdown waiter");

        // Wait for components to be ready before building genesis.
        let before_genesis_ready = TokioReadyWaiter::new();
        {
            let pool = node.provider.get::<PoolProvider<TestNodeComponents>>();
            let rpc = node.provider.get::<Rpc<TestNodeComponents>>();
            let before_genesis_ready = before_genesis_ready.clone();
            let shutdown = shutdown.clone();
            spawn!(
                async move {
                    // Wait for pool to be ready.
                    let pool_state = pool.wait_for_ready().await;

                    // Wait for rpc to be ready.
                    let rpc_state = rpc.wait_for_ready().await;

                    // Notify that we are ready.
                    let state = TestNodeBeforeGenesisReadyState {
                        pool_listen_address: pool_state.listen_address.unwrap(),
                        rpc_listen_address: rpc_state.listen_address,
                    };
                    before_genesis_ready.notify(state);
                },
                "TEST-NODE before genesis ready watcher",
                crucial(shutdown)
            );
        }

        // Wait for components to be ready after genesis.
        let after_genesis_ready = TokioReadyWaiter::new();
        {
            let app_query = node
                .provider
                .get::<Application<TestNodeComponents>>()
                .sync_query();
            let checkpointer = node.provider.get::<Checkpointer<TestNodeComponents>>();
            let after_genesis_ready = after_genesis_ready.clone();
            let shutdown = shutdown.clone();
            spawn!(
                async move {
                    // Wait for genesis to be applied.
                    app_query.wait_for_genesis().await;

                    // Wait for the checkpointer to be ready.
                    checkpointer.wait_for_ready().await;

                    // Notify that we are ready.
                    after_genesis_ready.notify(());
                },
                "TEST-NODE after genesis ready watcher",
                crucial(shutdown)
            );
        }

        let app = node.provider.get::<Application<TestNodeComponents>>();
        Ok(TestNode {
            app_query: app.sync_query(),
            app,
            broadcast: node.provider.get::<Broadcast<TestNodeComponents>>(),
            checkpointer: node.provider.get::<Checkpointer<TestNodeComponents>>(),
            forwarder: node.provider.get::<MockForwarder<TestNodeComponents>>(),
            keystore: node.provider.get::<EphemeralKeystore<TestNodeComponents>>(),
            notifier: node.provider.get::<Notifier<TestNodeComponents>>(),
            pool: node.provider.get::<PoolProvider<TestNodeComponents>>(),
            rpc: node.provider.get::<Rpc<TestNodeComponents>>(),
            reputation_reporter: node.provider.get::<MyReputationReporter>(),

            inner: node,
            before_genesis_ready,
            after_genesis_ready,
            home_dir: self.home_dir.clone(),
            owner_secret_key: AccountOwnerSecretKey::generate(),
        })
    }
}
