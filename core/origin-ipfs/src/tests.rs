use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use lightning_interfaces::{OriginProviderInterface, UntrustedStream, WithStartAndShutdown};
use lightning_test_utils::ipfs_gateway::spawn_gateway;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use crate::{
    config::{Config, Gateway, Protocol},
    IPFSOrigin,
};

#[tokio::test]
async fn test_origin() {
    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let mut config = Config::default();

    let req_fut = async move {
        config.gateways.push(Gateway {
            protocol: Protocol::Http,
            authority: "127.0.0.1:30100".to_string(),
        });
        let ipfs_origin = IPFSOrigin::init(config).await.unwrap();
        ipfs_origin.start().await;

        let socket = ipfs_origin.get_socket();
        let stream = socket.run(req_cid.to_bytes()).await.unwrap().unwrap();

        assert_eq!(stream.was_content_valid(), None);

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
    };

    tokio::select! {
        _ = spawn_gateway(30100) => {

        }
        _ = req_fut => {

        }

    }
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
