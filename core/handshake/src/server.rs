use std::marker::PhantomData;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::{ClientPublicKey, NodePublicKey};
use lightning_interfaces::handshake::HandshakeInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::CompressionAlgoSet;
use lightning_interfaces::{ConfigConsumer, ConnectionInterface, WithStartAndShutdown};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::oneshot::{channel, Sender};
use tokio::{select, task};

use crate::connection::consts::{DELIVERY_ACK_TAG, HANDSHAKE_REQ_TAG, SERVICE_REQ_TAG};
use crate::connection::{HandshakeConnection, HandshakeFrame, Reason};

/// Generic listener to accept new connection streams with.
/// TODO: Move into interfaces
#[async_trait]
pub trait StreamProvider: Sync + Send + Sized {
    type Reader: AsyncRead + Sync + Send + Unpin;
    type Writer: AsyncWrite + Sync + Send + Unpin;

    async fn new<A: ToSocketAddrs + Send>(listen_addr: A) -> Result<Self>;
    async fn accept(&self) -> Result<Option<(Self::Reader, Self::Writer)>>;
}

pub struct TcpProvider {
    listener: TcpListener,
}

#[async_trait]
impl StreamProvider for TcpProvider {
    type Reader = OwnedReadHalf;
    type Writer = OwnedWriteHalf;

    async fn new<A: ToSocketAddrs + Send>(listen_addr: A) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(listen_addr).await?,
        })
    }

    async fn accept(&self) -> Result<Option<(Self::Reader, Self::Writer)>> {
        let conn = self.listener.accept().await?.0.into_split();
        Ok(Some(conn))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HandshakeServerConfig {
    pub listen_addr: SocketAddr,
}

impl Default for HandshakeServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from_str("0.0.0.0:6969").unwrap(),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum LaneState {
    #[default]
    Open,
    Active,
    Disconnected,
}

pub type TcpHandshakeServer<C> = HandshakeServer<C, TcpProvider>;

pub struct HandshakeServer<C: Collection, L: StreamProvider> {
    shutdown_channel: Arc<Mutex<Option<Sender<()>>>>,
    inner: Arc<HandshakeServerInner>,
    listener: Arc<L>,
    collection: PhantomData<C>,
}

#[derive(Clone, Default)]
pub struct HandshakeServerInner {
    lanes: Arc<DashMap<ClientPublicKey, [LaneState; 24]>>,
}

impl<C: Collection, L: StreamProvider> ConfigConsumer for HandshakeServer<C, L> {
    const KEY: &'static str = "handshake";

    type Config = HandshakeServerConfig;
}

#[async_trait]
impl<C: Collection, L: StreamProvider + 'static> WithStartAndShutdown for HandshakeServer<C, L> {
    fn is_running(&self) -> bool {
        self.shutdown_channel.lock().unwrap().is_some()
    }

    async fn start(&self) {
        if !self.is_running() {
            let (tx, mut rx) = channel();
            let listener = self.listener.clone();
            let inner = self.inner.clone();

            task::spawn(async move {
                loop {
                    select! {
                        Ok(Some((reader, writer))) = listener.accept() => {
                            let inner = inner.clone();
                            task::spawn(async move {
                                let conn = HandshakeConnection::new(reader, writer);
                                if let Err(e) = HandshakeServerInner::handle(inner, conn).await {
                                    eprintln!("Error handling connection: {e:?}");
                                }
                            });
                        }
                        _ = &mut rx => break,
                    }
                }
            });

            *self.shutdown_channel.lock().unwrap() = Some(tx);
        }
    }

    async fn shutdown(&self) {
        let mut sender = self.shutdown_channel.lock().unwrap();
        if let Some(tx) = sender.take() {
            tx.send(()).unwrap();
        }
    }
}

#[async_trait]
impl<
    C: Collection,
    // This implementation accepts an SDK that uses our [`RawLaneConnection`],
    // and the same reader/writer types that the StreamProvider yields.
    L: StreamProvider + 'static,
> HandshakeInterface<C> for HandshakeServer<C, L>
{
    type Connection = RawLaneConnection<L::Reader, L::Writer>;

    fn init(config: Self::Config) -> anyhow::Result<Self> {
        // TODO: Do not consume resources on initialization.
        let listener = futures::executor::block_on(L::new(config.listen_addr))?.into();

        Ok(Self {
            listener,
            inner: Arc::new(HandshakeServerInner::new()),
            shutdown_channel: Mutex::new(None).into(),
            collection: PhantomData,
        })
    }
}

impl HandshakeServerInner {
    pub fn new() -> HandshakeServerInner {
        Self::default()
    }

    pub async fn handle<
        R: AsyncRead + Unpin + Send + Sync + 'static,
        W: AsyncWrite + Unpin + Send + Sync + 'static,
    >(
        inner: Arc<HandshakeServerInner>,
        mut conn: HandshakeConnection<R, W>,
    ) -> Result<()> {
        // wait for a handshake request
        match conn.read_frame(Some(HANDSHAKE_REQ_TAG)).await? {
            Some(HandshakeFrame::HandshakeRequest {
                resume_lane,
                pubkey,
                supported_compression_set: _,
                ..
            }) => {
                let mut user_lanes = inner.lanes.entry(pubkey).or_default();

                // find or resume a lane
                let _lane = match resume_lane {
                    Some(lane) => {
                        let state = &mut user_lanes[lane as usize];
                        if *state == LaneState::Disconnected {
                            // 2. send response w last info
                            conn.write_frame(HandshakeFrame::HandshakeResponseUnlock {
                                pubkey: NodePublicKey([0u8; 32]),
                                nonce: 1000,
                                lane,
                                // TODO: When exiting, services should return if the session was
                                // pending a delivery acknowledgement.
                                last_bytes: 0,
                                last_service_id: 0,
                                last_signature: [0; 96],
                            })
                            .await?;

                            // todo: read delivery acknowledgment
                            match conn.read_frame(Some(DELIVERY_ACK_TAG)).await? {
                                Some(HandshakeFrame::DeliveryAcknowledgement { .. }) => {
                                    // TODO: verify & submit signature
                                    *state = LaneState::Active;
                                    lane
                                },
                                _ => unreachable!(),
                            }
                        } else {
                            todo!("handle resume lane not disconnected")
                        }
                    },
                    None => {
                        // find open lane
                        if let Some(lane) = user_lanes
                            .iter()
                            .position(|&s| s == LaneState::Open)
                            .map(|l| l as u8)
                        {
                            conn.write_frame(HandshakeFrame::HandshakeResponse {
                                pubkey: NodePublicKey([0u8; 32]),
                                nonce: 1000,
                                lane,
                            })
                            .await?;

                            lane
                        } else {
                            conn.termination_signal(Reason::OutOfLanes).await.ok();
                            return Err(anyhow!("out of lanes"));
                        }
                    },
                };

                // wait for a service request
                match conn.read_frame(Some(SERVICE_REQ_TAG)).await? {
                    Some(HandshakeFrame::ServiceRequest { .. }) => {
                        // TODO(qti3e): Bring these back when the handshake interface has a way to
                        // direct a connection to a service.
                        //
                        // match inner.services.get(&service_id) {
                        //     Some(res) => {
                        //         let (sdk, handler) = res.clone();
                        //         let (read, write) = conn.finish();
                        //         let conn = RawLaneConnection::new(
                        //             read,
                        //             write,
                        //             lane,
                        //             pubkey,
                        //             supported_compression_set,
                        //         );

                        //         // TODO: Figure out lifetimes to more correctly pass conn as a
                        //         //       mutable reference.
                        //         handler(sdk, conn).await;

                        //         // TODO: Should we loop back to waiting for another service
                        //         //       request here?
                        //         Ok(())
                        //     },
                        //     None => {
                        //         conn.termination_signal(Reason::ServiceNotFound).await.ok();
                        //         Err(anyhow!("service not found"))
                        //     },
                        // }
                        Ok(())
                    },
                    None => Err(anyhow!("session disconnected")),
                    _ => unreachable!(), // Guaranteed by frame filter
                }
            },
            None => Err(anyhow!("session disconnected")),
            Some(_) => unreachable!(), // Guaranteed by frame filter
        }
    }
}

pub struct RawLaneConnection<R: AsyncRead + Send + Sync, W: AsyncWrite + Send + Sync> {
    reader: R,
    writer: W,
    lane: u8,
    client_id: ClientPublicKey,
    compression_set: CompressionAlgoSet,
}

impl<R: AsyncRead + Send + Sync, W: AsyncWrite + Send + Sync> RawLaneConnection<R, W> {
    pub fn new(
        reader: R,
        writer: W,
        lane: u8,
        client_id: ClientPublicKey,
        compression_set: CompressionAlgoSet,
    ) -> Self {
        Self {
            reader,
            writer,
            lane,
            client_id,
            compression_set,
        }
    }
}

impl<R: AsyncRead + Unpin + Send + Sync, W: AsyncWrite + Unpin + Send + Sync> ConnectionInterface
    for RawLaneConnection<R, W>
{
    type Writer = W;
    type Reader = R;

    fn split(&mut self) -> (&mut Self::Writer, &mut Self::Reader) {
        (&mut self.writer, &mut self.reader)
    }
    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.writer
    }
    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.reader
    }
    fn get_lane(&self) -> u8 {
        self.lane
    }
    fn get_client(&self) -> &ClientPublicKey {
        &self.client_id
    }
    fn get_compression_set(&self) -> CompressionAlgoSet {
        self.compression_set
    }
}

// TODO(qti3e): Bring these tests back to life after we have more things in the mock crate.
//
// #[cfg(test)]
// mod tests {
//     use affair::{Executor, TokioSpawn};
//     use lightning_application::{app::Application, config::Config};
//     use lightning_interfaces::ApplicationInterface;
//     use fleek_crypto::ClientSignature;
//     use tokio::{
//         self,
//         io::{AsyncReadExt, AsyncWriteExt},
//         net::TcpStream,
//     };
//
//     use super::*;
//     use crate::client::HandshakeClient;
//
//     async fn hello_world_server(
//         port: u16,
//     ) -> Result<(
//         TcpHandshakeServer<Sdk<OwnedReadHalf, OwnedWriteHalf>>,
//         Sdk<OwnedReadHalf, OwnedWriteHalf>,
//     )> {
//         let signer = TokioSpawn::spawn(Signer {});
//         let app = Application::init(Config::default()).await?;
//         let sdk = Sdk::<OwnedReadHalf, OwnedWriteHalf>::new(
//             app.sync_query(),
//             MyReputationReporter {},
//             FileSystem {},
//             signer,
//         );
//         let config = HandshakeServerConfig {
//             listen_addr: ([0; 4], port).into(),
//         };
//         Ok((HandshakeServer::init(config).await?, sdk))
//     }
//
//     #[tokio::test]
//     async fn handshake_e2e() -> anyhow::Result<()> {
//         // server setup
//         let (mut server, sdk) = hello_world_server(6969).await?;
//
//         // service setup
//         server.register_service_request_handler(0, sdk, |_, mut conn| {
//             Box::pin(async move {
//                 let writer = conn.writer();
//                 writer.write_all(b"hello lightning").await.unwrap();
//             })
//         });
//
//         server.start().await;
//         assert!(server.is_running());
//
//         // dial the server and create a client
//         let (read, write) = TcpStream::connect("0.0.0.0:6969").await?.into_split();
//         let mut client = HandshakeClient::new(
//             read,
//             write,
//             ClientPublicKey([0u8; 20]),
//             CompressionAlgoSet::new(),
//         );
//
//         // send a handshake
//         client.handshake().await?;
//
//         // start a service request
//         let (mut r, _) = client.request(0).await?;
//
//         // service subprotocol
//         let mut buf = [0u8; 11];
//         r.read_exact(&mut buf).await?;
//         assert_eq!(String::from_utf8_lossy(&buf), "hello lightning");
//
//         server.shutdown().await;
//         assert!(!server.is_running());
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn handshake_unlock_e2e() -> anyhow::Result<()> {
//         // server setup
//         let (mut server, sdk) = hello_world_server(6970).await?;
//
//         let mut lanes = server
//             .inner
//             .lanes
//             .entry(ClientPublicKey([0u8; 20]))
//             .or_default();
//         lanes[0] = LaneState::Disconnected;
//         drop(lanes);
//
//         // service setup
//         server.register_service_request_handler(0, sdk, |_, mut conn| {
//             Box::pin(async move {
//                 let writer = conn.writer();
//                 writer.write_all(b"hello lightning").await.unwrap();
//             })
//         });
//
//         server.start().await;
//         assert!(server.is_running());
//
//         // dial the server and create a client
//         let (read, write) = TcpStream::connect("0.0.0.0:6970").await?.into_split();
//         let mut client = HandshakeClient::new(
//             read,
//             write,
//             ClientPublicKey([0u8; 20]),
//             CompressionAlgoSet::new(),
//         );
//
//         // send a handshake
//         client.handshake_unlock(0, ClientSignature).await?;
//
//         // start a service request
//         let (mut r, _) = client.request(0).await?;
//
//         // service subprotocol
//         let mut buf = [0u8; 11];
//         r.read_exact(&mut buf).await?;
//         assert_eq!(String::from_utf8_lossy(&buf), "hello lightning");
//
//         server.shutdown().await;
//         assert!(!server.is_running());
//
//         Ok(())
//     }
// }
