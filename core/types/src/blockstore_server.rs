use crate::{Blake3Hash, NodeIndex, RejectReason};

#[derive(Clone, Debug)]
pub struct ServerRequest {
    pub hash: Blake3Hash,
    pub peer: NodeIndex,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("Failed to fetch data from other peers")]
pub enum PeerRequestError {
    Timeout,
    Rejected(RejectReason),
    Incomplete,
}
