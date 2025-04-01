//! Mostly copied from `lib/fn_sdk/src/connection.rs`

use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use enclave_runner::usercalls::AsyncListener;
use fn_sdk::header::{read_header, TransportDetail};
use futures::ready;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

/// Listener for incoming connections
pub struct ConnectionListener {
    pub rx: mpsc::Receiver<std::io::Result<Connection>>,
}

impl ConnectionListener {
    pub async fn bind() -> Self {
        let listener =
            UnixListener::bind(PathBuf::from(std::env::var("IPC_PATH").unwrap()).join("conn"))
                .expect("failed to bind to connection socket listener");
        let ConnectionListener { rx } = ConnectionListener::new(listener);
        Self { rx }
    }

    /// Create a new connection listener from the provided unix socket.
    pub fn new(listener: UnixListener) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let this = Self { rx };

        // Since creation of a connection is a task that awaits at two different points (1-
        // Accepting new streams. 2- Reading the connection header.) we can't simply await
        // the two in the accept function because otherwise the `accept` would not be cancel
        // safe and a slow header coming in would block the progress of other connections.
        // So we use a channel to send the accepted connections after getting the header to
        // the caller of `accept`.
        tokio::spawn(async move {
            loop {
                let maybe_stream = listener.accept().await.map(|(v, _)| v);
                let stream = match maybe_stream {
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    },
                    Ok(s) => s,
                };

                let tx = tx.clone();
                tokio::spawn(async move {
                    let conn = Connection::new_with_info(stream).await;
                    let _ = tx.send(conn).await;
                });
            }
        });

        this
    }
}

impl AsyncListener for ConnectionListener {
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

/// A client connection
pub struct Connection {
    pub stream: UnixStream,
    buffer: BytesMut,
}

impl Connection {
    async fn new_with_info(mut stream: UnixStream) -> std::io::Result<Self> {
        //let header = read_header(&mut stream)
        //    .await
        //    .ok_or(std::io::ErrorKind::Other)?;

        let mut buffer = BytesMut::new();
        //if let TransportDetail::Task { ref payload, .. } = header.transport_detail {
        //    buffer.put_u32(payload.len() as u32);
        //    buffer.put_slice(payload);
        //}

        Ok(Self { stream, buffer })
    }
}

impl AsyncWrite for Connection {
    #[inline(always)]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    #[inline(always)]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    #[inline(always)]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

impl AsyncRead for Connection {
    #[inline(always)]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if !self.buffer.is_empty() {
            let len = self.buffer.len().min(buf.remaining());
            buf.put_slice(&self.buffer.split_to(len));
            return std::task::Poll::Ready(Ok(()));
        }

        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}
