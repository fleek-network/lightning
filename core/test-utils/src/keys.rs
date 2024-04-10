use std::marker::PhantomData;

use fdi::{BuildGraph, DependencyGraph};
use fleek_crypto::{
    ConsensusPublicKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use lightning_interfaces::{Collection, ConfigConsumer, KeystoreInterface};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct EphemeralKeystore<C> {
    node_sk: NodeSecretKey,
    consensus_sk: ConsensusSecretKey,
    _p: PhantomData<C>,
}

impl<C> ConfigConsumer for EphemeralKeystore<C> {
    const KEY: &'static str = "keystore";
    type Config = EphemeralKeystoreConfig;
}

#[derive(Serialize, Deserialize, Default)]
pub struct EphemeralKeystoreConfig {}

impl<C> Default for EphemeralKeystore<C> {
    fn default() -> Self {
        Self {
            node_sk: SecretKey::generate(),
            consensus_sk: SecretKey::generate(),
            _p: PhantomData,
        }
    }
}

impl<C: Collection> BuildGraph for EphemeralKeystore<C> {
    fn build_graph() -> fdi::DependencyGraph {
        DependencyGraph::default().with_infallible(Self::default)
    }
}

impl<C: Collection> KeystoreInterface<C> for EphemeralKeystore<C> {
    fn get_ed25519_pk(&self) -> NodePublicKey {
        self.node_sk.to_pk()
    }

    fn get_ed25519_sk(&self) -> NodeSecretKey {
        self.node_sk.clone()
    }

    fn get_bls_pk(&self) -> ConsensusPublicKey {
        self.consensus_sk.to_pk()
    }

    fn get_bls_sk(&self) -> ConsensusSecretKey {
        self.consensus_sk.clone()
    }

    fn generate_keys(_provider: Self::Config, _accept_partial: bool) -> anyhow::Result<()> {
        Ok(())
    }
}
