use ethers::types::{TransactionReceipt as EthersTxnReceipt, U256, U64};
use fleek_crypto::{EthAddress, TransactionSender};
use serde::{Deserialize, Serialize};

use super::{Epoch, NodeInfo};
use crate::{Event, UpdateMethod};

/// Info on a Narwhal epoch
#[derive(
    Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct EpochInfo {
    /// List of committee members
    pub committee: Vec<NodeInfo>,
    /// The current epoch number
    pub epoch: Epoch,
    /// Timestamp when the epoch ends
    pub epoch_end: u64,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub enum TransactionResponse {
    Success(ExecutionData),
    Revert(ExecutionError),
}

impl TransactionResponse {
    pub fn is_success(&self) -> bool {
        match self {
            Self::Success(_) => true,
            Self::Revert(_) => false,
        }
    }
}

// todo(dalton): Get something in here to indicate which function it called
#[derive(Clone, Debug, Hash, Serialize, Deserialize, Eq, PartialEq)]
pub struct TransactionReceipt {
    /// The hash of the block where the given transaction was included.
    pub block_hash: [u8; 32],
    /// The number of the block where the given transaction was included.
    pub block_number: u64,
    /// The index of the transaction within the block.
    pub transaction_index: u64,
    /// Hash of the transaction
    pub transaction_hash: [u8; 32],
    /// The sender of the transaction
    pub from: TransactionSender,
    /// Which state function or address a transaction was headed to
    pub to: TransactionDestination,
    /// The results of the transaction
    pub response: TransactionResponse,
    /// The event that was emitted by the transaction
    pub event: Option<Event>,
}

/// What state function a transaction was calling. If an ethereum transaction it will either be
/// Fleek Contract address or another ethereum address, if its another ethereum address it would
/// indicate that the transaction was a transfer
#[derive(Clone, Debug, Hash, Serialize, Deserialize, Eq, PartialEq)]
pub enum TransactionDestination {
    Fleek(UpdateMethod),
    Ethereum(EthAddress),
}

impl TransactionDestination {
    fn to_eth_address(&self) -> EthAddress {
        match self {
            Self::Ethereum(address) => *address,
            _ => [0; 20].into(),
        }
    }
}

impl From<TransactionReceipt> for EthersTxnReceipt {
    fn from(value: TransactionReceipt) -> Self {
        let sender = if let TransactionSender::AccountOwner(address) = value.from {
            address.0.into()
        } else {
            [0u8; 20].into()
        };
        Self {
            transaction_hash: value.transaction_hash.into(),
            transaction_index: value.transaction_index.into(),
            block_hash: Some(value.block_hash.into()),
            block_number: Some((value.block_number).into()),
            from: sender,
            to: Some(value.to.to_eth_address().0.into()),
            gas_used: Some(U256::zero()),
            transaction_type: Some(U64::one()),
            effective_gas_price: Some(U256::zero()),
            status: Some(if value.response.is_success() {
                U64::one()
            } else {
                U64::zero()
            }),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub enum ExecutionData {
    None,
    String(String),
    UInt(u128),
    EpochInfo(EpochInfo),
    EpochChange,
}

/// Error type for transaction execution on the application layer
#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub enum ExecutionError {
    CommitteeSelectionBeaconNotCommitted,
    CommitteeSelectionBeaconNotInCommitPhase,
    CommitteeSelectionBeaconCommitPhaseNotStarted,
    CommitteeSelectionAlreadyCommitted,
    CommitteeSelectionBeaconRevealPhaseNotStarted,
    CommitteeSelectionAlreadyRevealed,
    CommitteeSelectionBeaconNotInRevealPhase,
    CommitteeSelectionBeaconInvalidReveal,
    CommitteeSelectionBeaconCommitPhaseNotTimedOut,
    CommitteeSelectionBeaconRevealPhaseNotTimedOut,
    CommitteeSelectionBeaconNonRevealingNode,
    MissingCommittee,
    InsufficientBalance,
    InvalidChainId,
    InvalidSignature,
    InvalidNonce,
    InvalidProof,
    InvalidInternetAddress,
    InsufficientNodeDetails,
    InvalidStateFunction,
    InvalidConsensusKey,
    InvalidToken,
    InvalidStateForContentRemoval,
    InvalidContentRemoval,
    NoLockedTokens,
    TokensLocked,
    NotNodeOwner,
    NotCommitteeMember,
    NodeDoesNotExist,
    CantSendToYourself,
    AlreadySignaled,
    SubmittedTooManyTransactions,
    NonExistingService,
    OnlyAccountOwner,
    OnlyNode,
    OnlyGovernance,
    InvalidServiceId,
    InsufficientStake,
    LockExceededMaxStakeLockTime,
    LockedTokensUnstakeForbidden,
    EpochAlreadyChanged,
    EpochHasNotStarted,
    ConsensusKeyAlreadyIndexed,
    ContentAlreadyRegistered,
    Unimplemented,
    TooManyMeasurements,
    TooManyUpdates,
    TooManyUpdatesForContent,
}
