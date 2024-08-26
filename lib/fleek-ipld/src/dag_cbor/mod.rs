#![allow(dependency_on_unit_never_type_fallback)]
use core::fmt;

use libipld::cbor::DagCborCodec;
use libipld::prelude::*;
use libipld::{Cid, DagCbor, Ipld};

#[derive(DagCbor, Clone, PartialEq)]
pub struct File {
    //pub cid: Cid,
    pub cats: String,
    pub cheese: Vec<Ipld>,
    pub something: Ipld,
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data: {:?}", self.cheese)
    }
}

pub async fn download() -> anyhow::Result<()> {
    // Replace with the CID of the IPLD file
    let cid_str = "bafyreicnokmhmrnlp2wjhyk2haep4tqxiptwfrp2rrs7rzq7uk766chqvq";
    let cid = Cid::try_from(cid_str).expect("Invalid CID");

    // Construct the URL to fetch the IPLD data
    let url = format!("https://ipfs.io/ipfs/{}", cid);

    // Download the file
    let response = reqwest::get(&url).await?;
    let bytes = response.bytes().await?;

    // Decode the CBOR-encoded data
    let data: File = DagCborCodec
        .decode(&bytes)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Data: {}", data);

    Ok(())
}
