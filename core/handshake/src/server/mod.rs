#![allow(unused)] // temp

pub mod config;

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
pub use config::HandshakeServerConfig;
use fleek_crypto::{ClientPublicKey, NodePublicKey};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::ConnectionMetadata;
use lightning_interfaces::{
    ConfigConsumer,
    HandshakeInterface,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::select;
use tracing::error;

use crate::connection::consts::{HANDSHAKE_REQ_TAG, SERVICE_REQ_TAG, VERSION};
use crate::connection::{HandshakeConnection, HandshakeFrame, Reason};

pub type TcpHandshakeServer<C> = infusion::Blank<C>;

pub struct HandshakeServer<C: Collection> {
    config: HandshakeServerConfig,
    state: HandshakeState<C>,
    shutdown_channel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for HandshakeServer<C> {
    fn is_running(&self) -> bool {
        self.shutdown_channel.lock().unwrap().is_some()
    }

    async fn start(&self) {
        if !self.is_running() {
            let (tx, mut rx) = tokio::sync::oneshot::channel();

            // TODO: Bind to ports and get connection listeners
            // let listener_websocket = L::new(self.config.address)
            //     .await
            //     .expect("failed to establish listener");

            tokio::spawn(async move {
                loop {
                    select! {
                        // TODO: Listen for new connections and spawn handler task
                        _ = async {} => {},
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

impl<C: Collection> ConfigConsumer for HandshakeServer<C> {
    const KEY: &'static str = "handshake";

    type Config = HandshakeServerConfig;
}

impl<C: Collection> HandshakeInterface<C> for HandshakeServer<C> {
    fn init(config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            state: HandshakeState {
                collection: PhantomData,
            },
            shutdown_channel: Mutex::new(None).into(),
        })
    }
}

impl<C: Collection> HandshakeServer<C> {}

pub struct HandshakeState<C: Collection> {
    collection: PhantomData<C>,
}

impl<C: Collection> Clone for HandshakeState<C> {
    fn clone(&self) -> Self {
        Self {
            collection: PhantomData,
        }
    }
}

impl<C: Collection> HandshakeState<C> {
    /// Reserve an open lane for the client. Returns None if there are no open lanes.
    pub fn reserve_lane(&self, _client: ClientPublicKey) -> Option<u8> {
        // TODO: Store lane state
        Some(0)
    }

    /// Open a lane back up after a connection terminates.
    pub fn reopen_lane(&self, _client: ClientPublicKey, _lane: u8) {}
}

pub struct ServiceConnection {
    metadata: ConnectionMetadata,
}

async fn handle<C: Collection>(
    state: HandshakeState<C>,
    reader: impl AsyncRead + Send + Sync + Unpin + 'static,
    writer: impl AsyncWrite + Send + Sync + Unpin + 'static,
) where
    C::HandshakeInterface: HandshakeInterface<C>,
{
    error!("handling inside");
    let mut conn = HandshakeConnection::new(reader, writer);

    // Read handshake request
    match conn.read_frame(Some(HANDSHAKE_REQ_TAG)).await {
        Ok(Some(HandshakeFrame::HandshakeRequest {
            version,
            supported_compression_set,
            pubkey,
            resume_lane,
        })) => {
            if version != VERSION {
                error!("received handshake request for an unsupported version");
                conn.termination_signal(Reason::UnsupportedVersion)
                    .await
                    .ok();
                return;
            }

            let lane = if let Some(_lane) = resume_lane {
                todo!("Handle lane resumption (receive and verify delivery ack)")
            } else {
                let Some(lane) = state.reserve_lane(pubkey) else {
                        conn.termination_signal(Reason::OutOfLanes).await.ok();
                        return;
                    };

                // Send handshake response
                if let Err(e) = conn
                    .write_frame(HandshakeFrame::HandshakeResponse {
                        lane,
                        // TODO: Re-add signer to get our pubkey for client verification.
                        //       Also, shouldn't this be the key used to sign commitments?
                        pubkey: NodePublicKey([0; 32]),
                        // TODO: proper nonce support
                        nonce: 0,
                    })
                    .await
                {
                    error!("failed to send handshake response: {e}");
                    state.reopen_lane(pubkey, lane);
                    return;
                }

                lane
            };

            // Read a service request frame
            match conn.read_frame(Some(SERVICE_REQ_TAG)).await {
                Ok(Some(HandshakeFrame::ServiceRequest { service_id })) => {
                    // create channels for the service connection
                    // let (in_tx, in_rx) = tokio::sync::mpsc::channel(256);
                    // let (out_tx, mut out_rx) = tokio::sync::mpsc::channel(256);

                    // TODO: spawn task for reading and writing length delimited frame buffers for
                    // the service.

                    // create connection
                    let connection = ServiceConnection {
                        metadata: ConnectionMetadata {
                            lane,
                            client: pubkey,
                            compression_set: supported_compression_set,
                        },
                    };

                    // Handle the service connection (background)
                    // state.connector.handle(service_id, connection);

                    // TODO: Wait for service connection to drop, and handle more service
                    // requests
                },
                Ok(None) => error!("failed to read service request: connection terminated"),
                Err(e) => error!("failed to read service request: {e}"),
                _ => unreachable!(),
            }

            // Finally, reopen the lane
            state.reopen_lane(pubkey, lane);
        },
        Ok(None) => error!("failed to read handshake request: connection terminated"),
        Err(e) => error!("failed to read handshake request: {e}"),
        _ => unreachable!(),
    };
}

#[cfg(test)]
mod tests {
    use infusion::Blank;
    use lightning_interfaces::partial;
    use lightning_interfaces::types::CompressionAlgoSet;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::oneshot::channel;

    use super::*;
    use crate::client::HandshakeClient;

    partial!(TestBindings {
        HandshakeInterface = HandshakeServer<Self>;
    });

    #[tokio::test]
    async fn handle_blank_handshake() {
        // Bind to a tcp port and get the address
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get listen address");

        // Spawn task to accept a single connection
        let (tx, rx) = channel();
        tokio::task::spawn(async move {
            let (s, _) = listener.accept().await.unwrap();
            tx.send(s).unwrap();
        });

        // Get streams
        let client_stream = TcpStream::connect(addr)
            .await
            .expect("failed to establish stream");
        let server_stream = rx.await.unwrap();

        // Setup dependencies and state
        let executor = Blank::<TestBindings>::default();
        let state = HandshakeState::<TestBindings> {
            collection: PhantomData,
        };

        // spawn client task
        let (r, w) = server_stream.into_split();
        let server = tokio::spawn(handle(state, r, w));

        // handle the connection
        let (r, w) = client_stream.into_split();
        let mut client = HandshakeClient::new(
            r,
            w,
            ClientPublicKey([0; 96]),
            CompressionAlgoSet::default(),
        );
        client
            .handshake()
            .await
            .expect("failed to handshake with server");
        client.request(0).await.expect("failed to request service");

        server.await.unwrap();
    }
}
