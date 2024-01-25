use anyhow::{anyhow, Result};
use bytes::Bytes;
use cid::Cid;
use fleek_ipld::unixfs::Data;
use libipld::pb::PbNode;

pub fn decode_block(cid: Cid, data: Vec<u8>) -> Result<Option<Bytes>> {
    match cid.codec() {
        0x70 => {
            let node = PbNode::from_bytes(data.into())?;
            let Some(data) = node.data else {
                return Ok(None);
            };
            // Optimistically try to decode the data as unixfs
            match Data::try_from(data.to_vec().as_ref()) {
                Ok(unixfs) => Ok(Some(unixfs.Data.to_vec().into())),
                Err(_) => Ok(Some(data)),
            }
        },
        codec => Err(anyhow!("Unsupported codec found in CID: {codec}")),
    }
}
