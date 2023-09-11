use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tokio_util::codec::{FramedRead, FramedWrite};
use wtransport::endpoint::{IncomingSession, Server};
use wtransport::{Connection, Endpoint, RecvStream, SendStream};

use crate::shutdown::ShutdownWaiter;

/// The execution context of the WebTransport server.
pub struct Context {
    pub endpoint: Endpoint<Server>,
    pub conn_tx: Sender<(SendStream, RecvStream)>,
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
    accept_tx: Sender<(SendStream, RecvStream)>,
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
        let accept_tx_clone = accept_tx.clone();
        tokio::spawn(async move {
            if accept_tx_clone.send((stream_tx, stream_rx)).await.is_err() {
                log::error!("failed to send new WebTransport bi-directional stream")
            }
        });
    }
}
