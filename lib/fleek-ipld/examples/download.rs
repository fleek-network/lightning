use fleek_ipld::walker::dag_pb::IpldDagPbProcessor;
use fleek_ipld::walker::processor::{IpldItem, IpldStream};
use ipld_core::cid::Cid;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt as _;

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let processor = IpldDagPbProcessor::new("http://ipfs.io");
    //let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".try_into()?; // all
    let cid: Cid = "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi".try_into()?; // jpg big file
    //let cid: Cid = "Qmej4L6L4UYxHF4s4QeAzkwUX8VZ45GiuZ2BLtVds5LXad".try_into()?; // css file
    //let cid: Cid = "QmbvrHYWXAU1BuxMPNRtfeF4DS2oPmo5hat7ocqAkNPr74".try_into()?; // png small
    let doc_id = cid.into();
    let mut stream = IpldStream::new(processor, doc_id).fuse();
    let mut count_files = 0;
    let mut count_dirs = 0;
    while let Some(item) = stream.next().await {
        match item {
            Ok(IpldItem::File(file)) => {
                let file_name = file
                    .id()
                    .link()
                    .name()
                    .clone()
                    .unwrap_or("unknown_file.jpg".to_string());
                let mut f = tokio::fs::File::create(file_name).await?;
                f.write_all(file.data()).await?;
                println!("File data: {:?} \n", file);
                count_files += 1;
            },
            Ok(IpldItem::Dir(dir)) => {
                println!("Dir data: {:?} \n", dir);
                count_dirs += 1;
            },
            Err(e) => {
                panic!("Error: {:?}", e);
            },
        }
    }
    println!("Files: {}, Dirs: {}", count_files, count_dirs);
    Ok(())
}
