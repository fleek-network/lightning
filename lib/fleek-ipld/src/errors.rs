use thiserror::Error;

/// Error type for IPLD operations
#[derive(Debug, Error)]
pub enum IpldError {
    #[error("IPLD error: Error parsing Cid {0}")]
    CidParsingError(#[from] ipld_core::cid::Error),

    #[error("IPLD error: Error Decoding DAG-PB data {0}")]
    DagPbError(#[from] ipld_dagpb::Error),

    #[error("IPLD error: UnixFS error {0}")]
    UnixFsProtobufError(#[from] quick_protobuf::Error),

    #[error("IPLD error: Error decoding UnixFS data {0}")]
    UnixFsDecodingError(String),

    #[error("IPLD error: Error fetching data from IPFS {0}")]
    IpfsDataError(String),

    #[error("IPLD error: Cannot decode DAG-PB data {0}")]
    CannotDecodeDagPbData(String),

    #[error("IPLD error: Error fetching data from IPFS {0}")]
    IpfsError(#[from] reqwest::Error),

    #[error("IPLD error: Error IPFS URL {0}")]
    IpfsUrlError(#[from] url::ParseError),

    #[error("IPLD error: Error traversing stream {0}")]
    TraverseError(String),

    #[error("IPLD error: Missing link")]
    MissingLink,
}
