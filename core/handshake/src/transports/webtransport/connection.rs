use std::io::Error;
use std::pin::Pin;
use std::task::Poll;

use anyhow::Result;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use wtransport::{Connection, RecvStream, SendStream};

pub async fn connection_loop(mut ctx: Context) -> Result<()> {
    // Todo: let's find a way to use multi-stream feature.
    let (stream_tx, stream_rx) = ctx.connection.accept_bi().await?;
    let mut frame = Framed::new(
        BiStream {
            tx: stream_tx,
            rx: stream_rx,
        },
        LengthDelimitedCodec::new(),
    );
    loop {
        tokio::select! {
            Some(data) = frame.next() => {
                match data {
                    Ok(data) => {
                        if ctx.data_tx.send(data.to_vec()).await.is_err() {
                            log::error!("failed to send incoming data");
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("failed to get frame: {e:?}");
                        break;
                    }
                }
            }
            data = ctx.data_rx.recv() => {
                let Some(data) = data else {
                  break;
                };
                if let Err(e) = frame.send(Bytes::from(data)).await {
                    log::error!("failed to write content into stream: {e:?}");
                    break;
                }

            }
        }
    }
    Ok(())
}

pub struct Context {
    connection: Connection,
    data_tx: Sender<Vec<u8>>,
    data_rx: Receiver<Vec<u8>>,
}

impl Context {
    pub fn new(
        connection: Connection,
        data_tx: Sender<Vec<u8>>,
        data_rx: Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            connection,
            data_tx,
            data_rx,
        }
    }
}

struct BiStream {
    tx: SendStream,
    rx: RecvStream,
}

impl AsyncRead for BiStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        AsyncRead::poll_read(Pin::new(&mut self.rx), cx, buf)
    }
}

impl AsyncWrite for BiStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::result::Result<usize, Error>> {
        AsyncWrite::poll_write(Pin::new(&mut self.tx), cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::result::Result<(), Error>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.tx), cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::result::Result<(), Error>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.tx), cx)
    }
}
