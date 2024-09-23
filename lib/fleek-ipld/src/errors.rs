use ipld_core::cid::Cid;
use thiserror::Error;

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

    #[error("IPLD error: Error validating hash - Cid {0}")]
    MultihashError(Cid),

    #[error("IPLD error: UnssuportedCode Multihash code: {0}")]
    MultihashCodeError(u64),

    #[error("IPLD error: Error processing item {0}")]
    ParallelIpldItemProcessingError(#[from] tokio::task::JoinError),

    #[error("IPLD error: Error downloading item {0}")]
    HttpError(reqwest::StatusCode),

    #[error("IPLD error: Error sending signal to controlled process {0}")]
    ControlError(String),

    #[error("IPLD error: Error trying to convert to Chunk {1} - Cid {0}")]
    InvalidChunk(Cid, usize),

    #[error("IPLD error: Error trying to convert to Dir - Cid {0}")]
    InvalidDir(Cid),

    #[error("IPLD error: Error downloading - {0}")]
    DownloaderError(String),

    #[error("IPLD error: Unsupported codec - Cid {0}")]
    UnsupportedCodec(Cid),
}
