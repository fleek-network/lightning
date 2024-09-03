use fleek_ipld::walker::dag_pb::IpldDagPbProcessor;
use fleek_ipld::walker::IpldItem;
use ipld_core::cid::Cid;
use tokio_stream::StreamExt as _;

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".try_into()?; // all
    //let cid: Cid = "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi".try_into()?; // jpg big file
    //let cid: Cid = "Qmej4L6L4UYxHF4s4QeAzkwUX8VZ45GiuZ2BLtVds5LXad".try_into()?; // css file
    //let cid: Cid = "QmbvrHYWXAU1BuxMPNRtfeF4DS2oPmo5hat7ocqAkNPr74".try_into()?; // png small
    let mut stream = IpldDagPbProcessor::new("https://ipfs.io")
        .stream(cid)
        .fuse();
    while let Some(item) = stream.next().await {
        match item {
            Ok(IpldItem::File(file)) => {
                println!("File data: {:?} \n", file);
                if file.id().link().cid().to_string()
                    == "QmQSuagGCLjLaoSGcHGXEyDdhLvazB6EEtpUYxZKmSTHVZ"
                {
                    break;
                }
            },
            Ok(IpldItem::Dir(dir)) => {
                println!("Dir data: {:?} \n", dir);
            },
            Err(e) => {
                panic!("Error: {:?}", e);
            },
        }
    }
    Ok(())
}
