use anyhow::Result;

use crate::{Blake3Hash, ImmutablePointer};

#[derive(Clone, Debug)]
pub enum FetcherRequest {
    Put { pointer: ImmutablePointer },
    Fetch { hash: Blake3Hash },
}

#[derive(Debug)]
pub enum FetcherResponse {
    Put(Result<Blake3Hash>),
    Fetch(Result<()>),
}
