use anyhow::Result;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use quinn::Connection;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::{Framed, FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::endpoint::{Message, NodeAddress};

pub async fn start_driver(
    connection: Connection,
    mut message_rx: Receiver<Message>,
    event_tx: Sender<Message>,
    accept: bool,
) -> Result<()> {
    // Todo: If we stick with QUIC, we should use the stream more efficiently.
    let (tx, rx) = match accept {
        true => connection.accept_bi().await?,
        false => connection.open_bi().await?,
    };
    let mut writer = FramedWrite::new(tx, LengthDelimitedCodec::new());
    let mut reader = FramedRead::new(rx, LengthDelimitedCodec::new());
    loop {
        tokio::select! {
            outgoing = message_rx.recv() => {
                let message = match outgoing {
                    None => break,
                    Some(message) => message,
                };
                writer.send(Bytes::from(message)).await?;
            }
            incoming = reader.next() => {
                let message = match incoming {
                    None => break,
                    Some(message) => message?,
                };
                if event_tx.send(message.to_vec()).await.is_err() {
                    anyhow::bail!("failed to send incoming network event");
                }
            }
        }
    }
    Ok(())
}
