//! The types used by the Application interface.

use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use serde::{Deserialize, Serialize};

use super::TransactionResponse;

/// The response generated from executing an entire batch of transactions (aka a block).
#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub struct BlockExecutionResponse {
    /// The new block hash
    pub block_hash: [u8; 32],
    /// This *flag* is only set to `true` if performing a transaction in the block
    /// has determined that we should move the epoch forward.
    pub change_epoch: bool,
    /// The changes to the node registry.
    pub node_registry_delta: Vec<(NodePublicKey, NodeRegistryChange)>,
    /// Receipts of all executed transactions
    pub txn_receipts: Vec<TransactionResponse>,
}

#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub enum NodeRegistryChange {
    New,
    Removed,
}

/// The account info stored per account on the blockchain
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct AccountInfo {
    /// The accounts FLK balance
    pub flk_balance: HpUfixed<18>,
    /// the accounts stable coin balance
    pub stables_balance: HpUfixed<6>,
    /// The accounts stables/bandwidth balance
    pub bandwidth_balance: u128,
    /// The nonce of the account. Added to each transaction before signed to prevent replays and
    /// enforce ordering
    pub nonce: u64,
}
