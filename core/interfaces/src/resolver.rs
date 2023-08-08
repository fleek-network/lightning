use async_trait::async_trait;
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature};

use crate::Blake3Hash;

pub struct ImmutablePointer {
    pub origin: OriginProvider,
}

pub enum OriginProvider {
    IPFS,
}

pub struct ResolvedHashRecord {
    pub hash: Blake3Hash,
    pub pointer: ImmutablePointer,
    pub originator: NodeNetworkingPublicKey,
    pub signature: NodeNetworkingSignature,
}

/// The resolver is responsible to resolve an FNIP (Fleek Network Immutable Pointer),
/// into a Blake3 hash of the content.
#[async_trait]
pub trait ResolverInterface {
    fn publish(&self, hash: Blake3Hash, pointers: &[ImmutablePointer]);

    /// Try to find and return
    async fn resolve_hash(&self, pointer: ImmutablePointer) -> Option<ResolvedHashRecord>;
}
