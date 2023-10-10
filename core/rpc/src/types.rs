use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PublicKeyParam {
    pub public_key: EthAddress,
}

#[derive(Deserialize)]
pub struct PublicKeyLatestParam {
    pub public_key: EthAddress,
    pub tag: Option<String>,
}

#[derive(Deserialize)]
pub struct NodeKeyParam {
    pub public_key: NodePublicKey,
}

#[derive(Deserialize)]
pub struct ClientKeyParam {
    pub public_key: ClientPublicKey,
}

#[derive(Deserialize)]
pub struct DhtPutParam {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Deserialize)]
pub struct DhtGetParam {
    pub key: Vec<u8>,
}
