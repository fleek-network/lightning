use anyhow::{bail, Result};
use fleek_crypto::{ClientPublicKey, ClientSignature};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::Receiver;

use crate::mode::ModeSetting;
use crate::schema::{HandshakeRequestFrame, ResponseFrame};
use crate::transport::Transport;

pub async fn drive_connection<T: Transport, D: Driver>(
    transport: T,
    mut driver: D,
    ctx: Context,
) -> Result<()> {
    let (mut tx, mut rx) = transport.connect().await?;

    if !handshake::<T>((&mut tx, &mut rx), &ctx).await? {
        bail!("handshake failed")
    }

    driver.on_connected();

    connection_loop::<T, D>((tx, rx), ctx, driver).await
}

async fn handshake<T: Transport>(
    (tx, rx): (&mut T::Sender, &mut T::Receiver),
    ctx: &Context,
) -> Result<bool> {
    let success = match &ctx.mode {
        ModeSetting::Primary(setting) => {
            start_handshake::<T>((tx, rx), setting.client_secret_key, setting.service_id).await?
        },
        ModeSetting::Secondary(setting) => {
            join_connection::<T>((tx, rx), setting.access_token, setting.node_pk).await?
        },
    };

    Ok(success)
}

async fn start_handshake<T: Transport>(
    (tx, _): (&mut T::Sender, &mut T::Receiver),
    _client_secret_key: [u8; 32],
    service_id: u32,
) -> Result<bool> {
    tx.send(
        HandshakeRequestFrame::Handshake {
            retry: None,
            service: service_id,
            pk: ClientPublicKey([1; 96]),
            pop: ClientSignature([2; 48]),
        }
        .encode(),
    )
    .await?;

    // Todo: Handle complete handshake.
    // let response = rx
    //     .next()
    //     .await
    //     .ok_or_else(|| anyhow::anyhow!("connection closed unexpectedly"))?;
    //
    // let _ = HandshakeResponse::decode(response.as_ref())?;

    Ok(true)
}

async fn join_connection<T: Transport>(
    _: (&mut T::Sender, &mut T::Receiver),
    _access_token: [u8; 48],
    _node_pk: [u8; 32],
) -> Result<bool> {
    todo!()
}

pub struct Context {
    mode: ModeSetting,
    pk: ClientPublicKey,
    service_id: u32,
    request_rx: Receiver<HandshakeRequestFrame>,
}

async fn connection_loop<T: Transport, D: Driver>(
    (mut tx, mut rx): (T::Sender, T::Receiver),
    mut ctx: Context,
    mut driver: D,
) -> Result<()> {
    while let Some(request) = ctx.request_rx.recv().await {
        tx.send(request.encode()).await?;
        let Some(bytes) = rx.next().await else {
          bail!("connection closed unexpectedly");
        };
        let frame = ResponseFrame::decode(bytes.as_ref())?;

        if let ResponseFrame::ServicePayload { bytes } = frame {
            driver.on_service_payload()
        }
    }

    driver.on_disconnected();

    Ok(())
}

pub trait Driver {
    fn on_connected(&mut self) {}

    fn on_disconnected(&mut self) {}

    fn on_service_payload(&mut self) {}
}
