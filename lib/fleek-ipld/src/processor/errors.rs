use thiserror::Error;

/// Error type for IPLD operations
#[derive(Debug, Error)]
pub enum IpldError {
    #[error("IPLD error: Error parsing Cid {0}")]
    CidParsingError(#[from] ipld_core::cid::Error),

    #[error("IPLD error: Error Decoding DAG-PB data {0}")]
    DagPbError(#[from] ipld_dagpb::Error),

    #[error("IPLD error: Error fetching data from IPFS {0}")]
    IpfsError(#[from] reqwest::Error),

    #[error("IPLD error: Error IPFS URL {0}")]
    IpfsUrlError(#[from] url::ParseError),

    #[error("IPLD error: Error traversing stream {0}")]
    TraverseError(String),
}
