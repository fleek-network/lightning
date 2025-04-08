use fleek_ipld::decoder::fs::IpldItem;
use fleek_ipld::decoder::reader::IpldReader;
use fleek_ipld::walker::downloader::ReqwestDownloader;
use fleek_ipld::walker::stream::IpldStream;
use ipld_core::cid::Cid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".parse()?;
    //let cid: Cid = "QmeqQyim8DtFfse3XNaVE2AbH4gY5uauSndiAJcArVHnzA".try_into()?; // all
    //let cid: Cid = "Qmb4KDzrnDHdHcH1UUTF3jTC3RhPJ6UyZ2wB8fNPnwiP5R".try_into()?; // all
    //let cid: Cid = "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi".try_into()?; // jpg big file
    //let cid: Cid = "Qmej4L6L4UYxHF4s4QeAzkwUX8VZ45GiuZ2BLtVds5LXad".try_into()?; // css file
    //let cid: Cid = "QmbvrHYWXAU1BuxMPNRtfeF4DS2oPmo5hat7ocqAkNPr74".try_into()?; // png small

    let mut stream = IpldStream::builder()
        .reader(IpldReader::default())
        .downloader(ReqwestDownloader::new("https://ipfs.io"))
        .build();

    stream.start(cid).await;

    loop {
        let item = stream.next().await?;
        match item {
            Some((IpldItem::ChunkedFile(chunk), _)) => {
                let mut stream_file = stream.new_chunk_file_streamer(chunk).await;
                while let Some(chunk) = stream_file.next_chunk().await? {
                    println!("Chunk: {:?} \n\n", chunk);
                }
            },
            Some((IpldItem::File(file), _)) => {
                println!("File: {:?} \n\n", file);
            },
            Some((IpldItem::Dir(dir), _)) => {
                println!("Directory: {:?} \n\n", dir);
            },
            Some((IpldItem::Chunk(_), _)) => {
                panic!("Chunked file should be handled by ChunkedFile");
            },
            None => break,
        }
    }

    Ok(())
}
