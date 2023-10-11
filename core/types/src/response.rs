use serde::{Deserialize, Serialize};

use super::{Epoch, NodeInfo};

/// Info on a Narwhal epoch
#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
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
    InsufficientBalance,
    InvalidSignature,
    InvalidNonce,
    InvalidProof,
    InvalidInternetAddress,
    InsufficientNodeDetails,
    InvalidStateFunction,
    InvalidConsensusKey,
    InvalidToken,
    NoLockedTokens,
    TokensLocked,
    NotNodeOwner,
    NotCommitteeMember,
    NodeDoesNotExist,
    AlreadySignaled,
    NonExistingService,
    OnlyAccountOwner,
    OnlyNode,
    OnlyGovernance,
    InvalidServiceId,
    InsufficientStakesToLock,
    LockExceededMaxStakeLockTime,
    LockedTokensUnstakeForbidden,
    EpochAlreadyChanged,
    EpochHasNotStarted,
    ConsensusKeyAlreadyIndexed,
}
