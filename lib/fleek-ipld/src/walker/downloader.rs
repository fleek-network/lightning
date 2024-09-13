//! This module provides the `Downloader` trait and an implementation using `reqwest`.
use std::ops::Deref;
use std::pin::Pin;

use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use ipld_core::cid::Cid;
use url::Url;

use crate::errors::IpldError;

/// Response complex type for the downloader.
pub type Response = Pin<Box<dyn Stream<Item = Result<Bytes, IpldError>> + Send + Sync + 'static>>;

/// The `Downloader` trait defines the interface for downloading data from IPFS.
///
/// **Note**: `async_trait` is not used here because macro is not able to generate the correct code
/// with the complex return type.
pub trait Downloader {
    fn download(
        &self,
        cid: &Cid,
    ) -> impl std::future::Future<Output = Result<Response, IpldError>> + Send;
}

/// The `ReqwestDownloader` is an implementation of the `Downloader` trait that uses the `reqwest`
#[derive(Clone)]
pub struct ReqwestDownloader(Url);

impl Deref for ReqwestDownloader {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ReqwestDownloader {
    pub fn new(url: &str) -> Self {
        let ipfs_url = Url::parse(url).unwrap_or_else(|_| panic!("Invalid IPFS URL {}", url));
        ReqwestDownloader(ipfs_url)
    }
}

impl Downloader for ReqwestDownloader {
    async fn download(&self, cid: &Cid) -> Result<Response, IpldError> {
        let url = self.join(&format!("ipfs/{}?format=raw", cid))?;
        let response = reqwest::get(url).await?;
        if !response.status().is_success() {
            return Err(IpldError::HttpError(response.status()));
        }
        let stream = response.bytes_stream().map_err(Into::into);
        Ok(Box::pin(stream))
    }
}
