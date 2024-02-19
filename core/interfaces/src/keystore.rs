use fleek_crypto::{ConsensusPublicKey, ConsensusSecretKey, NodePublicKey, NodeSecretKey};

use crate::infu_collection::Collection;
use crate::{ConfigConsumer, ConfigProviderInterface};

#[infusion::service]
pub trait KeystoreInterface<C: Collection>: ConfigConsumer + Clone + Sized + Send + Sync {
    fn _init(config: ::ConfigProviderInterface) {
        Self::init(config.get::<Self>())
    }

    /// Initialize the keystore
    fn init(config: Self::Config) -> anyhow::Result<Self>;

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
    /// Otherwise if true, partial keys will be preserved and only missing ones will be generated.
    fn generate_keys(provider: Self::Config, accept_partial: bool) -> anyhow::Result<()>;
}
