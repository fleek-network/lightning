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
    pub fn transport(self, transport: T) -> Self {
        Self {
            mode: self.mode,
            transport: Some(transport),
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

    /// Builds a client.
    pub fn client(self) -> Client {
        todo!()
    }

    /// Builds a driver.
    pub fn driver(self) {
        todo!()
    }
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

    /// Builds a client.
    pub fn client(self) {
        todo!()
    }

    /// Builds a driver.
    pub fn driver(self) {
        todo!()
    }
}

pub struct Client;
