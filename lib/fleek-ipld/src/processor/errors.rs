use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpldError {
    #[error("IPLD error: {0}")]
    CidParsingError(#[from] ipld_core::cid::Error),

    #[error("IPLD error: {0}")]
    DagPbError(#[from] ipld_dagpb::Error),
}
