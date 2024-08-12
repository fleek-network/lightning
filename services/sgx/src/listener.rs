use std::task::Poll;

use enclave_runner::usercalls::AsyncListener;
use futures::ready;
use tokio::net::UnixListener;
use tokio::sync::mpsc::Receiver;

use crate::connection::{Connection, ConnectionListener};
use crate::IPC_PATH;

/// Simple wrapper around the sdk channel for incoming connections,
/// which in turn implement async read + write. Used to expose request
/// connections to the service.
pub struct RequestListener {
    rx: Receiver<std::io::Result<Connection>>,
}

impl RequestListener {
    pub async fn bind() -> Self {
        let listener = UnixListener::bind(IPC_PATH.join("conn"))
            .expect("failed to bind to connection socket listener");
        let ConnectionListener { rx } = ConnectionListener::new(listener);
        Self { rx }
    }
}

impl AsyncListener for RequestListener {
    fn poll_accept(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
        _local_addr: Option<&mut String>,
        // TODO: set client pubkey?
        _peer_addr: Option<&mut String>,
    ) -> std::task::Poll<tokio::io::Result<Option<Box<dyn enclave_runner::usercalls::AsyncStream>>>>
    {
        let res = ready!(self.rx.poll_recv(cx));
        Poll::Ready(res.transpose().map(|v| v.map(|c| Box::new(c) as _)))
    }
}
