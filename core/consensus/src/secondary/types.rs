use derive_more::{From, IsVariant, TryInto};
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature};
use narwhal_config::Committee;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, IsVariant, From, TryInto)]
pub enum PubSubMessage {
    Certificate(narwhal_types::Certificate),
    Batch(narwhal_types::Batch),
}
