use std::marker::PhantomData;

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
        // TODO
        expiry: 0,
        nonce: 0,
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

pub struct Connector<C, T: Transport> {
    transport: T,
    ctx: Context,
    _marker: PhantomData<C>,
}

impl<M, T: Transport> Connector<M, T> {
    pub(crate) fn new(transport: T, ctx: Context) -> Self {
        Self {
            transport,
            ctx,
            _marker: PhantomData,
        }
    }
}

impl<T: Transport> Connector<PrimaryConnection<T>, T> {
    pub async fn connect(&self) -> Result<PrimaryConnection<T>> {
        let (sender, receiver) = connect(&self.transport, &self.ctx).await?;
        let inner = InnerConnection {
            sender: Sender { inner: sender },
            receiver: Receiver { inner: receiver },
        };
        Ok(PrimaryConnection { inner })
    }
}

impl<T: Transport> Connector<SecondaryConnection<T>, T> {
    pub async fn connect(&self) -> Result<SecondaryConnection<T>> {
        let (sender, receiver) = connect(&self.transport, &self.ctx).await?;
        let inner = InnerConnection {
            sender: Sender { inner: sender },
            receiver: Receiver { inner: receiver },
        };
        Ok(SecondaryConnection { inner })
    }
}

struct InnerConnection<T: Transport> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

// These primary and secondary connection objects
// allow us to restrict some operations that
// are only allowed for specific type of connection.
pub struct PrimaryConnection<T: Transport> {
    inner: InnerConnection<T>,
}

impl<T: Transport> PrimaryConnection<T> {
    pub async fn request_access_token(&mut self, ttl: u64) -> Result<(u64, Box<[u8; 48]>)> {
        self.inner
            .sender
            .send(RequestFrame::AccessToken { ttl }.encode())
            .await?;
        match self.inner.receiver.recv().await.ok_or(anyhow::anyhow!(
            "failed to request an access token: transport connection closed"
        ))?? {
            ResponseFrame::AccessToken { ttl, access_token } => Ok((ttl, access_token)),
            ResponseFrame::Termination { reason } => {
                Err(anyhow::anyhow!("failed to get token: {reason:?}"))
            },
            ResponseFrame::ServicePayload { .. } => {
                // This assumes that the server will not send any frame until we do.
                // This assumption may not be true in the future.
                panic!("received an invalid frame: received a service payload frame");
            },
            _ => unimplemented!(),
        }
    }

    pub async fn extend_access_token(&mut self, _: usize) -> Result<()> {
        // Todo: discuss what is the response given this request.
        todo!()
    }

    pub fn split(self) -> (Sender<T>, Receiver<T>) {
        (self.inner.sender, self.inner.receiver)
    }
}

pub struct SecondaryConnection<T: Transport> {
    inner: InnerConnection<T>,
}

impl<T: Transport> SecondaryConnection<T> {
    pub fn split(self) -> (Sender<T>, Receiver<T>) {
        (self.inner.sender, self.inner.receiver)
    }
}

pub struct Sender<T: Transport> {
    inner: T::Sender,
}

impl<T: Transport> Sender<T> {
    // Todo: should this be cancel-safe?
    /// Cancel safety: This method is not cancel-safe.
    pub async fn send(&mut self, data: Bytes) -> Result<()> {
        let serialized_frame = RequestFrame::ServicePayload { bytes: data }.encode();
        self.inner.send(serialized_frame.as_ref()).await
    }
}

pub struct Receiver<T: Transport> {
    inner: T::Receiver,
}

impl<T: Transport> Receiver<T> {
    /// Cancel safety: This method is cancel-safe.
    pub async fn recv(&mut self) -> Option<Result<ResponseFrame>> {
        Some(ResponseFrame::decode(self.inner.recv().await?.as_ref()))
    }
}
