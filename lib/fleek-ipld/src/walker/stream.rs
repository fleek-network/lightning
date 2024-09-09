use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use ipld_core::cid::Cid;
use url::Url;

use crate::decoder::fs::IpldItem;
use crate::errors::IpldError;

#[async_trait]
pub trait IpldItemProcessor {
    async fn on_item(&mut self, item: IpldItem) -> Result<(), IpldError>;
}

pub type Response = Pin<Box<dyn Stream<Item = Result<Bytes, IpldError>> + Send + Sync + 'static>>;

pub trait Downloader {
    fn download(
        &self,
        cid: &Cid,
    ) -> impl std::future::Future<Output = Result<Response, IpldError>> + Send;
}

pub struct ReqwestDownloader {
    url: Url,
}

impl ReqwestDownloader {
    pub fn new(url: &str) -> Self {
        let ipfs_url = Url::parse(url).unwrap_or_else(|_| panic!("Invalid IPFS URL {}", url));
        ReqwestDownloader { url: ipfs_url }
    }
}

impl Downloader for ReqwestDownloader {
    async fn download(&self, cid: &Cid) -> Result<Response, IpldError> {
        let url = self.url.join(&cid.to_string())?;
        let response = reqwest::get(url).await?;
        let stream = response.bytes_stream().map_err(Into::into);
        Ok(Box::pin(stream))
    }
}

pub struct IpldStream<'a, R> {
    processor: &'a mut R,
}
