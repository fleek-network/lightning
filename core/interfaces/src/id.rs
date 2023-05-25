use std::{fmt::Debug, hash::Hash};

use affair::Socket;
use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroize;

use crate::{config::ConfigConsumer, consensus::MempoolSocket};

/// The primary trait for verifying signatures and public keys for Fleek Network.
pub trait IdentityProvider {
    /// The public key of a node. Currently BLS12-381.
    type NodePk: PublicKey;

    /// The public key of a node used for network transmission. Currently Ed25519.
    type NodeNetworkingPk: PublicKey;

    /// The public key of anyone with a FLK balance. Such as a node runner.
    type AccountOwnerPk: PublicKey;

    /// The public key of a client. This is someone who has a USD balance and
    /// can sign delivery acknowledgments.
    type ClientPk: PublicKey;
}

/// The signer object for a node that is responsible for loading a signature.
pub trait NodeSigner<I: IdentityProvider>: Sized + ConfigConsumer {
    type NodeSk: SecretKey<Pk = I::NodePk>;

    ///
    type NodeNetworkingSk: SecretKey<Pk = I::NodeNetworkingPk>;

    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// This function can be used to return the loaded secret key.
    fn get_sk(&self) -> (Self::NodeSk, Self::NodeNetworkingSk);
}

pub type SignerSocket = Socket<Vec<u8>, Vec<u8>>;

pub trait SignerServiceInterface<I: IdentityProvider> {
    type Signer: NodeSigner<I>;

    /// Instantiates a new signer service.
    fn init(&mut self, signer: &Self::Signer, mempool: MempoolSocket) -> Self;

    ///
    fn get_socket(&self) -> SignerSocket;
}

/// Any type implementing a secret key.
///
/// # Safety
///
/// Types implementing secret key should protect the memory of the secret material,
/// this includes preventing direct accidental reads of this memory, zeroizing it
/// on drop and implementing custom [`Debug`] and [`std::fmt::Display`] for the type
/// which is specified below.
///
/// Other forms of access to the secret material such as `AsRef<[u8]>`, `Deref<T> where
/// T: Debug` must also be avoided.
///
/// # Displaying the secret key.
///
/// As described in the safety guide, a secret key should never leak it's information.
/// To prevent accidental leakage (for example in logs), any type implementing the
/// [`SecretKey`] trait must implement a custom [`Debug`] implementation.
///
/// ## Release Build
///
/// In release build (i.e `#[cfg(release)]`) we should not leak ANYTHING about the
/// secret key, the implementation *MUST* only print the fixed statement: `[SK;REDACTED]`.
///
/// ## Debug Build
///
/// In debug build (i.e `#[cfg(debug)]`) a little more information about a secret
/// key is okay, but this should not be the secret key itself, but you should
/// print the public key instead as an identification: `[SK;pk={}]` where {} is
/// the representation of the public key.
pub trait SecretKey: Zeroize + Clone + Eq {
    /// The public key for this key.
    type Pk: PublicKey;

    /// Returns the public key of this
    fn get_pk(&self) -> Self::Pk;

    /// Sign a message and returns the signature.
    ///
    /// # Safety
    ///
    /// The algorithm used to sign a message should be resistant to side-channel attacks,
    /// most importantly the timing attacks and a constant time implementation
    /// is a requirement.
    fn sign(&self, digest: &[u8; 32]) -> <Self::Pk as PublicKey>::Signature;
}

/// The bounds for a public key.
pub trait PublicKey:
    Serialize + DeserializeOwned + Debug + Copy + Clone + Eq + PartialEq + Hash + AsRef<[u8]>
{
    /// The signature used for this public key, if a constant sized array is
    /// provided its size must be consistent [`SIGNATURE_BYTES`].
    type Signature: Serialize + DeserializeOwned + AsRef<[u8]>;

    /// Number of bytes for this public key.
    const BYTES: usize;

    /// Number of bytes for the signature. This *MUST* be consistent with the
    /// type of [`Self::Signature`].
    const SIGNATURE_BYTES: usize;

    /// Verifies a signature against this public key for the provided message digest.
    fn verify_signature(&self, signature: Self::Signature, digest: &[u8; 32]) -> bool;
}
