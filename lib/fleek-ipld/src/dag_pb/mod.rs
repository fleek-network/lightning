use async_trait::async_trait;
use ipld_core::codec::Codec;
use ipld_core::ipld::Ipld;
use ipld_dagpb::{DagPbCodec, PbNode};
use reqwest::Url;

use crate::processor::errors::IpldError;
use crate::processor::{DocDir, DocFile, DocId, IpldItem, Processor};

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
    async fn get(&self, cid: DocId) -> Result<Option<IpldItem>, IpldError> {
        let url = self.ipfs_url.clone();
        let url = url.join(&format!("ipfs/{}/?format=raw", *cid.cid()))?;
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        let node: PbNode = DagPbCodec::decode_from_slice(&bytes)?;
        let ipld = node.clone().into();
        println!("Ipld: {:?}", ipld);
        if let Ipld::Map(map) = ipld {
            if let Some(Ipld::Bytes(ty)) = map.get("Data") {
                println!("Data: {:?}", ty);
                if *ty == [8, 1] {
                    let data = DocDir::from_pb_node(cid, node).into();
                    return Ok(Some(data));
                } else {
                    let data = DocFile::from_pb_node(cid, node);
                    return Ok(Some(data.into()));
                }
            }
        }
        let data = Some(DocDir::from_pb_node(cid, node).into());
        println!("data: {:?}", data);
        Ok(data)
    }
}
