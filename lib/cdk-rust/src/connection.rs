use anyhow::bail;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::Receiver;

use crate::context::Context;
use crate::handle::RequestResponse;
use crate::mode::{ModeSetting, PrimaryMode, SecondaryMode};
use crate::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
use crate::transport::Transport;

pub async fn connect_and_drive<T: Transport>(
    transport: T,
    request_rx: Receiver<RequestResponse>,
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

    connection_loop::<T>((tx, rx), request_rx).await
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
    mut request_rx: Receiver<RequestResponse>,
) -> anyhow::Result<()> {
    while let Some(request) = request_rx.recv().await {
        // Todo: If (tx, rx) was an individual (QUIC) stream,
        // we could move this pair in a separate task and
        // avoid waiting.
        tx.send(RequestFrame::from(request.payload).encode())
            .await?;

        let Some(bytes) = rx.next().await else {
          bail!("connection closed unexpectedly");
        };

        tokio::spawn(async move {
            match ResponseFrame::decode(bytes.as_ref()) {
                Ok(response) => {
                    if request.respond.send(response).is_err() {
                        log::error!("failed to send response");
                    }
                },
                Err(e) => {
                    log::error!("failed to decode frame: {e:?}");
                },
            }
        });
    }

    Ok(())
}
