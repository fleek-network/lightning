use fleek_ipld::decoder::reader::IpldReader;
use fleek_ipld::errors::IpldError;
use fleek_ipld::walker::stream::{IpldItemProcessor, IpldStream, Item, ReqwestDownloader};
use ipld_core::cid::Cid;

#[derive(Clone)]
struct PrintProcessor;

#[async_trait::async_trait]
impl IpldItemProcessor for PrintProcessor {
    async fn on_item(&self, item: &Item) -> Result<(), IpldError> {
        println!("Item: {:?}", item);
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
    let stream = IpldStream::new(reader, downloader, processor);

    stream.download(cid).await.map_err(Into::into)
}
