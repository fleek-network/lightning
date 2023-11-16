use anyhow::Result;
use arrayref::array_ref;
use bytes::{Buf, Bytes};
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
}

impl<S: TransportSender, R: TransportReceiver> Proxy<S, R> {
    pub fn new(tx: S, rx: R, socket: UnixStream, shutdown: ShutdownWaiter) -> Self {
        Self {
            tx,
            rx,
            socket,
            current_write: 0,
            shutdown,
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
        let mut buffer = [0; 4 << 10];
        loop {
            tokio::select! {
                // if we have an incoming payload, handle it
                res = self.rx.recv() => match res {
                    Some(req) => self.handle_incoming(req).await?,
                    None => break Ok(()),
                },
                // If we read any bytes from the socket, send it over the transport
                Ok(n) = self.socket.read(&mut buffer) => {
                    if n == 0 {
                        break Ok(())
                    }
                    self.handle_socket_bytes(buffer[0..n].to_vec().into()).await?;
                },
                _ = self.shutdown.wait_for_shutdown() => break Ok(()),
            }
        }
    }

    /// Handle outgoing bytes from the service socket
    async fn handle_socket_bytes(&mut self, mut bytes: Bytes) -> Result<()> {
        while !bytes.is_empty() {
            if self.current_write > 0 {
                // Send bytes to the transport
                let mut bytes = bytes.split_to(self.current_write.min(bytes.len()));
                while !bytes.is_empty() {
                    let sent = self.tx.write(&bytes)?;
                    bytes.advance(sent);
                }
            } else {
                // Send delimiter to the transport
                let bytes = bytes.split_to(4);
                let len = u32::from_be_bytes(*array_ref![bytes, 0, 4]);
                self.tx.start_write(len as usize);
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
