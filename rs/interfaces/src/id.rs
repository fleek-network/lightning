use std::hash::Hash;

use crate::config::ConfigConsumer;

pub trait IdentityProvider {
    /// The public key of a node. Currently BLS12-381.
    type NodePk: Copy + Clone + Eq + PartialEq + Hash + AsRef<[u8]>;

    /// The public key of a node used for network transmission. Currently Ed25519.
    type NodeNetworkingPk: Copy + Clone + Eq + PartialEq + Hash + AsRef<[u8]>;

    /// The public key of anyone with a FLK balance. Such as a node runner.
    type AccountOwnerPk: Copy + Clone + Eq + PartialEq + Hash + AsRef<[u8]>;

    /// The public key of a client. This is someone who has a USD balance and
    /// can sign delivery acknowledgments.
    type ClientPk: Copy + Clone + Eq + PartialEq + Hash + AsRef<[u8]>;

    fn verify_node_signature(pk: &Self::NodePk, signature: &[u8], digest: &[u8; 32]);

    fn verify_networking_signature(
        pk: &Self::NodeNetworkingPk,
        signature: &[u8],
        digest: &[u8; 32],
    );

    fn verify_account_owner_signature(
        pk: &Self::AccountOwnerPk,
        signature: &[u8],
        digest: &[u8; 32],
    );

    fn verify_client_signature(pk: &Self::ClientPk, signature: &[u8], digest: &[u8; 32]);
}

pub trait NodeSigner<I: IdentityProvider>: ConfigConsumer {}
