//! Mostly copied from `lib/fn_sdk/src/connection.rs`

use std::collections::HashMap;
use std::pin::Pin;

use anyhow::Result;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use url::Url;

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

    /// Accept a new connection
    pub async fn accept(&mut self) -> std::io::Result<Connection> {
        self.rx
            .recv()
            .await
            .expect("Could not get new connection from the socket.")
    }
}

/// A client connection
pub struct Connection {
    pub stream: UnixStream,
    pub header: ConnectionHeader,
    // TODO: Should the wrapper have a debug assertion to ensure the correct
    //       number of bytes are written to the stream?
}

impl Connection {
    async fn new_with_info(mut stream: UnixStream) -> std::io::Result<Self> {
        let header = read_header(&mut stream)
            .await
            .ok_or(std::io::ErrorKind::Other)?;
        Ok(Self { stream, header })
    }

    /// Start writing a payload to the handshake server for a new payload to the client.
    /// This function *MUST* always be called with the exact number of bytes before writing
    /// data on the stream, otherwise behavior will be undefined.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tokio::io::{AsyncWrite, AsyncWriteExt};
    ///
    /// let listener = fn_sdk::ipc::conn_bind();
    /// let conn = listener.accept().await?;
    ///
    /// // Declare we're going to write a 12 byte payload
    /// conn.start_write(12).await?;
    ///
    /// // Write some data
    /// conn.write_u64(8).await?;
    /// conn.write_u32(16).await?;
    /// ```
    #[inline(always)]
    pub async fn start_write(&mut self, len: usize) -> std::io::Result<()> {
        self.stream.write_u32(len as u32).await?;
        Ok(())
    }

    /// Write a full service payload to the handshake server, tHe entire buffer is written as one
    /// service payload in the proper length prepended encoding.
    ///
    /// # Cancel safety
    ///
    /// This method is not cancel safe. If it is used in a tokio select and another branch completes
    /// first data may be partially written.
    #[inline(always)]
    pub async fn write_payload(&mut self, payload: &[u8]) -> std::io::Result<()> {
        self.stream.write_u32(payload.len() as u32).await?;
        self.stream.write_all(payload).await?;
        Ok(())
    }

    /// Read a full payload from the handshake server.
    ///
    /// # Cancel safety
    ///
    /// This method is not cancel safe. On cancellation data may be partially read from the
    /// handshake, leading to unexpected behavior on the subsequent call.
    pub async fn read_payload(&mut self) -> Option<BytesMut> {
        read_length_delimited(&mut self.stream).await
    }

    /// Returns true if this connection is an HTTP request.
    #[inline(always)]
    pub fn is_http_request(&self) -> bool {
        matches!(
            self.header.transport_detail,
            TransportDetail::HttpRequest { .. }
        )
    }

    /// Returns true if this connection is a task request from a node.
    #[inline(always)]
    pub fn is_task_request(&self) -> bool {
        matches!(self.header.transport_detail, TransportDetail::Task { .. })
    }

    /// Returns true if this connection is an anonymous connection without a public key.
    #[inline(always)]
    pub fn is_anonymous(&self) -> bool {
        self.header.pk.is_none()
    }

    /// Shutdown the connection stream.
    pub async fn shutdown(&mut self) -> std::io::Result<()> {
        self.stream.shutdown().await
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
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}
