use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tracing::{error, trace};

use crate::schema::RequestFrame;
use crate::shutdown::ShutdownWaiter;
use crate::transports::{TransportReceiver, TransportSender};

pub struct Proxy<S: TransportSender, R: TransportReceiver> {
    tx: S,
    rx: R,
    socket: UnixStream,
    shutdown: ShutdownWaiter,
}

impl<S: TransportSender, R: TransportReceiver> Proxy<S, R> {
    pub fn new(tx: S, rx: R, socket: UnixStream, shutdown: ShutdownWaiter) -> Self {
        Self {
            tx,
            rx,
            socket,
            shutdown,
        }
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                error!("connection proxy encountered an error: {e}");
            }
        })
    }

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
                    trace!("sending out {n} bytes");
                    // TODO: OS like write api
                    self.tx.send(crate::schema::ResponseFrame::ServicePayload {
                        bytes: buffer[0..n].to_vec().into(),
                    });
                },
                _ = self.shutdown.wait_for_shutdown() => break Ok(()),
            }
        }
    }

    async fn handle_incoming(&mut self, req: RequestFrame) -> Result<()> {
        match req {
            // if we have incoming bytes, send it over the unix socket
            RequestFrame::ServicePayload { bytes } => self.socket.write_all(&bytes).await?,
            RequestFrame::AccessToken { .. } => todo!(),
            RequestFrame::ExtendAccessToken { .. } => todo!(),
            RequestFrame::DeliveryAcknowledgment {} => todo!(),
        }

        Ok(())
    }
}
