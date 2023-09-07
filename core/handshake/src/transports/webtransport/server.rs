use anyhow::Result;
use tokio::sync::mpsc::Sender;
use wtransport::endpoint::{IncomingSession, Server};
use wtransport::{Connection, Endpoint};

use crate::shutdown::ShutdownWaiter;

pub async fn main_loop(
    endpoint: Endpoint<Server>,
    waiter: ShutdownWaiter,
    conn_sender: Sender<Connection>,
) {
    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let conn_tx = conn_sender.clone();
                tokio::spawn(async move  {
                    if let Err(e) = handle_incoming_session(incoming, conn_tx).await {
                        log::error!("failed to handle incoming WebTransport session");
                    }
                });
            }
            _ = waiter.wait_for_shutdown() => {
                log::info!("shutting down WebTransport server");
                break;
            }
        }
    }
}

pub async fn handle_incoming_session(
    incoming: IncomingSession,
    conn_tx: Sender<Connection>,
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
    conn_tx
        .send(connection)
        .await
        .map_err(|_| anyhow::anyhow!("failed to send new WebTransport session connection"))
}
