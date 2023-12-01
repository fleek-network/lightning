use anyhow::Result;
use arrayref::array_ref;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tracing::error;

use crate::schema::RequestFrame;
use crate::shutdown::ShutdownWaiter;
use crate::transports::{TransportReceiver, TransportSender};

pub struct Proxy<S: TransportSender, R: TransportReceiver> {
    tx: S,
    rx: R,
    socket: UnixStream,
    current_write: usize,
    shutdown: ShutdownWaiter,
    socket_buffer: BytesMut,
}

impl<S: TransportSender, R: TransportReceiver> Proxy<S, R> {
    pub fn new(tx: S, rx: R, socket: UnixStream, shutdown: ShutdownWaiter) -> Self {
        Self {
            tx,
            rx,
            socket,
            current_write: 0,
            shutdown,
            socket_buffer: BytesMut::new(),
        }
    }

    /// Spawn the proxy task for the connection
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                error!("connection proxy encountered an error: {e}");
            }
        })
    }

    /// Main loop, handling incoming frames and outgoing bytes until the shutdown
    /// signal is received or an error occurs.
    async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                // if we have an incoming payload, handle it
                res = self.rx.recv() => match res {
                    Some(req) => self.handle_incoming(req).await?,
                    None => break Ok(()),
                },
                // If we read any bytes from the socket, send it over the transport
                res = self.socket.read_buf(&mut self.socket_buffer) => match res {
                    Ok(n) if n == 0 => break Ok(()),
                    Ok(_) => self.handle_socket_bytes().await?,
                    Err(_) => break Ok(()),
                },
                _ = self.shutdown.wait_for_shutdown() => break Ok(()),
            }
        }
    }

    /// Handle outgoing bytes from the service socket
    async fn handle_socket_bytes(&mut self) -> Result<()> {
        while !self.socket_buffer.is_empty() {
            if self.current_write > 0 {
                // write bytes to the transport
                let len = self.current_write.min(self.socket_buffer.len());
                let bytes = self.socket_buffer.split_to(len);
                self.current_write -= len;
                self.tx.write(&bytes)?;
            } else if self.socket_buffer.len() >= 4 {
                // read the payload delimiter
                let bytes = self.socket_buffer.split_to(4);
                let len = u32::from_be_bytes(*array_ref![bytes, 0, 4]) as usize;
                self.tx.start_write(len);
                self.current_write = len;
                self.socket_buffer.reserve(len);
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Handle incoming frames from the transport
    async fn handle_incoming(&mut self, req: RequestFrame) -> Result<()> {
        match req {
            // if we have incoming bytes, send it over the unix socket
            RequestFrame::ServicePayload { bytes } => {
                // write delimiter and payload
                self.socket.write_u32(bytes.len() as u32).await?;
                self.socket.write_all(&bytes).await?
            },
            RequestFrame::AccessToken { .. } => todo!(),
            RequestFrame::ExtendAccessToken { .. } => todo!(),
            RequestFrame::DeliveryAcknowledgment {} => todo!(),
        }

        Ok(())
    }
}
