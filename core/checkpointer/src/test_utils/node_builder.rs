use std::path::PathBuf;

use anyhow::Result;
use lightning_application::{Application, ApplicationConfig};
use lightning_broadcast::Broadcast;
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;

use super::{TestNode, TestNodeComponents};
use crate::test_utils::TestNodeBeforeGenesisReadyState;
use crate::{Checkpointer, CheckpointerConfig};

pub struct TestNodeBuilder {
    home_dir: PathBuf,
}

impl TestNodeBuilder {
    pub fn new(home_dir: PathBuf) -> Self {
        Self { home_dir }
    }

    pub async fn build(self) -> Result<TestNode> {
        let config = JsonConfigProvider::default()
            .with::<Application<TestNodeComponents>>(ApplicationConfig {
                genesis_path: None,
                db_path: Some(self.home_dir.join("app").try_into().unwrap()),
                ..Default::default()
            })
            .with::<Checkpointer<TestNodeComponents>>(CheckpointerConfig::default_with_home_dir(
                self.home_dir.as_path(),
            ))
            .with::<PoolProvider<TestNodeComponents>>(PoolConfig {
                // Specify port 0 to get a random available port.
                address: "0.0.0.0:0".parse().unwrap(),
                ..Default::default()
            });

        let keystore = EphemeralKeystore::<TestNodeComponents>::default();

        let node = Node::<TestNodeComponents>::init_with_provider(
            fdi::Provider::default().with(config).with(keystore),
        )?;

        node.start().await;

        let shutdown = node
            .shutdown_waiter()
            .expect("node missing shutdown waiter");

        // Wait for pool to be ready before building genesis.
        let before_genesis_ready = TokioReadyWaiter::new();
        {
            let pool = node.provider.get::<PoolProvider<TestNodeComponents>>();
            let before_genesis_ready = before_genesis_ready.clone();
            let shutdown = shutdown.clone();
            spawn!(
                async move {
                    // Wait for pool to be ready.
                    let pool_state = pool.wait_for_ready().await;
                    let state = TestNodeBeforeGenesisReadyState {
                        pool_listen_address: pool_state.listen_address.unwrap(),
                    };

                    // Notify that we are ready.
                    before_genesis_ready.notify(state);
                },
                "TEST-NODE before genesis ready watcher",
                crucial(shutdown)
            );
        }

        // Wait for checkpointer to be ready after genesis.
        let after_genesis_ready = TokioReadyWaiter::new();
        {
            let checkpointer = node.provider.get::<Checkpointer<TestNodeComponents>>();
            let after_genesis_ready = after_genesis_ready.clone();
            let shutdown = shutdown.clone();
            spawn!(
                async move {
                    // Wait for checkpointer to be ready.
                    checkpointer.wait_for_ready().await;

                    // Notify that we are ready.
                    after_genesis_ready.notify(());
                },
                "TEST-NODE after genesis ready watcher",
                crucial(shutdown)
            );
        }

        Ok(TestNode {
            app: node.provider.get::<Application<TestNodeComponents>>(),
            checkpointer: node.provider.get::<Checkpointer<TestNodeComponents>>(),
            keystore: node.provider.get::<EphemeralKeystore<TestNodeComponents>>(),
            notifier: node.provider.get::<Notifier<TestNodeComponents>>(),
            pool: node.provider.get::<PoolProvider<TestNodeComponents>>(),
            broadcast: node.provider.get::<Broadcast<TestNodeComponents>>(),

            inner: node,
            before_genesis_ready,
            after_genesis_ready,
        })
    }
}
