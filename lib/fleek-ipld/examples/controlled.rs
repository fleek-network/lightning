use fleek_ipld::decoder::reader::IpldReader;
use fleek_ipld::errors::IpldError;
use fleek_ipld::walker::controlled::{ControlledIpldStream, StreamState};
use fleek_ipld::walker::stream::{IpldItemProcessor, Item, ReqwestDownloader};
use ipld_core::cid::Cid;

#[derive(Clone)]
struct PrintProcessor;

#[async_trait::async_trait]
impl IpldItemProcessor for PrintProcessor {
    async fn on_item(&self, item: &Item) -> Result<(), IpldError> {
        println!("Item: {:?}", item);
        let cid = "QmTPYQ2T8ten7RRN7pzxuty3ujbc8p2o242nQEfPQQ2jWA";
        if item.is_cid(cid) {
            println!("Found the file we were looking for!");
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".try_into()?; // all
    //let cid: Cid = "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi".try_into()?; // jpg big file
    //let cid: Cid = "Qmej4L6L4UYxHF4s4QeAzkwUX8VZ45GiuZ2BLtVds5LXad".try_into()?; // css file
    //let cid: Cid = "QmbvrHYWXAU1BuxMPNRtfeF4DS2oPmo5hat7ocqAkNPr74".try_into()?; // png small

    let processor = PrintProcessor;
    let downloader = ReqwestDownloader::new("https://ipfs.io");
    let reader = IpldReader::default();
    let mut stream = ControlledIpldStream::new(reader, downloader, processor);
    let control = stream.control();

    stream.download(cid).await?;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    control.send(StreamState::Stopped).await?;
    Ok(())
}
