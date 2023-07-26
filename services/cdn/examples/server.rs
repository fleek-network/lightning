// TODO(qti3e): Bring this back.
fn main() {}

// use std::net::SocketAddr;
//
// use anyhow::Result;
// use draco_handshake::server::{HandshakeServerConfig, RawLaneConnection, TcpHandshakeServer};
// use draco_interfaces::{
//     ApplicationInterface, BlockStoreInterface, CompressionAlgorithm, ConnectionInterface,
//     FileSystemInterface, HandshakeInterface, IncrementalPutInterface, SdkInterface,
//     SignerInterface, WithStartAndShutdown,
// };
// use draco_test_utils::{
//     app::app::Application,
//     blockstore::MemoryBlockStore,
//     empty_interfaces::{
//         MockConfig, MockIndexer, MockQueryRunner, MockReputationReporter, MockSigner,
//     },
//     filesystem::MockFileSystem,
// };
// use fleek_cdn::server::handle_session;
// use futures::FutureExt;
// use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
//
// fn create_content() -> Vec<u8> {
//     (0..4)
//         .map(|i| Vec::from([i; 256 * 1024]))
//         .flat_map(|a| a.into_iter())
//         .collect()
// }
//
// #[tokio::main]
// async fn main() -> Result<()> {
//     let server_addr: SocketAddr = ([0; 4], 6969).into();
//
//     // setup blockstore with some content
//     let blockstore = MemoryBlockStore::init(draco_test_utils::blockstore::Config {}).await?;
//     let content = create_content();
//     let mut putter = blockstore.put(None);
//     putter
//         .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
//         .unwrap();
//     let hash = putter.finalize().await.unwrap();
//     println!("content hash: {hash:?}");
//
//     // setup sdk and friends
//     let app = Application::init(draco_test_utils::app::config::Config {
//         genesis: None,
//         mode: draco_test_utils::app::config::Mode::Test,
//     })
//     .await?;
//     let signer = MockSigner::init(MockConfig {}, MockQueryRunner {}).await?;
//     let sdk = MockSdk::<OwnedReadHalf, OwnedWriteHalf>::new(
//         app.sync_query(),
//         MockReputationReporter {},
//         MockFileSystem::new(&blockstore, &MockIndexer {}),
//         signer.get_socket(),
//     );
//
//     // initialize the handshake server
//     let mut server = TcpHandshakeServer::init(HandshakeServerConfig {
//         listen_addr: server_addr,
//     })
//     .await?;
//
//     // setup and register the cdn handler
//     let handler = |s, c: RawLaneConnection<OwnedReadHalf, OwnedWriteHalf>| {
//         async {
//             let client = *c.get_client();
//             println!("handling new cdn session for pubkey {client}");
//             if let Err(e) = handle_session(s, c).await {
//                 eprintln!("Session error: {e}")
//             }
//             println!("session completed for {client}");
//         }
//         .boxed()
//     };
//     server.register_service_request_handler(0, sdk, handler);
//
//     // finally, start the node
//     server.start().await;
//     println!("listening on {server_addr}");
//
//     tokio::signal::ctrl_c()
//         .await
//         .expect("Failed to setup control-c handler.");
//     server.shutdown().await;
//     Ok(())
// }
