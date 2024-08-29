use fleek_ipld::walker::dag_pb::IpldDagPbProcessor;
use fleek_ipld::walker::processor::{IpldItem, IpldStream};
use ipld_core::cid::Cid;
use tokio_stream::StreamExt as _;

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let processor = IpldDagPbProcessor::new("http://ipfs.io");
    let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".try_into()?;
    let doc_id = cid.into();
    let mut stream = IpldStream::new(processor, doc_id).fuse();
    let mut count_files = 0;
    let mut count_dirs = 0;
    while let Some(item) = stream.next().await {
        match item {
            Ok(IpldItem::File(file)) => {
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
