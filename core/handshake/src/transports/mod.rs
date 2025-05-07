use async_trait::async_trait;
use axum::Router;
use bytes::{BufMut, Bytes, BytesMut};
use fn_sdk::header::TransportDetail;
use futures::Future;
use lightning_interfaces::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

use self::mock::{MockTransportReceiver, MockTransportSender};
use self::tcp::{TcpReceiver, TcpSender};
use self::webrtc::{WebRtcReceiver, WebRtcSender};
use self::webtransport::{WebTransportReceiver, WebTransportSender};
use crate::config::TransportConfig;
use crate::handshake::Context;
use crate::schema;
use crate::transports::http::{HttpReceiver, HttpSender};

pub mod http;
pub mod mock;
pub mod tcp;
pub mod webrtc;
pub mod webtransport;

macro_rules! transport_pairs {
    {$($name:ident($sender:tt, $receiver:tt),)*} => {
        /// Static enum of transport pairs for handling secondary connections
        #[non_exhaustive]
        pub enum TransportPair {
            $($name($sender, $receiver),)*
        }

        $(
        impl From<($sender, $receiver)> for TransportPair {
            fn from(value: ($sender, $receiver)) -> Self {
                TransportPair::$name(value.0, value.1)
            }
        }
        )*

        macro_rules! match_transport {
            ($pair:tt { ($tx:ident, $rx:ident) => $call:expr }) => {
                match $pair {
                    $(TransportPair::$name($tx, $rx) => { $call },)*
                }
            };
        }

        pub(super) use match_transport;
    }
}

transport_pairs! {
    Mock(MockTransportSender, MockTransportReceiver),
    Tcp(TcpSender, TcpReceiver),
    WebRtc(WebRtcSender, WebRtcReceiver),
    WebTransport(WebTransportSender, WebTransportReceiver),
    Http(HttpSender, HttpReceiver),

}

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

    /// Spawn a thread loop accepting connections and initializing the connection to the service.
    #[inline(always)]
    fn spawn_listener_task<P: ExecutorProviderInterface, QR: SyncQueryRunnerInterface>(
        mut self,
        ctx: Context<P, QR>,
    ) where
        (Self::Sender, Self::Receiver): Into<TransportPair>,
    {
        spawn!(
            async move {
                loop {
                    tokio::select! {
                        res = self.accept() => match res {
                            // Connection established with a handshake frame
                            Some((req, tx, rx)) => ctx.handle_new_connection(req, tx, rx).await,
                            // The transport listener has closed
                            None => break,
                        },
                        _ = ctx.shutdown.wait_for_shutdown() => break,
                    }
                }
            },
            "HANDSHAKE: accept connections"
        );
    }
}

pub trait TransportSender: Sized + Send + Sync + 'static {
    /// Send the initial handshake response to the client.
    fn send_handshake_response(
        &mut self,
        response: schema::HandshakeResponse,
    ) -> impl Future<Output = ()> + Send;

    /// Send a frame to the client.
    fn send(&mut self, frame: schema::ResponseFrame) -> impl Future<Output = ()> + Send;

    /// Terminate the connection
    fn terminate(mut self, reason: schema::TerminationReason) -> impl Future<Output = ()> + Send {
        async move {
            self.send(schema::ResponseFrame::Termination { reason })
                .await
        }
    }

    /// Declare a number of bytes to write as service payloads.
    fn start_write(&mut self, len: usize) -> impl Future<Output = ()> + Send;

    /// Write some bytes as service payloads. Must ALWAYS be called after
    /// [`TransportSender::start_write`].
    fn write(&mut self, buf: Bytes) -> impl Future<Output = anyhow::Result<usize>> + Send;
}

pub trait TransportReceiver: Send + Sync + 'static {
    /// Returns the transport detail from this connection which is then sent to the service on
    /// the hello frame.
    #[inline(always)]
    fn detail(&mut self) -> TransportDetail {
        TransportDetail::Other
    }

    /// Receive a frame from the connection. Returns `None` when the connection
    /// is closed.
    fn recv(&mut self) -> impl Future<Output = Option<schema::RequestFrame>> + Send;
}

pub async fn spawn_transport_by_config<
    P: ExecutorProviderInterface,
    QR: SyncQueryRunnerInterface,
>(
    shutdown: ShutdownWaiter,
    ctx: Context<P, QR>,
    config: TransportConfig,
) -> anyhow::Result<Option<Router>> {
    match config {
        TransportConfig::Mock(config) => {
            let (transport, router) = mock::MockTransport::bind(shutdown.clone(), config).await?;
            transport.spawn_listener_task(ctx);
            Ok(router)
        },
        TransportConfig::Tcp(config) => {
            let (transport, router) = tcp::TcpTransport::bind(shutdown.clone(), config).await?;
            transport.spawn_listener_task(ctx);
            Ok(router)
        },
        TransportConfig::WebRTC(config) => {
            let (transport, router) =
                webrtc::WebRtcTransport::bind(shutdown.clone(), config).await?;
            transport.spawn_listener_task(ctx);
            Ok(router)
        },
        TransportConfig::WebTransport(config) => {
            let (transport, router) =
                webtransport::WebTransport::bind(shutdown.clone(), config).await?;
            transport.spawn_listener_task(ctx);
            Ok(router)
        },
        TransportConfig::Http(config) => {
            let (_, router) = http::HttpTransport::<P, QR>::bind(shutdown.clone(), config).await?;
            // Axum has a `Context` and will handle `accept()`.
            Ok(router)
        },
    }
}

/// Delimit a complete frame with a u32 length.
#[inline(always)]
pub fn delimit_frame(bytes: Bytes) -> Bytes {
    let mut buf = BytesMut::with_capacity(4 + bytes.len());
    buf.put_u32(bytes.len() as u32);
    buf.put(bytes);
    buf.freeze()
}
