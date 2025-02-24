use derive_more::{From, IsVariant, TryInto};
use serde::{Deserialize, Serialize};

use crate::{
    AccountOwnerPublicKey,
    AccountOwnerSignature,
    ConsensusPublicKey,
    ConsensusSignature,
    EthAddress,
    NodePublicKey,
    NodeSignature,
    PublicKey,
};

#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    From,
    IsVariant,
    TryInto,
    schemars::JsonSchema,
)]
pub enum TransactionSender {
    NodeConsensus(ConsensusPublicKey),
    NodeMain(NodePublicKey),
    AccountOwner(EthAddress),
}

#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    From,
    IsVariant,
    TryInto,
    schemars::JsonSchema,
)]
pub enum TransactionSignature {
    NodeConsensus(ConsensusSignature),
    NodeMain(NodeSignature),
    AccountOwner(AccountOwnerSignature),
}

impl TransactionSender {
    pub fn verify(&self, signature: TransactionSignature, digest: &[u8; 32]) -> bool {
        match (self, signature) {
            (TransactionSender::NodeConsensus(pk), TransactionSignature::NodeConsensus(sig)) => {
                pk.verify(&sig, digest).unwrap_or(false)
            },
            (TransactionSender::AccountOwner(pk), TransactionSignature::AccountOwner(sig)) => {
                pk.verify(&sig, digest)
            },
            (TransactionSender::NodeMain(pk), TransactionSignature::NodeMain(sig)) => {
                pk.verify(&sig, digest).unwrap_or(false)
            },
            _ => false,
        }
    }
}

impl From<AccountOwnerPublicKey> for TransactionSender {
    fn from(value: AccountOwnerPublicKey) -> Self {
        TransactionSender::AccountOwner(value.into())
    }
}
