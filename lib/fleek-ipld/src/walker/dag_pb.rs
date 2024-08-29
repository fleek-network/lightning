use async_trait::async_trait;
use ipld_core::codec::Codec;
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
}

#[async_trait]
impl Processor for IpldDagPbProcessor {
    async fn get(&self, cid: DocId) -> Result<Option<IpldItem>, IpldError> {
        let url = self.ipfs_url.clone();
        let url = url.join(&format!("ipfs/{}/?format=raw", *cid.cid()))?;
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        let node: PbNode = DagPbCodec::decode_from_slice(&bytes)?;
        Ok(Some(IpldItem::from_pb_node(cid, node)))
        //let ipld = node.clone().into();
        //if let Ipld::Map(map) = ipld {
        //    if let Some(Ipld::Bytes(ty)) = map.get("Data") {
        //        if *ty == [8, 1] {
        //            Ok(Some(DocDir::from_pb_node(cid, node).into()))
        //        } else {
        //            Ok(Some(DocFile::from_pb_node(cid, node)))
        //        }
        //    } else {
        //        Err(IpldError::CannotDecodeDagPbData(
        //            "Ipld object does not contain a Data field to identify type".to_string(),
        //        ))
        //    }
        //} else {
        //    Err(IpldError::CannotDecodeDagPbData(format(
        //        "Ipld object does not contain a map {}",
        //        ipld,
        //    )))
        //}
    }
}
