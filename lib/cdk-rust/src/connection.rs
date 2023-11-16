use anyhow::Result;
use bytes::Bytes;
use fleek_crypto::{ClientPublicKey, ClientSignature};

use crate::context::Context;
use crate::mode::{ModeSetting, PrimaryMode, SecondaryMode};
use crate::schema::HandshakeRequestFrame;
use crate::transport::{Transport, TransportStream};

pub async fn connect<T: Transport>(transport: T, ctx: Context) -> Result<Connection<T>> {
    let mut stream = transport.connect().await?;

    match ctx.mode() {
        ModeSetting::Primary(setting) => {
            start_handshake::<T>(&mut stream, setting, *ctx.pk()).await?
        },
        ModeSetting::Secondary(setting) => join_connection::<T>(&mut stream, setting).await?,
    }

    Ok(Connection { inner: stream })
}

async fn start_handshake<T: Transport>(
    stream: &mut T::Stream,
    setting: &PrimaryMode,
    pk: ClientPublicKey,
) -> Result<()> {
    let frame = HandshakeRequestFrame::Handshake {
        retry: None,
        service: setting.service_id,
        pk,
        // Todo: Create signature.
        pop: ClientSignature([0; 48]),
    }
    .encode();
    stream.send(frame.as_ref()).await?;

    // Todo: Complete HANDSHAKE.

    Ok(())
}

async fn join_connection<T: Transport>(
    stream: &mut T::Stream,
    setting: &SecondaryMode,
) -> Result<()> {
    let frame = HandshakeRequestFrame::JoinRequest {
        access_token: setting.access_token,
    }
    .encode();
    stream.send(frame.as_ref()).await?;

    // Todo: Complete JOIN.

    Ok(())
}

pub struct Connection<T: Transport> {
    inner: T::Stream,
}

impl<T: Transport> Connection<T> {
    pub async fn send(&self, data: &[u8], _: usize) -> Result<()> {
        // Todo: More to do here.
        self.inner.send(data).await
    }

    pub async fn receive(&mut self) -> Result<Bytes> {
        // Todo: More to do here.
        self.inner.recv().await
    }
}
