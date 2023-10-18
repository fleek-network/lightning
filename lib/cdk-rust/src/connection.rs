use fleek_crypto::{ClientPublicKey, ClientSignature};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinSet;

use crate::context::Context;
use crate::frame::{Request, Response};
use crate::mode::{ModeSetting, PrimaryMode, SecondaryMode};
use crate::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
use crate::transport::Transport;

pub async fn connect_and_drive<T: Transport>(
    transport: T,
    request_rx: Receiver<Request>,
    response_tx: Sender<Response>,
    ctx: Context,
) -> anyhow::Result<()> {
    let (mut tx, mut rx) = transport.connect().await?;

    match ctx.mode() {
        ModeSetting::Primary(setting) => {
            start_handshake::<T>((&mut tx, &mut rx), setting, *ctx.pk()).await?
        },
        ModeSetting::Secondary(setting) => {
            join_connection::<T>((&mut tx, &mut rx), setting).await?
        },
    }

    connection_loop::<T>((tx, rx), request_rx, response_tx).await
}

async fn start_handshake<T: Transport>(
    (tx, _): (&mut T::Sender, &mut T::Receiver),
    setting: &PrimaryMode,
    pk: ClientPublicKey,
) -> anyhow::Result<()> {
    tx.send(
        HandshakeRequestFrame::Handshake {
            retry: None,
            service: setting.service_id,
            pk,
            // Todo: Create signature.
            pop: ClientSignature([0; 48]),
        }
        .encode(),
    )
    .await?;

    // Todo: Complete HANDSHAKE.

    Ok(())
}

async fn join_connection<T: Transport>(
    (tx, _): (&mut T::Sender, &mut T::Receiver),
    setting: &SecondaryMode,
) -> anyhow::Result<()> {
    tx.send(
        HandshakeRequestFrame::JoinRequest {
            access_token: setting.access_token,
        }
        .encode(),
    )
    .await?;

    // Todo: Complete JOIN.

    Ok(())
}

async fn connection_loop<T: Transport>(
    (mut tx, mut rx): (T::Sender, T::Receiver),
    mut request_rx: Receiver<Request>,
    response_tx: Sender<Response>,
) -> anyhow::Result<()> {
    // We spawn a separate task because the transport sender is not cloneable.
    let mut sender_task = JoinSet::new();
    sender_task.spawn(async move {
        while let Some(request) = request_rx.recv().await {
            if let Err(e) = tx.send(RequestFrame::from(request).encode()).await {
                log::error!("failed to send request frame: {e:?}");
                break;
            }
        }
    });

    loop {
        tokio::select! {
            bytes = rx.next() => {
                let Some(bytes) = bytes else {
                    break;
                };

                let response_event_tx = response_tx.clone();
                tokio::spawn(async move {
                    match ResponseFrame::decode(bytes.as_ref()) {
                        Ok(frame) => {
                            let _ = response_event_tx.send(frame).await;
                        }
                        Err(e) => {
                            log::error!("invalid response frame: {e:?}");
                        }
                    }
                });
            }
            _ = sender_task.join_next() => {
                // The sending task is the only one in the join set.
                // If that finishes, there is nothing we can do so we return.
                break;
            }
        }
    }

    Ok(())
}
