use derive_more::IsVariant;
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature};
use serde::{Deserialize, Serialize};

use crate::Blake3Hash;

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

/// Once a content is put on the network (i.e a node fetches the content from the origin), the
/// node that fetched the content computes the blake3 hash of the content and signs a record
/// attesting that it witnessed the immutable pointer resolving to the said blake3 hash.
pub struct ResolvedImmutablePointerRecord {
    /// The immutable pointer that was fetched.
    pub pointer: ImmutablePointer,
    /// The blake3 hash of the content. Used to store the content on the blockstore.
    pub hash: Blake3Hash,
    /// The public key of the node which fetched and attested to this content.
    // TODO: Replace with node id later.
    pub originator: NodeNetworkingPublicKey,
    /// The signature of the node.
    pub signature: NodeNetworkingSignature,
}
