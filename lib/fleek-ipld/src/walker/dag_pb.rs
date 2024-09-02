use std::ops::Deref;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use ipld_core::cid::Cid;
use ipld_core::ipld::Ipld;
use ipld_dagpb::PbNode;
use reqwest::Url;

use super::processor::{DocId, IpldItem, IpldStream, Link, Processor};
use crate::errors::IpldError;
use crate::unixfs::Data;

/// Processor for DAG-PB nodes in IPFS with UnixFS data
#[derive(Clone)]
pub struct IpldDagPbProcessor {
    ipfs_url: Url,
}

struct PbNodeWrapper(PbNode);

impl From<PbNode> for PbNodeWrapper {
    fn from(node: PbNode) -> Self {
        Self(node)
    }
}

impl From<PbNodeWrapper> for Vec<Link> {
    fn from(node: PbNodeWrapper) -> Self {
        node.0
            .links
            .into_iter()
            .map(|x| Link::new(x.cid, x.name, x.size))
            .collect()
    }
}

impl Deref for PbNodeWrapper {
    type Target = PbNode;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PbNodeWrapper {
    pub fn from_bytes(bytes: &Bytes) -> Result<Self, IpldError> {
        let node = PbNode::from_bytes(bytes.clone())?;
        Ok(node.into())
    }
}

impl IpldDagPbProcessor {
    pub fn new(ipfs_url: &str) -> Self {
        Self {
            ipfs_url: Url::parse(ipfs_url)
                .unwrap_or_else(|_| panic!("Invalid IPFS URL {}", ipfs_url)),
        }
    }

    async fn request(&self, cid: &Cid) -> Result<Bytes, IpldError> {
        let url = self.ipfs_url.clone();
        let url = url.join(&format!("ipfs/{}/?format=raw", cid))?;
        let response = reqwest::get(url).await?;
        response.bytes().await.map_err(Into::into)
    }

    pub fn stream(self, cid: impl Into<Cid>) -> IpldStream<Self> {
        IpldStream::new(self, cid.into().into())
    }
}

#[async_trait]
impl Processor for IpldDagPbProcessor {
    async fn get(&self, doc_id: DocId) -> Result<Option<IpldItem>, IpldError> {
        let bytes = self.request(doc_id.cid()).await?;
        let node = PbNodeWrapper::from_bytes(&bytes)?;
        let ipld = node.clone().into();
        if let Ipld::Map(map) = ipld {
            if let Some(Ipld::Bytes(ty)) = map.get("Data") {
                if *ty == [8, 1] {
                    let item = IpldItem::from_dir(doc_id, node.into());
                    return Ok(Some(item));
                } else if node.links.is_empty() {
                    let data = Data::try_from(&node.data)?;
                    let item =
                        IpldItem::from_file(doc_id, Some(Bytes::copy_from_slice(&data.Data)));
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
    async fn get_file_link_data(
        &self,
        cid: &DocId,
        node: PbNodeWrapper,
    ) -> Result<Bytes, IpldError> {
        let mut buf = BytesMut::new();
        for link in &node.links {
            let bytes = self.request(&link.cid).await?;
            let node = PbNodeWrapper::from_bytes(&bytes)?;
            if node.links.is_empty() {
                let data = Data::try_from(&node.data)?;
                buf.extend_from_slice(&data.Data);
            } else {
                let data = Box::pin(self.get_file_link_data(cid, node));
                let data = data.await?;
                buf.extend_from_slice(&data);
            }
        }
        Ok(buf.freeze())
    }
}
