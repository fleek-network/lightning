use std::ops::Deref;

use dashmap::DashMap;
use fleek_crypto::NodeSecretKey;
use lightning_interfaces::{ExecutorProviderInterface, ServiceHandleInterface};
use triomphe::Arc;

use crate::schema;
use crate::transports::StaticSender;

#[derive(Clone)]
pub struct StateRef<P: ExecutorProviderInterface>(Arc<StateData<P>>);

impl<P: ExecutorProviderInterface> Deref for StateRef<P> {
    type Target = StateData<P>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The entire state of the handshake server. This should not be a generic over the transport
/// but rather we attach different transports into the state using the functionality defined
/// in transport_driver.rs file.
pub struct StateData<P: ExecutorProviderInterface> {
    sk: NodeSecretKey,
    pub provider: P,
    /// Map each access token to the info about that access token. aHash has better performance
    /// on arrays than fxHash.
    pub access_tokens: DashMap<[u8; 48], (), ahash::RandomState>,
    /// Map each connection to the information we have about it.
    pub connections: DashMap<u64, Connection<P::Handle>, fxhash::FxBuildHasher>,
}

pub struct Connection<H: ServiceHandleInterface> {
    /// The two possible senders attached to this connection. The index
    /// is for permission. Look at [`TransportPermission`].
    senders: [Option<StaticSender>; 2],
    /// Cached handle to the service.
    handle: H,
}

pub struct ProcessHandshakeResult {
    pub connection_id: u64,
    pub perm: TransportPermission,
}

#[derive(Clone, Copy, Debug)]
pub enum TransportPermission {
    ClientKeyOwner = 0x00,
    AccessTokenUser = 0x01,
}

impl<P: ExecutorProviderInterface> StateData<P> {
    /// Process a handshake request and returns the assigned connection id as well as a handle
    /// to the service. A transport id is also returned in order to
    pub fn process_handshake(
        &self,
        _frame: schema::HandshakeRequestFrame,
        _sender: StaticSender,
    ) -> Option<ProcessHandshakeResult> {
        todo!()
    }

    /// Called by any of the transports connected to a trance
    pub fn on_request_frame(
        &self,
        _perm: TransportPermission,
        connection_id: u64,
        frame: schema::RequestFrame,
    ) {
        let connection = self.connections.get(&connection_id).unwrap();

        match frame {
            schema::RequestFrame::ServicePayload { bytes } => {
                connection.handle.message(fn_sdk::internal::OnMessageArgs {
                    connection_id,
                    payload: bytes.to_vec(),
                });
            },
            schema::RequestFrame::AccessToken { ttl: _ } => {
                todo!()
            },
            schema::RequestFrame::RefreshAccessToken { access_token: _ } => {
                todo!()
            },
            schema::RequestFrame::DeliveryAcknowledgment {} => {
                todo!()
            },
        }

        todo!()
    }

    pub fn on_transport_closed(&self, _perm: TransportPermission, _connection_id: u64) {
        todo!()
    }
}
