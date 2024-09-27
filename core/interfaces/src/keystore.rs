use fdi::BuildGraph;
use fleek_crypto::{ConsensusPublicKey, ConsensusSecretKey, NodePublicKey, NodeSecretKey};

use crate::components::NodeComponents;
use crate::ConfigConsumer;

#[interfaces_proc::blank]
pub trait KeystoreInterface<C: NodeComponents>:
    BuildGraph + ConfigConsumer + Clone + Sized + Send + Sync
{
    /// Returns the Ed25519 public key
    fn get_ed25519_pk(&self) -> NodePublicKey;

    /// Returns the raw Ed25519 secret key. Should be used with caution!
    fn get_ed25519_sk(&self) -> NodeSecretKey;

    /// Returns the BLS public key
    fn get_bls_pk(&self) -> ConsensusPublicKey;

    /// Returns the raw BLS secret key. Should be used with caution!
    fn get_bls_sk(&self) -> ConsensusSecretKey;

    /// Standalone utility to generate keys from a given config.
    /// If accept_partial is false, then all keys MUST NOT exist for this method to return ok.
    /// Otherwise if true, partial_node_components keys will be preserved and only missing ones will
    /// be generated.
    fn generate_keys(provider: Self::Config, accept_partial: bool) -> anyhow::Result<()>;
}
