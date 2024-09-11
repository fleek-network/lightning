use ipld_core::cid::{multihash, Cid};
use thiserror::Error;

use crate::walker::concurrent::controlled::StreamState;

/// Error type for IPLD operations
#[derive(Debug, Error)]
pub enum IpldError {
    #[error("IPLD error: Error parsing Cid {0}")]
    CidParsingError(#[from] ipld_core::cid::Error),

    #[error("IPLD error: Error decoding IPLD format - Cid {0} - {1}")]
    IpldCodecError(Cid, String),

    #[error("IPLD error: Error Decoding DAG-PB data {0}")]
    DagPbError(#[from] ipld_dagpb::Error),

    #[error("IPLD error: UnixFS error {0}")]
    UnixFsProtobufError(#[from] quick_protobuf::Error),

    #[error("IPLD error: Error decoding UnixFS data - Cid {0}")]
    UnixFsDecodingError(String),

    #[error("IPLD error: Error fetching data from IPFS - Cid {0}")]
    IpfsDataError(Cid),

    #[error("IPLD error: Cannot decode DAG-PB data - Cid {0}")]
    CannotDecodeDagPbData(Cid),

    #[error("IPLD error: Error fetching data from IPFS {0}")]
    IpfsError(#[from] reqwest::Error),

    #[error("IPLD error: Error IPFS URL {0}")]
    IpfsUrlError(#[from] url::ParseError),

    #[error("IPLD error: Error processing UnixFS - Cid {0}")]
    UnsupportedUnixFsDataType(Cid),

    #[error("IPLD error: Error traversing stream {0}")]
    TraverseError(String),

    #[error("IPLD error: Missing link")]
    MissingLink,

    #[error("IPLD error: Stream buffer empty")]
    StreamBufferEmpty,

    #[error("IPLD error: Error validating cid {0}")]
    IpldMultihashError(#[from] multihash::Error),

    #[error("IPLD error: Error processing item {0}")]
    ParallelIpldItemProcessingError(#[from] tokio::task::JoinError),

    #[error("IPLD error: Error processing item {0}")]
    StreamError(#[from] tokio::sync::mpsc::error::SendError<StreamState>),

    #[error("IPLD error: Error downloading item {0}")]
    HttpError(reqwest::StatusCode),
}
