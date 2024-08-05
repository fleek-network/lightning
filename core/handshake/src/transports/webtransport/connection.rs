use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::{NodeSecretKey, SecretKey};
use futures::StreamExt;
use lightning_interfaces::{spawn, ShutdownWaiter};
use tokio::sync::mpsc::Sender;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};
use tracing::{error, info};
use wtransport::endpoint::endpoint_side::Server;
use wtransport::endpoint::IncomingSession;
use wtransport::{Endpoint, RecvStream, SendStream};

use crate::schema::HandshakeRequestFrame;
use crate::transports::webtransport::{self, WebTransportConfig};

pub type FramedStreamRx = FramedRead<RecvStream, LengthDelimitedCodec>;

const CERTIFICATE_RENEWAL_PERIOD: u64 = 1166400; // 13.5 days.

/// The execution context of the WebTransport server.
pub struct Context {
    pub endpoint: Endpoint<Server>,
    pub accept_tx: Sender<(HandshakeRequestFrame, (SendStream, FramedStreamRx))>,
    pub published_cert_hash: Arc<RwLock<Vec<u8>>>,
    pub transport_config: WebTransportConfig,
    pub shutdown: ShutdownWaiter,
}

pub async fn main_loop(ctx: Context) {
    // Timer used to manage the self-signed certificate.
    let mut timer = tokio::time::interval(Duration::from_secs(CERTIFICATE_RENEWAL_PERIOD));
    // The first tick completes immediately.
    timer.tick().await;

    loop {
        tokio::select! {
            incoming = ctx.endpoint.accept() => {
                let accept_tx = ctx.accept_tx.clone();
                spawn!(async move  {
                    if let Err(e) = handle_incoming_session(incoming, accept_tx).await {
                        error!("failed to handle incoming WebTransport session: {e:?}");
                    }
                }, "HANDSHAKE: handle incoming webtransport session");
            }
            _ = timer.tick() => {
                match webtransport::create_cert_hash_and_server_config(
                    NodeSecretKey::generate(),
                    ctx.transport_config.clone()
                ).await {
                    Ok((cert_hash, server_config)) => {
                        match ctx.endpoint.reload_config(server_config, false) {
                            Ok(_) => {
                                *ctx.published_cert_hash.write().unwrap() = cert_hash;
                            }
                            Err(e) => {
                                error!("failed to reload server configuration: {e:?}");
                            }
                        }
                    },
                    Err(e) => {
                        error!("failed to reload server configuration: {e:?}");
                    }
                }
            }
            _ = ctx.shutdown.wait_for_shutdown() => {
                info!("shutting down WebTransport server");
                break;
            }
        }
    }
}

pub async fn handle_incoming_session(
    incoming: IncomingSession,
    accept_tx: Sender<(HandshakeRequestFrame, (SendStream, FramedStreamRx))>,
) -> Result<()> {
    let session_request = incoming.await?;
    // Todo: validate authority and scheme.
    // Upon receiving an extended CONNECT request with a :protocol field set to webtransport,
    // the HTTP/3 server can check if it has a WebTransport server
    // associated with the specified :authority and :path values.
    // If it does not, it SHOULD reply with status code 404 (Section 15.5.5 of [HTTP]).
    // When the request contains the Origin header, the WebTransport server MUST verify
    // the Origin header to ensure that the specified origin is allowed to access
    // the server in question. If the verification fails, the WebTransport server SHOULD
    // reply with status code 403 (Section 15.5.4 of [HTTP]). If all checks pass,
    // the WebTransport server MAY accept the session by replying with a 2xx series status code,
    // as defined in Section 15.3 of [HTTP].
    let connection = session_request.accept().await?;
    loop {
        let (stream_tx, stream_rx) = connection.accept_bi().await?;
        let mut reader = FramedRead::new(stream_rx, LengthDelimitedCodec::new());

        match reader.next().await {
            None => {
                error!("failed to get handshake request frame");
            },
            Some(Err(e)) => {
                error!("unexpected error: {e:?}");
            },
            Some(Ok(bytes)) => match HandshakeRequestFrame::decode(&bytes) {
                Ok(frame) => {
                    let accept_tx_clone = accept_tx.clone();
                    spawn!(
                        async move {
                            if accept_tx_clone
                                .send((frame, (stream_tx, reader)))
                                .await
                                .is_err()
                            {
                                error!("failed to send new WebTransport bi-directional stream")
                            }
                        },
                        "HANDSHAKE: webtransport accept"
                    );
                },
                Err(e) => {
                    error!("failed to decode frame: {e:?}");
                },
            },
        }
    }
}
