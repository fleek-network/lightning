use async_trait::async_trait;
use ipld_core::codec::Codec;
use ipld_dagpb::{DagPbCodec, PbNode};
use reqwest::Url;

use crate::processor::errors::IpldError;
use crate::processor::{DocDir, DocId, IpldItem, Processor};

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
}

#[async_trait]
impl Processor for IpldDagPbProcessor {
    type Codec = DagPbCodec;

    async fn get(&self, cid: DocId) -> Result<Option<IpldItem>, IpldError> {
        let url = self.ipfs_url.clone();
        let url = url.join(&format!("ipfs/{}/?format=raw", *cid))?;
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        let data = DagPbCodec::decode_from_slice(&bytes)
            .map(|node: PbNode| Some(DocDir::from_pb_node(cid, node).into()))?;
        Ok(data)
    }
}
