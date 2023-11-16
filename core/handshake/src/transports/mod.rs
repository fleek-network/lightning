use async_trait::async_trait;
use axum::Router;
use bytes::{BufMut, Bytes, BytesMut};
use lightning_interfaces::ExecutorProviderInterface;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::warn;
use triomphe::Arc;

use crate::config::TransportConfig;
use crate::handshake::Context;
use crate::schema;
use crate::shutdown::ShutdownWaiter;

pub mod mock;
pub mod tcp;
pub mod webrtc;
pub mod webtransport;

#[async_trait]
pub trait Transport: Sized + Send + Sync + 'static {
    type Config: Default + Serialize + DeserializeOwned;
    type Sender: TransportSender;
    type Receiver: TransportReceiver;

    /// Bind the transport with the provided config.
    async fn bind(
        shutdown: ShutdownWaiter,
        config: Self::Config,
    ) -> anyhow::Result<(Self, Option<Router>)>;

    /// Accept a new connection.
    async fn accept(
        &mut self,
    ) -> Option<(schema::HandshakeRequestFrame, Self::Sender, Self::Receiver)>;
}

// TODO: Explore being able to make this async while also avoiding dynamic dispatch after support
//       for secondary connections are added
pub trait TransportSender: Sized + Send + Sync + 'static {
    /// Send the initial handshake response to the client.
    fn send_handshake_response(&mut self, response: schema::HandshakeResponse);

    /// Send a frame to the client.
    fn send(&mut self, frame: schema::ResponseFrame);

    /// Terminate the connection
    fn terminate(mut self, reason: schema::TerminationReason) {
        self.send(schema::ResponseFrame::Termination { reason })
    }

    /// Declare a number of bytes to write as service payloads.
    fn start_write(&mut self, len: usize);

    /// Write some bytes as service payloads. Must ALWAYS be called after
    /// [`TransportSender::start_write`].
    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize>;
}

#[async_trait]
pub trait TransportReceiver: Send + Sync + 'static {
    /// Receive a frame from the connection. Returns `None` when the connection
    /// is closed.
    async fn recv(&mut self) -> Option<schema::RequestFrame>;
}

pub async fn spawn_transport_by_config<P: ExecutorProviderInterface>(
    shutdown: ShutdownWaiter,
    ctx: Arc<Context<P>>,
    config: TransportConfig,
) -> anyhow::Result<Option<Router>> {
    match config {
        TransportConfig::Mock(config) => {
            let (transport, router) = mock::MockTransport::bind(shutdown.clone(), config).await?;
            spawn_listener_task(transport, ctx);
            Ok(router)
        },
        TransportConfig::Tcp(config) => {
            let (transport, router) = tcp::TcpTransport::bind(shutdown.clone(), config).await?;
            spawn_listener_task(transport, ctx);
            Ok(router)
        },
        TransportConfig::WebRTC(config) => {
            let (transport, router) =
                webrtc::WebRtcTransport::bind(shutdown.clone(), config).await?;
            spawn_listener_task(transport, ctx);
            Ok(router)
        },
        TransportConfig::WebTransport(config) => {
            let (transport, router) =
                webtransport::WebTransport::bind(shutdown.clone(), config).await?;
            spawn_listener_task(transport, ctx);
            Ok(router)
        },
    }
}

/// Spawn a thread loop accepting connections and initializing the connection to the service.
fn spawn_listener_task<T: Transport, P: ExecutorProviderInterface>(
    mut transport: T,
    ctx: Arc<Context<P>>,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = transport.accept() => match res {
                    // Connection established with a handshake frame
                    Some((req, tx, rx)) => {
                        match req {
                            schema::HandshakeRequestFrame::Handshake {
                                retry: None,
                                service,
                                ..
                            } => ctx.handle_new_connection(service, tx, rx).await,
                            _ => warn!("TODO: support resumption and secondary connections"),
                        }
                    },
                    // The transport listener has closed
                    None => break,
                },
                _ = ctx.shutdown.wait_for_shutdown() => break,
            }
        }
    });
}

/// Delimit a complete frame with a u32 length.
pub fn delimit_frame(bytes: Bytes) -> Bytes {
    let mut buf = BytesMut::with_capacity(4 + bytes.len());
    buf.put_u32(bytes.len() as u32);
    buf.put(bytes);
    buf.into()
}
