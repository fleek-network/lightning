use std::pin::Pin;

use fleek_crypto::ClientPublicKey;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// Listener for incoming connections
pub struct ConnectionListener {
    pub listener: UnixListener,
}

impl ConnectionListener {
    /// Accept a new connection
    pub async fn accept(&self) -> std::io::Result<Connection> {
        let (stream, _) = self.listener.accept().await?;
        Connection::new_with_info(stream).await
    }
}

/// A client connection
pub struct Connection {
    pub stream: UnixStream,
    pub _id: u64,
    pub _pubkey: ClientPublicKey,
    // TODO: Should the wrapper have a debug assertion to ensure the correct
    //       number of bytes are written to the stream?
}

impl Connection {
    async fn new_with_info(stream: UnixStream) -> std::io::Result<Self> {
        // TODO: read client information and connection id from a hello frame
        Ok(Self {
            stream,
            _id: 0,
            _pubkey: ClientPublicKey([0; 96]),
        })
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
}

impl AsyncWrite for Connection {
    #[inline(always)]
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    #[inline(always)]
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    #[inline(always)]
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
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
