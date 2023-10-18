use fleek_crypto::ClientPublicKey;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::connection;
use crate::context::Context;
use crate::frame::{Request, Response};
use crate::mode::{Mode, ModeSetting, PrimaryMode, SecondaryMode};
use crate::transport::Transport;

/// Builds a client.
pub struct Builder<M, T> {
    mode: M,
    transport: Option<T>,
    pk: Option<ClientPublicKey>,
}

impl<M, T> Builder<M, T>
where
    M: Mode,
    T: Transport,
{
    pub fn pk(self, pk: ClientPublicKey) -> Builder<M, T> {
        Builder {
            mode: self.mode,
            transport: self.transport,
            pk: Some(pk),
        }
    }

    /// Set the transport of the client or driver.
    pub fn transport(self, transport: T) -> Builder<M, AttachedTransport<T>> {
        Builder {
            mode: self.mode,
            transport: Some(AttachedTransport(transport)),
            pk: self.pk,
        }
    }
}

impl<T: Transport> Builder<PrimaryMode, T> {
    /// Creates a builder for constructing a client or driver in primary mode.
    pub fn primary(client_secret_key: [u8; 32], service_id: u32) -> Builder<PrimaryMode, T> {
        Self {
            mode: PrimaryMode {
                _client_secret_key: client_secret_key,
                service_id,
            },
            transport: None,
            pk: None,
        }
    }
}

impl<T: Transport> Builder<PrimaryMode, AttachedTransport<T>> {
    pub fn build(self) -> (Sender<Request>, Receiver<Response>) {
        // This unwrap is safe because `transport()` is required (using the type system).
        let transport = self.transport.unwrap().0;
        let context = Context::new(
            ModeSetting::Primary(self.mode),
            // Todo: better default?
            self.pk.unwrap_or(ClientPublicKey([1; 96])),
        );
        let (request_tx, request_rx) = mpsc::channel(1024);
        let (response_tx, response_rx) = mpsc::channel(1024);
        tokio::spawn(connection::connect_and_drive::<T>(
            transport,
            request_rx,
            response_tx,
            context,
        ));
        (request_tx, response_rx)
    }
}

impl<T: Transport> Builder<SecondaryMode, T> {
    /// Creates a builder for constructing a client or driver in secondary mode.
    pub fn secondary(access_token: [u8; 48], node_pk: [u8; 32]) -> Builder<SecondaryMode, T> {
        Self {
            mode: SecondaryMode {
                access_token,
                _node_pk: node_pk,
            },
            transport: None,
            pk: None,
        }
    }
}

impl<T: Transport> Builder<SecondaryMode, AttachedTransport<T>> {
    pub fn build(self) -> (Sender<Request>, Receiver<Response>) {
        // This unwrap is safe because `transport()` because this method is only available
        // after attaching a transport.
        let transport = self.transport.unwrap().0;
        let context = Context::new(
            ModeSetting::Secondary(self.mode),
            // Todo: better default?
            self.pk.unwrap_or(ClientPublicKey([1; 96])),
        );
        let (request_tx, request_rx) = mpsc::channel(1024);
        let (response_tx, response_rx) = mpsc::channel(1024);
        tokio::spawn(connection::connect_and_drive::<T>(
            transport,
            request_rx,
            response_tx,
            context,
        ));
        (request_tx, response_rx)
    }
}

pub struct AttachedTransport<T>(T);
