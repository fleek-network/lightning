//! The types used by the Application interface.

use ethers::types::{Block as EthersBlock, H256, U256, U64};
use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use serde::{Deserialize, Serialize};

use crate::TransactionReceipt;

/// The response generated from executing an entire batch of transactions (aka a block).
#[derive(Debug, Hash)]
pub struct BlockExecutionResponse {
    pub block_number: u128,
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

impl From<BlockExecutionResponse> for EthersBlock<H256> {
    fn from(value: BlockExecutionResponse) -> Self {
        Self {
            hash: Some(value.block_hash.into()),
            parent_hash: value.parent_hash.into(),
            uncles_hash: H256::zero(),
            author: None,
            state_root: H256::zero(),
            transactions_root: H256::zero(),
            receipts_root: H256::zero(),
            number: Some(U64::from(value.block_number as u64)),
            gas_used: U256::zero(),
            gas_limit: U256::zero(),
            extra_data: Default::default(),
            logs_bloom: None,
            timestamp: U256::zero(),
            difficulty: U256::zero(),
            total_difficulty: None,
            seal_fields: Default::default(),
            uncles: Default::default(),
            transactions: value
                .txn_receipts
                .iter()
                .map(|txn| H256(txn.transaction_hash))
                .collect(),
            size: None,
            mix_hash: None,
            nonce: None,
            base_fee_per_gas: None,
            withdrawals_root: None,
            withdrawals: None,
            other: Default::default(),
        }
    }
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
