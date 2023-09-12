use std::io::Error;

use anyhow::Result;
use bytes::BytesMut;
use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use wtransport::endpoint::{IncomingSession, Server};
use wtransport::{Connection, Endpoint, RecvStream, SendStream};

use crate::schema::HandshakeRequestFrame;
use crate::shutdown::ShutdownWaiter;

pub type SendTx = FramedWrite<SendStream, LengthDelimitedCodec>;
pub type RecvRx = FramedRead<RecvStream, LengthDelimitedCodec>;

/// The execution context of the WebTransport server.
pub struct Context {
    pub endpoint: Endpoint<Server>,
    pub conn_tx: Sender<(HandshakeRequestFrame, (SendTx, RecvRx))>,
    pub shutdown: ShutdownWaiter,
}

pub async fn main_loop(ctx: Context) {
    loop {
        tokio::select! {
            incoming = ctx.endpoint.accept() => {
                let conn_tx = ctx.conn_tx.clone();
                tokio::spawn(async move  {
                    if let Err(e) = handle_incoming_session(incoming, conn_tx).await {
                        log::error!("failed to handle incoming WebTransport session: {e:?}");
                    }
                });
            }
            _ = ctx.shutdown.wait_for_shutdown() => {
                log::info!("shutting down WebTransport server");
                break;
            }
        }
    }
}

pub async fn handle_incoming_session(
    incoming: IncomingSession,
    accept_tx: Sender<(HandshakeRequestFrame, (SendTx, RecvRx))>,
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
        let writer = FramedWrite::new(stream_tx, LengthDelimitedCodec::new());
        let mut reader = FramedRead::new(stream_rx, LengthDelimitedCodec::new());

        match reader.next().await {
            None => {
                log::error!("failed to get handshake request frame");
            },
            Some(Err(e)) => {
                log::error!("unexpected error: {e:?}");
            },
            Some(Ok(bytes)) => match HandshakeRequestFrame::decode(&bytes) {
                Ok(frame) => {
                    let accept_tx_clone = accept_tx.clone();
                    tokio::spawn(async move {
                        if accept_tx_clone
                            .send((frame, (writer, reader)))
                            .await
                            .is_err()
                        {
                            log::error!("failed to send new WebTransport bi-directional stream")
                        }
                    });
                },
                Err(e) => {
                    log::error!("failed to decode frame: {e:?}");
                },
            },
        }
    }
}
