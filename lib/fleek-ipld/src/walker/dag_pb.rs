use std::pin::Pin;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::{Future, FutureExt};
use ipld_core::cid::Cid;
use ipld_core::codec::Codec;
use ipld_core::ipld::Ipld;
use ipld_dagpb::{DagPbCodec, PbNode};
use reqwest::Url;

use super::errors::IpldError;
use super::processor::{DocId, IpldItem, Processor};

#[derive(Clone)]
pub struct IpldDagPbProcessor {
    ipfs_url: Url,
}

impl IpldDagPbProcessor {
    pub fn new(ipfs_url: &str) -> Self {
        Self {
            ipfs_url: Url::parse(ipfs_url)
                .unwrap_or_else(|_| panic!("Invalid IPFS URL {}", ipfs_url)),
        }
    }

    pub async fn request(&self, cid: &Cid) -> Result<Bytes, IpldError> {
        let url = self.ipfs_url.clone();
        let url = url.join(&format!("ipfs/{}/?format=raw", cid))?;
        let response = reqwest::get(url).await?;
        response.bytes().await.map_err(Into::into)
    }
}

#[async_trait]
impl Processor for IpldDagPbProcessor {
    async fn get(&self, doc_id: DocId) -> Result<Option<IpldItem>, IpldError> {
        let bytes = self.request(&doc_id.cid()).await?;
        let node: PbNode = DagPbCodec::decode_from_slice(&bytes)?;
        let ipld = node.clone().into();
        if let Ipld::Map(map) = ipld {
            if let Some(Ipld::Bytes(ty)) = map.get("Data") {
                if *ty == [8, 1] {
                    let item = IpldItem::from_dir(doc_id, node);
                    return Ok(Some(item));
                } else if node.links.is_empty() {
                    let item = IpldItem::from_file(doc_id, node.data);
                    return Ok(Some(item));
                } else {
                    let data: Bytes = self.get_file_link_data(&doc_id, node).await?;
                    let item = IpldItem::from_file(doc_id, Some(data));
                    return Ok(Some(item));
                }
            } else {
                Err(IpldError::CannotDecodeDagPbData(
                    "Ipld object does not contain a Data field to identify type".to_string(),
                ))
            }
        } else {
            Err(IpldError::CannotDecodeDagPbData(format!(
                "Ipld object does not contain a map {:?}",
                ipld,
            )))
        }
    }
}

impl IpldDagPbProcessor {
    async fn get_file_link_data(&self, cid: &DocId, node: PbNode) -> Result<Bytes, IpldError> {
        let mut buf = BytesMut::with_capacity(cid.size().unwrap_or(1024 * 10) as usize);
        for link in &node.links {
            let bytes = self.request(&link.cid).await?;
            let node: PbNode = DagPbCodec::decode_from_slice(&bytes)?;
            if node.links.is_empty() {
                buf.extend_from_slice(&node.data.unwrap_or_default());
            } else {
                let data = Box::pin(self.get_file_link_data(cid, node));
                let data = data.await?;
                buf.extend_from_slice(&data);
            }
        }
        Ok(buf.freeze())
    }
}