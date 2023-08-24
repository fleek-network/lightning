use fleek_crypto::ClientPublicKey;
use serde::{Deserialize, Serialize};

use crate::CompressionAlgoSet;

/// The metadata set on a handshake connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConnectionMetadata {
    pub lane: u8,
    pub client: ClientPublicKey,
    pub compression_set: CompressionAlgoSet,
}
