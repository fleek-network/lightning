use std::task::Poll;

use enclave_runner::usercalls::AsyncListener;
use fn_sdk::connection::{Connection, ConnectionListener};
use futures::ready;
use tokio::sync::mpsc::Receiver;

pub struct RequestListener {
    rx: Receiver<std::io::Result<Connection>>,
}

impl RequestListener {
    pub async fn bind() -> Self {
        let ConnectionListener { rx } = fn_sdk::ipc::conn_bind().await;
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
        let res = ready!(self.rx.poll_recv(cx))
            .transpose()
            .map(|v| v.map(|c| Box::new(c) as _));
        Poll::Ready(res)
    }
}
