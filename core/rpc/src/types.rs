use fleek_crypto::{AccountOwnerPublicKey, ClientPublicKey, NodePublicKey};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PublicKeyParam {
    pub public_key: AccountOwnerPublicKey,
}

#[derive(Deserialize)]
pub struct NodeKeyParam {
    pub public_key: NodePublicKey,
}

#[derive(Deserialize)]
pub struct ClientKeyParam {
    pub public_key: ClientPublicKey,
}
