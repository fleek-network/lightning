use std::collections::HashMap;

use ethers::types::{Address, H256, U256, Bytes};
pub use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct DhtPutParam {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Deserialize)]
pub struct DhtGetParam {
    pub key: Vec<u8>,
}

/// A set of account overrides
pub type StateOverride = HashMap<Address, AccountOverride>;

/// Custom account override used in call
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct AccountOverride {
    /// Fake balance to set for the account before executing the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance: Option<U256>,
    /// Fake nonce to set for the account before executing the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,
    /// Fake EVM bytecode to inject into the account before executing the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,
    /// Fake key-value mapping to override all slots in the account storage before executing the
    /// call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<HashMap<H256, U256>>,
    /// Fake key-value mapping to override individual slots in the account storage before executing
    /// the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_diff: Option<HashMap<H256, U256>>,
}

/// Call request for `eth_call` and adjacent methods.
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CallRequest {
    /// From
    pub from: Option<Address>,
    /// To
    pub to: Option<Address>,
    /// Transaction data
    pub input: Option<Bytes>,
    /// Transaction data
    /// This is the same as `input` but is used for backwards compatibility: <https://github.com/ethereum/go-ethereum/issues/15628>
    pub data: Option<Bytes>,
    pub value: Option<U256>
}
