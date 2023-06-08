use fleek_crypto::{AccountOwnerPublicKey, NodePublicKey};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PublicKeyParam {
    pub public_key: AccountOwnerPublicKey,
}

#[derive(Deserialize)]
pub struct NodeKeyParam {
    pub node_key: NodePublicKey,
}
