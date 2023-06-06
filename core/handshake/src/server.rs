use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use draco_interfaces::{
    handshake::HandshakeInterface, types::ServiceId, ConfigConsumer, ConnectionInterface,
    HandlerFn, SdkInterface, WithStartAndShutdown,
};
use fleek_crypto::{ClientPublicKey, NodePublicKey};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::ToSocketAddrs,
    select,
    sync::oneshot::{channel, Sender},
    task,
};

use crate::connection::{
    consts::{DELIVERY_ACK_TAG, HANDSHAKE_REQ_TAG, SERVICE_REQ_TAG},
    HandshakeConnection, HandshakeFrame, Reason,
};

/// Generic listener to accept new connection streams with.
/// TODO: Move into interfaces
#[async_trait]
pub trait StreamProvider: Sync + Send + Sized {
    type Reader: AsyncRead + Sync + Send + Unpin;
    type Writer: AsyncWrite + Sync + Send + Unpin;

    async fn new<A: ToSocketAddrs + Send>(listen_addr: A) -> Result<Self>;
    async fn accept(&self) -> Result<Option<(Self::Reader, Self::Writer)>>;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HandshakeServerConfig {
    listen_addr: SocketAddr,
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

pub struct HandshakeServer<
    SDK: SdkInterface<Connection = RawLaneConnection<L::Reader, L::Writer>>,
    L: StreamProvider,
> {
    shutdown_channel: Arc<Mutex<Option<Sender<()>>>>,
    inner: Arc<HandshakeServerInner<SDK>>,
    listener: Arc<L>,
}

#[derive(Clone)]
pub struct HandshakeServerInner<SDK: SdkInterface> {
    services: Arc<DashMap<ServiceId, (SDK, HandlerFn<'static, SDK>)>>,
    lanes: Arc<DashMap<ClientPublicKey, [LaneState; 24]>>,
}

impl<SDK: SdkInterface<Connection = RawLaneConnection<L::Reader, L::Writer>>, L: StreamProvider>
    ConfigConsumer for HandshakeServer<SDK, L>
{
    const KEY: &'static str = "handshake";

    type Config = HandshakeServerConfig;
}

#[async_trait]
impl<
    SDK: SdkInterface<Connection = RawLaneConnection<L::Reader, L::Writer>>,
    L: StreamProvider + 'static,
> WithStartAndShutdown for HandshakeServer<SDK, L>
{
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
    // This implementation accepts an SDK that uses our [`RawLaneConnection`],
    // and the same reader/writer types that the StreamProvider yields.
    SDK: SdkInterface<Connection = RawLaneConnection<L::Reader, L::Writer>>,
    L: StreamProvider + 'static,
> HandshakeInterface for HandshakeServer<SDK, L>
{
    type Sdk = SDK;

    async fn init(config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            listener: L::new(config.listen_addr).await?.into(),
            inner: Arc::new(HandshakeServerInner::new().await),
            shutdown_channel: Mutex::new(None).into(),
        })
    }

    fn register_service_request_handler(
        &mut self,
        service: draco_interfaces::types::ServiceId,
        sdk: Self::Sdk,
        handler: draco_interfaces::HandlerFn<'static, Self::Sdk>,
    ) {
        // TODO: adjust the interface; sdk should probably be added on init,
        //       instead of passed in along each service.

        self.inner.services.insert(service, (sdk, handler));
    }
}

impl<'a, SDK: SdkInterface> HandshakeServerInner<SDK> {
    pub async fn new() -> HandshakeServerInner<SDK> {
        Self {
            services: DashMap::new().into(),
            lanes: DashMap::new().into(),
        }
    }

    pub async fn handle<
        R: AsyncRead + Unpin + Send + Sync + 'static,
        W: AsyncWrite + Unpin + Send + Sync + 'static,
    >(
        inner: Arc<HandshakeServerInner<SDK>>,
        mut conn: HandshakeConnection<R, W>,
    ) -> Result<()>
    where
        SDK: SdkInterface<Connection = RawLaneConnection<R, W>>,
    {
        // wait for a handshake request
        match conn.read_frame(Some(HANDSHAKE_REQ_TAG)).await? {
            Some(HandshakeFrame::HandshakeRequest {
                resume_lane,
                pubkey,
                ..
            }) => {
                let mut user_lanes = inner.lanes.entry(pubkey).or_default();

                // find or resume a lane
                let lane = match resume_lane {
                    Some(lane) => {
                        let state = &mut user_lanes[lane as usize];
                        if *state == LaneState::Disconnected {
                            // 2. send response w last info
                            conn.write_frame(HandshakeFrame::HandshakeResponse {
                                pubkey: NodePublicKey([0u8; 96]),
                                nonce: 1000,
                                lane,
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
                                pubkey: NodePublicKey([0u8; 96]),
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
                    Some(HandshakeFrame::ServiceRequest { service_id }) => {
                        match inner.services.get(&service_id) {
                            Some(res) => {
                                let (sdk, handler) = res.clone();
                                let (read, write) = conn.finish();
                                let conn = SDK::Connection::new(read, write, lane, pubkey);

                                // TODO: Figure out lifetimes to more correctly pass conn as a
                                //       mutable reference.
                                handler(sdk, conn).await;

                                // TODO: Should we loop back to waiting for another service
                                //       request here?
                                Ok(())
                            },
                            None => {
                                conn.termination_signal(Reason::ServiceNotFound).await.ok();
                                Err(anyhow!("service not found"))
                            },
                        }
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
}

impl<R: AsyncRead + Send + Sync, W: AsyncWrite + Send + Sync> RawLaneConnection<R, W> {
    pub fn new(reader: R, writer: W, lane: u8, client_id: ClientPublicKey) -> Self {
        Self {
            reader,
            writer,
            lane,
            client_id,
        }
    }
}

impl<R: AsyncRead + Unpin + Send + Sync, W: AsyncWrite + Unpin + Send + Sync> ConnectionInterface
    for RawLaneConnection<R, W>
{
    type Writer = W;
    type Reader = R;

    fn new(reader: R, writer: W, lane: u8, client_id: ClientPublicKey) -> Self {
        Self {
            reader,
            writer,
            lane,
            client_id,
        }
    }

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
}

#[cfg(test)]
mod tests {
    use affair::{Executor, TokioSpawn};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{
            tcp::{OwnedReadHalf, OwnedWriteHalf},
            TcpListener, TcpStream,
        },
    };

    use crate::{
        client::HandshakeClient,
        dummy::{FileSystem, MyReputationReporter, QueryRunner, Sdk, Signer},
    };

    struct TcpProvider {
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

    use super::*;

    #[tokio::test]
    async fn hello_world_service() -> anyhow::Result<()> {
        // server setup
        let signer = TokioSpawn::spawn(Signer {});
        let sdk = Sdk::new(
            QueryRunner {},
            MyReputationReporter {},
            FileSystem {},
            signer,
        );
        let config = HandshakeServerConfig::default();
        let mut server = HandshakeServer::<
            Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
            TcpProvider,
        >::init(config)
        .await?;

        // service setup
        server.register_service_request_handler(0, sdk, |_, mut conn| {
            Box::pin(async move {
                let writer = conn.writer();
                writer.write_all(b"hello draco").await.unwrap();
            })
        });

        server.start().await;
        assert!(server.is_running());

        // dial the server and create a client
        let (read, write) = TcpStream::connect("0.0.0.0:6969").await?.into_split();
        let mut client = HandshakeClient::new(read, write, ClientPublicKey([0u8; 20]));

        // send a handshake
        client.handshake().await?;

        // start a service request
        let (mut r, _) = client.request(0).await?;

        // service subprotocol
        let mut buf = [0u8; 11];
        r.read_exact(&mut buf).await?;
        assert_eq!(String::from_utf8_lossy(&buf), "hello draco");

        server.shutdown().await;
        assert!(!server.is_running());

        Ok(())
    }
}
