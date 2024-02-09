use fn_sdk::api::Origin as ApiOrigin;
use serde::{Deserialize, Serialize};

/// Request to execute some javascript from an origin
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Request {
    /// Origin to use
    pub origin: Origin,
    /// URI For the origin
    /// - for blake3 should be hex encoded bytes
    /// - for ipfs should be cid string
    pub uri: String,
    /// Optional path to provide as the window location
    pub path: Option<String>,
    /// Parameter to pass to the script's main function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Unknown,
}

impl From<Origin> for ApiOrigin {
    #[inline(always)]
    fn from(val: Origin) -> Self {
        match val {
            Origin::Ipfs => ApiOrigin::IPFS,
            _ => unreachable!(),
        }
    }
}
