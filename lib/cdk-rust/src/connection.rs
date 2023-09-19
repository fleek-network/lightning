use std::borrow::Cow;

use anyhow::bail;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::Receiver;

use crate::driver::{Driver, Event, RequestResponse};
use crate::mode::{ModeSetting, PrimaryMode, SecondaryMode};
use crate::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
use crate::transport::Transport;

pub async fn connect_and_drive<T: Transport, D: Driver>(
    transport: T,
    mut driver: D,
    mut ctx: Context,
) -> anyhow::Result<()> {
    let (mut tx, mut rx) = transport.connect().await?;

    let success = handshake::<T>((&mut tx, &mut rx), &ctx).await?;

    driver.drive(Event::Connection { success }, &mut ctx);

    if success {
        bail!("handshake failed");
    }

    let _ = connection_loop::<T, D>((tx, rx), &mut ctx, &mut driver).await;

    // Todo: Add termination reason if applicable.
    driver.drive(Event::Disconnect { reason: None }, &mut ctx);

    Ok(())
}

async fn handshake<T: Transport>(
    (tx, rx): (&mut T::Sender, &mut T::Receiver),
    ctx: &Context,
) -> anyhow::Result<bool> {
    let success = match &ctx.mode {
        ModeSetting::Primary(setting) => start_handshake::<T>((tx, rx), setting, ctx.pk).await?,
        ModeSetting::Secondary(setting) => join_connection::<T>((tx, rx), setting).await?,
    };

    Ok(success)
}

async fn start_handshake<T: Transport>(
    (tx, _): (&mut T::Sender, &mut T::Receiver),
    setting: &PrimaryMode,
    pk: ClientPublicKey,
) -> anyhow::Result<bool> {
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

    Ok(true)
}

async fn join_connection<T: Transport>(
    (mut tx, _): (&mut T::Sender, &mut T::Receiver),
    setting: &SecondaryMode,
) -> anyhow::Result<bool> {
    tx.send(
        HandshakeRequestFrame::JoinRequest {
            access_token: setting.access_token,
        }
        .encode(),
    )
    .await?;

    // Todo: Complete JOIN.

    Ok(true)
}

pub struct Context {
    mode: ModeSetting,
    pk: ClientPublicKey,
    addr_rx: Receiver<RequestResponse>,
}

async fn connection_loop<T: Transport, D: Driver>(
    (mut tx, mut rx): (T::Sender, T::Receiver),
    mut ctx: &mut Context,
    driver: &mut D,
) -> anyhow::Result<()> {
    while let Some(request) = ctx.addr_rx.recv().await {
        // Todo: If (tx, rx) was a individual (QUIC) stream,
        // we could move this pair in a separate task and
        // avoid waiting.
        tx.send(RequestFrame::from(request.payload).encode())
            .await?;

        let Some(bytes) = rx.next().await else {
          bail!("connection closed unexpectedly");
        };

        driver.drive(
            Event::PayloadReceived {
                bytes: bytes.as_ref(),
            },
            &mut ctx,
        );

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
