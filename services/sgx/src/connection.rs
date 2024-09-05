//! Mostly copied from `lib/fn_sdk/src/connection.rs`

use std::collections::HashMap;
use std::pin::Pin;
use std::task::Poll;

use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use enclave_runner::usercalls::AsyncListener;
use futures::ready;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use url::Url;

use crate::IPC_PATH;

/// A utility function to read a U32 big-endian length delimited payload from an async reader.
///
/// Returns `None` if the stream is exhausted before the promised number of bytes is read.
pub async fn read_length_delimited<R>(reader: &mut R) -> Option<BytesMut>
where
    R: AsyncRead + Unpin,
{
    let mut size = [0; 4];
    // really unnecessary.
    let mut i = 0;
    while i < 4 {
        match reader.read(&mut size[i..]).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => {
                i += n;
            },
        }
    }
    let size = u32::from_be_bytes(size) as usize;
    // now let's read `size` bytes.
    let mut buffer = BytesMut::with_capacity(size);
    while buffer.len() < size {
        match reader.read_buf(&mut buffer).await {
            Ok(0) | Err(_) => return None,
            Ok(_) => {},
        }
    }
    debug_assert_eq!(buffer.len(), size);
    Some(buffer)
}

/// Client public key, mirrors type found in `fleek_crypto`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPublicKey(#[serde(with = "BigArray")] pub [u8; 96]);

/// The header of this connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionHeader {
    pub pk: Option<ClientPublicKey>,
    pub transport_detail: TransportDetail,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum HttpMethod {
    GET,
    POST,
    HEAD,
    PUT,
    DELETE,
    PATCH,
    OPTIONS,
}

///  Response type used by a service to override the handshake http response fields when the
/// transport is HTTP
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub headers: Option<Vec<(String, Vec<String>)>>,
    pub status: Option<u16>,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HttpOverrides {
    pub headers: Option<Vec<(String, Vec<String>)>>,
    pub status: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransportDetail {
    HttpRequest {
        method: HttpMethod,
        url: Url,
        header: HashMap<String, String>,
    },
    Task {
        depth: u8,
        payload: Bytes,
    },
    Other,
}

pub async fn read_header(stream: &mut UnixStream) -> Option<ConnectionHeader> {
    let buffer = read_length_delimited(stream).await?;
    serde_cbor::from_slice(&buffer).ok()
}

/// Listener for incoming connections
pub struct ConnectionListener {
    pub rx: mpsc::Receiver<std::io::Result<Connection>>,
}

impl ConnectionListener {
    pub async fn bind() -> Self {
        let listener = UnixListener::bind(IPC_PATH.join("conn"))
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
        let header = read_header(&mut stream)
            .await
            .ok_or(std::io::ErrorKind::Other)?;
        let mut buffer = BytesMut::new();
        if let TransportDetail::Task { ref payload, .. } = header.transport_detail {
            buffer.put_u32(payload.len() as u32);
            buffer.put_slice(payload);
        }
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
