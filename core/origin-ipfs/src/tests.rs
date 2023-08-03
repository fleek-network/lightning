use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use lightning_interfaces::{OriginProviderInterface, WithStartAndShutdown};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use crate::{config::Config, IPFSOrigin};

#[tokio::test]
async fn test_origin() {
    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let ipfs_origin = IPFSOrigin::init(Config::default()).await.unwrap();
    ipfs_origin.start().await;

    let socket = ipfs_origin.get_socket();
    let stream = socket.run(req_cid.to_bytes()).await.unwrap().unwrap();

    let mut read = StreamReader::new(stream);
    let mut buf = [0; 1024];
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let len = read.read(&mut buf).await.unwrap();
        bytes.extend(&buf[..len]);
        if len == 0 {
            break;
        }
    }
    assert!(
        Code::try_from(req_cid.hash().code())
            .ok()
            .map(|code| &code.digest(&bytes) == req_cid.hash())
            .unwrap()
    );
}

#[tokio::test]
async fn test_shutdown() {
    let ipfs_origin = IPFSOrigin::init(Config::default()).await.unwrap();
    assert!(!ipfs_origin.is_running());
    ipfs_origin.start().await;
    assert!(ipfs_origin.is_running());
    ipfs_origin.shutdown().await;
    assert!(!ipfs_origin.is_running());
}
