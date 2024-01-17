use std::path::Path;

use affair::{Executor, TokioSpawn, Worker};
use fleek_crypto::{
    ConsensusPublicKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    NodeSignature,
    SecretKey,
};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::UpdateMethod;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    MempoolSocket,
    Notification,
    SignerInterface,
    SubmitTxSocket,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};

pub struct KeyOnlySigner {
    node_sk: NodeSecretKey,
    consensus_sk: ConsensusSecretKey,
}

#[derive(Serialize, Deserialize, Default)]
pub struct KeyOnlySignerConfig {}

impl<C: Collection> SignerInterface<C> for KeyOnlySigner {
    fn init(
        _config: Self::Config,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        Ok(Self {
            node_sk: SecretKey::generate(),
            consensus_sk: SecretKey::generate(),
        })
    }

    fn provide_mempool(&mut self, _mempool: MempoolSocket) {}
    fn provide_new_block_notify(
        &mut self,
        _new_block_rx: tokio::sync::mpsc::Receiver<Notification>,
    ) {
    }

    fn get_bls_pk(&self) -> ConsensusPublicKey {
        self.consensus_sk.to_pk()
    }

    fn get_ed25519_pk(&self) -> NodePublicKey {
        self.node_sk.to_pk()
    }

    fn get_sk(&self) -> (ConsensusSecretKey, NodeSecretKey) {
        (self.consensus_sk.clone(), self.node_sk.clone())
    }

    fn get_socket(&self) -> SubmitTxSocket {
        struct Lazy {
            counter: u64,
        }
        impl Worker for Lazy {
            type Request = UpdateMethod;
            type Response = u64;

            fn handle(&mut self, _req: Self::Request) -> Self::Response {
                self.counter += 1;
                self.counter
            }
        }
        TokioSpawn::spawn(Lazy { counter: 0 })
    }

    fn sign_raw_digest(&self, digest: &[u8; 32]) -> NodeSignature {
        self.node_sk.sign(digest)
    }

    fn generate_node_key(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn generate_consensus_key(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}

impl WithStartAndShutdown for KeyOnlySigner {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

impl ConfigConsumer for KeyOnlySigner {
    const KEY: &'static str = "signer";
    type Config = KeyOnlySignerConfig;
}
