use fleek_ipld::walker::dag_pb::IpldDagPbProcessor;
use fleek_ipld::walker::processor::IpldStream;
use ipld_core::cid::Cid;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".try_into()?; // all
    let cid: Cid = "Qmb4KDzrnDHdHcH1UUTF3jTC3RhPJ6UyZ2wB8fNPnwiP5R".try_into()?; // all
    let cid: Cid = "QmVtxWYYaGKVrfiwSGn8s1ifJi4nvxwC7HE14ZbMtC4DeM".try_into()?; // all
    //let cid: Cid = "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi".try_into()?; // jpg big file
    //let cid: Cid = "Qmej4L6L4UYxHF4s4QeAzkwUX8VZ45GiuZ2BLtVds5LXad".try_into()?; // css file
    //let cid: Cid = "QmbvrHYWXAU1BuxMPNRtfeF4DS2oPmo5hat7ocqAkNPr74".try_into()?; // png small

    let processor = IpldDagPbProcessor::new("https://ipfs.io");
    let mut stream = IpldStream::new(processor, cid.into());

    while let Some(item) = stream.next().await {
        let item = item?;
        println!("Item: {:?}", item);
    }

    Ok(())
}
