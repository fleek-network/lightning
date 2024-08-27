#![allow(dependency_on_unit_never_type_fallback)]

use ipld_core::cid::Cid;
use ipld_core::codec::Codec;
use ipld_dagpb::{DagPbCodec, PbNode};

pub async fn download() -> anyhow::Result<()> {
    // Replace with the CID of the IPLD file
    let cid_str = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D";
    //let cid_str = "bafybeiaysi4s6lnjev27ln5icwm6tueaw2vdykrtjkwiphwekaywqhcjze";
    let cid = Cid::try_from(cid_str).expect("Invalid CID");

    // Construct the URL to fetch the IPLD data
    let url = format!("https://ipfs.io/ipfs/{}/?format=raw", cid);

    println!("URL: {}", url);

    // Download the file
    let response = reqwest::get(&url).await?;
    let bytes = response.bytes().await?;

    // Decode the CBOR-encoded data
    let data: PbNode = DagPbCodec::decode_from_slice(&bytes).expect("Failed to decode PB data");

    println!("Data: {:?}", data);

    Ok(())
}
