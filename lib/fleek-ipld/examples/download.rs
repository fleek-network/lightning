use fleek_ipld::dag_pb::IpldDagPbProcessor;
use fleek_ipld::processor::{IpldItem, IpldStream};
use ipld_core::cid::Cid;
use tokio_stream::StreamExt as _;

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let processor = IpldDagPbProcessor::new("http://ipfs.io");
    let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D".try_into()?;
    let doc_id = cid.into();
    let mut stream = IpldStream::new(processor, doc_id).fuse();
    while let Some(item) = stream.next().await {
        match item {
            Ok(IpldItem::File(file)) => {
                println!("File data: {:?}", file);
            },
            Ok(IpldItem::Dir(dir)) => {
                println!("Dir data: {:?}", dir);
            },
            Err(e) => {
                panic!("Error: {:?}", e);
            },
        }
    }
    Ok(())
}
