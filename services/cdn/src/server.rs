use anyhow::{anyhow, Result};
use blake3_tree::ProofBuf;
use draco_handshake::server::RawLaneConnection;
use draco_interfaces::{ConnectionInterface, FileSystemInterface, HandlerFn, SdkInterface};
use futures::FutureExt;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::connection::{
    consts::{DELIVERY_ACK_TAG, RANGE_REQUEST_TAG, REQUEST_TAG},
    CdnConnection, CdnFrame, ServiceMode,
};

pub fn setup_cdn_service<
    'a,
    R: AsyncRead + Send + Sync + Unpin + 'static,
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    SDK: SdkInterface<Connection = RawLaneConnection<R, W>>,
>(
    _sdk: SDK,
) -> HandlerFn<'a, SDK> {
    // TODO: What do we need to do when the node starts up?
    //         - Check if node has commited to the service via app query
    //         - If not, submit a tx committing to the service
    |s, c| {
        async {
            if let Err(e) = handle_session(s, c).await {
                eprintln!("Session error: {e}")
            }
        }
        .boxed()
    }
}

pub async fn handle_session<
    R: AsyncRead + Send + Sync + Unpin,
    W: AsyncWrite + Send + Sync + Unpin,
    SDK: SdkInterface<Connection = RawLaneConnection<R, W>>,
>(
    sdk: SDK,
    mut conn: SDK::Connection,
) -> Result<()> {
    let fs = sdk.get_fs();
    let compression = conn.get_compression_set();
    let (w, r) = conn.split();
    let mut conn = CdnConnection::new(r, w);

    // recieve client requests until the connection ends
    while let Some(frame) = conn
        .read_frame(Some(REQUEST_TAG | RANGE_REQUEST_TAG))
        .await
        .expect("unable to read frame")
    {
        match frame {
            CdnFrame::Request { service_mode, hash } => {
                let tree = match fs.get_tree(&hash).await {
                    Some(t) => t.to_owned(),
                    None => todo!("handle proof not found"),
                };
                let mut block_counter = 0u32;
                let mut tree_idx = 0;

                match service_mode {
                    ServiceMode::Tentative => todo!(),
                    ServiceMode::Optimistic => {
                        let mut proof = ProofBuf::new(&tree.0, 0);
                        let mut proof_len = proof.len() as u64;

                        while let Some(hash) = tree.0.get(tree_idx) {
                            let chunk = fs
                                .get(block_counter, hash, compression)
                                .await
                                .expect("failed to get block from store");

                            if block_counter > 0 {
                                proof = ProofBuf::resume(&tree.0, block_counter as usize);
                                proof_len = proof.len() as u64;
                            }

                            // send res frame
                            conn.write_frame(CdnFrame::ResponseBlock {
                                compression: chunk.compression as u8,
                                bytes_len: chunk.content.len() as u64,
                                proof_len,
                            })
                            .await
                            .unwrap();
                            if proof_len != 0 {
                                conn.write_frame(CdnFrame::Buffer(proof.as_slice().into()))
                                    .await?;
                            }
                            conn.write_frame(CdnFrame::Buffer(chunk.content.as_slice().into()))
                                .await?;

                            // read delivery ack
                            match conn.read_frame(Some(DELIVERY_ACK_TAG)).await? {
                                Some(CdnFrame::DeliveryAcknowledgement { signature: _ }) => {
                                    // TODO: verify and store delivery acks
                                },
                                Some(_) => unreachable!(),
                                None => {
                                    return Err(anyhow!(
                                        "client disconnected while waiting for delivery ack"
                                    ));
                                },
                            }

                            block_counter += 1;
                            tree_idx = (block_counter * 2 - block_counter.count_ones()) as usize;
                        }
                    },
                }
            },
            CdnFrame::RangeRequest { .. } => todo!("handle range req"),
            _ => unreachable!(),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use affair::{Executor, TokioSpawn};
    use anyhow::Result;
    use draco_application::{
        app::Application,
        config::{Config, Mode},
    };
    use draco_blockstore::memory::MemoryBlockStore;
    use draco_handshake::server::{HandshakeServer, HandshakeServerConfig, TcpHandshakeServer};
    use draco_interfaces::{
        ApplicationInterface, BlockStoreInterface, CompressionAlgorithm, FileSystemInterface,
        HandshakeInterface, IncrementalPutInterface, SdkInterface, WithStartAndShutdown,
    };
    use fleek_crypto::ClientPublicKey;
    use tokio::net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    };

    use crate::{
        client::CdnClient,
        connection::ServiceMode,
        dummy::{FileSystem, Indexer, MyReputationReporter, Sdk, Signer},
        server::setup_cdn_service,
    };

    fn create_content() -> Vec<u8> {
        (0..4)
            .map(|i| Vec::from([i; 256 * 1024]))
            .flat_map(|a| a.into_iter())
            .collect()
    }

    async fn setup_node(
        listen_addr: SocketAddr,
    ) -> Result<(
        TcpHandshakeServer<Sdk<OwnedReadHalf, OwnedWriteHalf>>,
        Sdk<OwnedReadHalf, OwnedWriteHalf>,
        [u8; 32],
    )> {
        // setup blockstore with some content
        let blockstore = MemoryBlockStore::init(draco_blockstore::config::Config {}).await?;
        let content = create_content();
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        let hash = putter.finalize().await.unwrap();

        // setup sdk and friends
        let signer = TokioSpawn::spawn(Signer {});
        let app = Application::init(Config {
            genesis: None,
            mode: Mode::Test,
        })
        .await?;
        let sdk = Sdk::<OwnedReadHalf, OwnedWriteHalf>::new(
            app.sync_query(),
            MyReputationReporter {},
            FileSystem::new(&blockstore, &Indexer {}),
            signer,
        );

        // finally, initialize the handshake server
        let server = HandshakeServer::init(HandshakeServerConfig { listen_addr }).await?;

        Ok((server, sdk, hash))
    }

    #[tokio::test]
    async fn raw_mode_multi_block() -> Result<()> {
        let server_addr: SocketAddr = ([0; 4], 6969).into();

        // setup server
        let (mut server, sdk, hash) = setup_node(server_addr).await?;
        let cdn_handler = setup_cdn_service(sdk.clone());
        server.register_service_request_handler(0, sdk, cdn_handler);
        server.start().await;

        // connect over tcp and setup client
        let mut tcp = TcpStream::connect(server_addr).await?;
        let (r, w) = tcp.split();
        // setup client, performing a handshake and sending a service request to the server
        let mut client = CdnClient::new(r, w, ClientPublicKey([0u8; 20]))
            .await
            .unwrap();

        // request some content
        let mut res = client.request(ServiceMode::Optimistic, hash).await.unwrap();

        // iterate over each chunk of the data
        while let Some(bytes) = res.next().await? {
            assert_eq!(bytes.len(), 256 * 1024);
        }

        // Cleanup client and server
        // TODO: cleaner finish method which returns the raw streams back
        drop(client);
        server.shutdown().await;

        Ok(())
    }
}
