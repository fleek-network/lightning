//! The types used by the Application interface.

use ethers::types::{Block as EthersBlock, H256, U64};
use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use serde::{Deserialize, Serialize};

use crate::TransactionReceipt;

/// The response generated from executing an entire batch of transactions (aka a block).
#[derive(Debug, Hash, Clone)]
pub struct BlockExecutionResponse {
    /// The number of the block
    pub block_number: u64,
    /// The new block hash
    pub block_hash: [u8; 32],
    /// The hash of the previous block
    pub parent_hash: [u8; 32],
    /// This *flag* is only set to `true` if performing a transaction in the block
    /// has determined that we should move the epoch forward.
    pub change_epoch: bool,
    /// The changes to the node registry.
    pub node_registry_delta: Vec<(NodePublicKey, NodeRegistryChange)>,
    /// Receipts of all executed transactions
    pub txn_receipts: Vec<TransactionReceipt>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct BlockReceipt {
    pub block_number: u64,
    /// The new block hash
    pub block_hash: [u8; 32],
    /// The hash of the previous block
    pub parent_hash: [u8; 32],
    /// This *flag* is only set to `true` if performing a transaction in the block
    /// has determined that we should move the epoch forward.
    pub change_epoch: bool,
    /// The changes to the node registry.
    pub node_registry_delta: Vec<(NodePublicKey, NodeRegistryChange)>,
    /// The hashes of the transactions included in the block
    pub txn_hashes: Vec<[u8; 32]>,
}

impl BlockExecutionResponse {
    /// Consumes self returning a block receipt  all of the txn_receipts
    pub fn to_receipts(self) -> (BlockReceipt, Vec<TransactionReceipt>) {
        let block_receipt = BlockReceipt {
            block_number: self.block_number,
            block_hash: self.block_hash,
            parent_hash: self.parent_hash,
            change_epoch: self.change_epoch,
            node_registry_delta: self.node_registry_delta,
            txn_hashes: self
                .txn_receipts
                .iter()
                .map(|txn| txn.transaction_hash)
                .collect(),
        };

        let txn_receipts = self.txn_receipts;

        (block_receipt, txn_receipts)
    }
}

impl From<BlockReceipt> for EthersBlock<H256> {
    fn from(value: BlockReceipt) -> Self {
        Self {
            hash: Some(value.block_hash.into()),
            parent_hash: value.parent_hash.into(),
            number: Some(U64::from(value.block_number)),
            transactions: value.txn_hashes.iter().map(|t| H256(*t)).collect(),
            ..Default::default()
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize, Clone)]
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
