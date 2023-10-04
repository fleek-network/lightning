use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

/// An immutable pointer is used as a general address to a content living off Fleek Network.
///
/// It is important that the underlying origins guarantee that the same `uri` is forever linked
/// to the same content. However we do not really care about the underlying source for the
/// persistence, the only guarantee which we build upon is that an origin provider once fed the
/// same path *WILL ALWAYS* return the same content (or explicit errors).
#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
pub struct ImmutablePointer {
    /// The underlying source of the data.
    pub origin: OriginProvider,
    /// The serialized path for the content within the respective origin.
    pub uri: Vec<u8>,
}

/// This enum represents the origins which we support in the protocol. More can be added as we
/// support more origins in future.
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize, IsVariant,
)]
#[non_exhaustive]
pub enum OriginProvider {
    IPFS,
}

impl ToString for OriginProvider {
    fn to_string(&self) -> String {
        match self {
            OriginProvider::IPFS => String::from("ipfs"),
        }
    }
}
