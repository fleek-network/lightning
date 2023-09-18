use crate::driver::Driver;
use crate::mode::{Mode, PrimaryMode, SecondaryMode};
use crate::transport::Transport;

/// Builds a client or driver.
pub struct Builder<M, T> {
    mode: M,
    transport: Option<T>,
}

impl<M, T> Builder<M, T>
where
    M: Mode,
    T: Transport,
{
    /// Set the transport of the client or driver.
    pub fn transport(self, transport: T) -> Builder<M, AttachedTransport<T>> {
        Builder {
            mode: self.mode,
            transport: Some(AttachedTransport(transport)),
        }
    }
}

impl<T: Transport> Builder<PrimaryMode, T> {
    /// Creates a builder for constructing a client or driver in primary mode.
    pub fn primary(client_secret_key: [u8; 32], service_id: u32) -> Builder<PrimaryMode, T> {
        Self {
            mode: PrimaryMode {
                client_secret_key,
                service_id,
            },
            transport: None,
        }
    }
}

impl<T: Transport> Builder<PrimaryMode, AttachedTransport<T>> {
    pub fn _drive<D: Driver>(self, _driver: D) {}
}

impl<T: Transport> Builder<SecondaryMode, T> {
    /// Creates a builder for constructing a client or driver in secondary mode.
    pub fn secondary(access_token: [u8; 48], node_pk: [u8; 32]) -> Builder<SecondaryMode, T> {
        Self {
            mode: SecondaryMode {
                access_token,
                node_pk,
            },
            transport: None,
        }
    }
}

impl<T: Transport> Builder<SecondaryMode, AttachedTransport<T>> {
    pub fn _drive<D: Driver>(self, _driver: D) {}
}

pub struct AttachedTransport<T>(T);
