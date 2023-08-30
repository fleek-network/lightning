use std::ops::Deref;

use dashmap::DashMap;
use fleek_crypto::NodeSecretKey;
use lightning_interfaces::{ServiceHandleInterface, ServiceHandleProviderInterface};
use smallvec::SmallVec;
use triomphe::Arc;

use crate::schema;
use crate::transports::StaticSender;

#[derive(Clone)]
pub struct StateRef<P: ServiceHandleProviderInterface>(Arc<StateData<P>>);

impl<P: ServiceHandleProviderInterface> Deref for StateRef<P> {
    type Target = StateData<P>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The entire state of the handshake server. This should not be a generic over the transport
/// but rather we attach different transports into the state using the functionality defined
/// in transport_driver.rs file.
pub struct StateData<P: ServiceHandleProviderInterface> {
    sk: NodeSecretKey,
    pub provider: P,
    /// Map each access token to the info about that access token. aHash has better performance
    /// on arrays than fxHash.
    pub access_tokens: DashMap<[u8; 48], (), ahash::RandomState>,
    /// Map each connection to the information we have about it.
    pub connections: DashMap<u64, Connection, fxhash::FxBuildHasher>,
}

pub struct Connection {
    senders: SmallVec<[StaticSender; 2]>,
}

pub struct ProcessHandshakeResult<H: ServiceHandleInterface> {
    pub connection_id: u64,
    pub handle: H,
}

impl<P: ServiceHandleProviderInterface> StateData<P> {
    pub fn process_handshake(
        &self,
        _frame: schema::HandshakeRequestFrame,
        _sender: StaticSender,
    ) -> Option<ProcessHandshakeResult<P::Handle>> {
        todo!()
    }
}
