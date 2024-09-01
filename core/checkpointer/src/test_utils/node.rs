use std::net::SocketAddr;

use lightning_application::Application;
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_test_utils::keys::EphemeralKeystore;
use ready::tokio::TokioReadyWaiter;
use types::NodeIndex;

use super::TestNodeComponents;
use crate::Checkpointer;

pub struct TestNode {
    pub inner: Node<TestNodeComponents>,
    pub before_genesis_ready: TokioReadyWaiter<TestNodeBeforeGenesisReadyState>,
    pub after_genesis_ready: TokioReadyWaiter<()>,

    pub app: fdi::Ref<Application<TestNodeComponents>>,
    pub checkpointer: fdi::Ref<Checkpointer<TestNodeComponents>>,
    pub keystore: fdi::Ref<EphemeralKeystore<TestNodeComponents>>,
    pub notifier: fdi::Ref<Notifier<TestNodeComponents>>,
}

impl TestNode {
    pub async fn start(&mut self) {
        self.inner.start().await;
    }

    pub async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }

    pub fn get_id(&self) -> Option<NodeIndex> {
        self.app
            .sync_query()
            .pubkey_to_index(&self.keystore.get_ed25519_pk())
    }
}

#[derive(Clone, Debug)]
pub struct TestNodeBeforeGenesisReadyState {
    pub pool_listen_address: SocketAddr,
}

impl Default for TestNodeBeforeGenesisReadyState {
    fn default() -> Self {
        Self {
            pool_listen_address: "0.0.0.0:0".parse().unwrap(),
        }
    }
}
