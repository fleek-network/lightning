use anyhow::Result;
use bytes::Bytes;
use fleek_crypto::{ClientPublicKey, ClientSignature};

use crate::context::Context;
use crate::mode::{ModeSetting, PrimaryMode, SecondaryMode};
use crate::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
use crate::transport::{Transport, TransportReceiver, TransportSender};

pub async fn connect<T: Transport>(
    transport: &T,
    ctx: &Context,
) -> Result<(T::Sender, T::Receiver)> {
    let (mut sender, receiver) = transport.connect().await?;

    match ctx.mode() {
        ModeSetting::Primary(setting) => {
            start_handshake::<T>(&mut sender, setting, *ctx.pk()).await?
        },
        ModeSetting::Secondary(setting) => join_connection::<T>(&mut sender, setting).await?,
    }

    Ok((sender, receiver))
}

async fn start_handshake<T: Transport>(
    stream: &mut T::Sender,
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
    stream: &mut T::Sender,
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

pub struct Connector<T: Transport> {
    transport: T,
    ctx: Context,
}

impl<T: Transport> Connector<T> {
    pub(crate) fn new(transport: T, ctx: Context) -> Self {
        Self { transport, ctx }
    }
    pub async fn connect(&self) -> Result<(Sender<T>, Receiver<T>)> {
        let (sender, receiver) = connect(&self.transport, &self.ctx).await?;
        Ok((Sender { inner: sender }, Receiver { inner: receiver }))
    }
}

pub struct Sender<T: Transport> {
    inner: T::Sender,
}

impl<T: Transport> Sender<T> {
    pub async fn send(&mut self, data: Bytes) -> Result<()> {
        let serialized_frame = RequestFrame::ServicePayload { bytes: data }.encode();
        self.inner.send(serialized_frame.as_ref()).await
    }

    async fn _request_access_token(&mut self, _ttl: u64) {
        todo!()
    }

    async fn _extend_access_token(&mut self, _ttl: u64) {
        todo!()
    }
}

pub struct Receiver<T: Transport> {
    inner: T::Receiver,
}

impl<T: Transport> Receiver<T> {
    pub async fn recv(&mut self) -> Result<ResponseFrame> {
        ResponseFrame::decode(self.inner.recv().await?.as_ref())
    }
}
