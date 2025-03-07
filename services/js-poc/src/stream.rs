use std::collections::HashMap;

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
    /// Optional path to provide as the window location,
    /// including query parameters and the fragment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Parameter to pass to the script's main function.
    /// For http oriented functions, an object can be passed
    /// here to simulate the http object with the fields for
    /// method, headers, query params, and the url fragment
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "params", alias = "parameter", alias = "parameters")]
    pub param: Option<serde_json::Value>,
    /// Optional endpoint to send otlp http logs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otel_endpoint: Option<deno_core::url::Url>,
    /// Optional headers to include when exporting otlp
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub otel_headers: HashMap<String, String>,
    /// Additional global tags to include with exported data
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub otel_tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

impl From<Origin> for ApiOrigin {
    #[inline(always)]
    fn from(val: Origin) -> Self {
        match val {
            Origin::Ipfs => ApiOrigin::IPFS,
            Origin::Http => ApiOrigin::HTTP,
            _ => unreachable!(),
        }
    }
}
