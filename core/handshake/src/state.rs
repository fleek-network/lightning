use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use dashmap::DashMap;
use fleek_crypto::{ClientPublicKey, NodeSecretKey};
use lightning_interfaces::{ConnectionWork, ExecutorProviderInterface, ServiceHandleInterface};
use rand::{thread_rng, Rng};
use triomphe::Arc;

use crate::schema;
use crate::shutdown::ShutdownWaiter;
use crate::transports::{StaticSender, TransportSender};

#[derive(Clone)]
pub struct StateRef<P: ExecutorProviderInterface>(Arc<StateData<P>>);

impl<P: ExecutorProviderInterface> Deref for StateRef<P> {
    type Target = StateData<P>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P: ExecutorProviderInterface> StateRef<P> {
    pub fn new(waiter: ShutdownWaiter, sk: NodeSecretKey, provider: P) -> Self {
        Self(Arc::new(StateData {
            sk,
            next_connection_id: AtomicU64::new(0),
            shutdown: waiter,
            provider,
            connections: Default::default(),
        }))
    }
}

/// The entire state of the handshake server. This should not be a generic over the transport
/// but rather we attach different transports into the state using the functionality defined
/// in transport_driver.rs file.
pub struct StateData<P: ExecutorProviderInterface> {
    sk: NodeSecretKey,
    next_connection_id: AtomicU64,
    pub shutdown: ShutdownWaiter,
    pub provider: P,
    /// Map each connection to the information we have about it.
    pub connections: DashMap<u64, Connection<P::Handle>, fxhash::FxBuildHasher>,
}

pub struct AccessTokenInfo {
    pub connection_id: u64,
    pub expiry_time: u64,
}

pub struct Connection<H: ServiceHandleInterface> {
    /// Public key of the client who made the first connection.
    pk: ClientPublicKey,
    /// The two possible senders attached to this connection. The index
    /// is for permission. Look at [`TransportPermission`].
    senders: [Option<StaticSender>; 2],
    /// Cached handle to the service.
    handle: H,
    /// The ephemeral access token generated for this connection object. The first
    /// 8 bytes of an access key are the connection id. The next 40 are random bytes.
    ///
    /// In future if this causes memory issue we can use encryption, to derive these
    /// bytes from the node's secret key signing some metadata.
    ///
    /// The randomizer always exists instead of being an `Option`.
    randomizer: [u8; 40],
    /// Expiry time of the access token. Set to `0` by default which would make any
    /// attempt to connect fail. Even if someone somehow knew the value of randomizer.
    expiry_time: u64,
}

pub struct ProcessHandshakeResult {
    pub connection_id: u64,
    pub perm: TransportPermission,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum TransportPermission {
    Primary = 0x00,
    Secondary = 0x01,
}

impl<P: ExecutorProviderInterface> StateData<P> {
    /// Handle a handshake request that is signed by a client key, and has an empty `retry`
    /// field. Suggesting that we need to create a new connection for the given public key.
    fn create_new_connection(
        &self,
        sender: StaticSender,
        pk: ClientPublicKey,
        service: u32,
    ) -> Option<ProcessHandshakeResult> {
        let Some(handle) = self.provider.get_service_handle(service) else {
            sender.terminate(schema::TerminationReason::InvalidService);
            return None;
        };

        // Get the next available connection id.
        let connection_id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);

        // TODO(qti3e):
        // Generate and fill the randomizer using the thread rng. Currently rand crate
        // is used to generate the randomizer. But we can explore a more audit friendly
        // implementation here.
        let mut randomizer = [0; 40];
        thread_rng().fill(randomizer.as_mut_slice());

        let connection = Connection {
            pk,
            senders: [Some(sender), None],
            handle: handle.clone(),
            randomizer,
            expiry_time: 0,
        };

        self.connections.insert(connection_id, connection);

        // Let the service know we got the connection.
        handle.connected(fn_sdk::internal::OnConnectedArgs {
            connection_id,
            client: pk,
        });

        Some(ProcessHandshakeResult {
            connection_id,
            perm: TransportPermission::Primary,
        })
    }

    /// Instead of creating a connection, join a connection as primary. This is initiated by
    /// a client key holder that performs a handshake with a `retry` filed that is provided.
    ///
    /// The service id is only used as sanity check over the connection retry request, and must
    /// be equal to the service id that we already have for the connection. It does not modify
    /// the service a connection is attached to. That data is immutable.
    fn join_connection_primary(
        &self,
        sender: StaticSender,
        pk: ClientPublicKey,
        connection_id: u64,
        service_id: u32,
    ) -> Option<ProcessHandshakeResult> {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            sender.terminate(schema::TerminationReason::InvalidHandshake);
            return None;
        };

        if connection.pk != pk {
            sender.terminate(schema::TerminationReason::InvalidHandshake);
            return None;
        }

        if connection.handle.get_service_id() != service_id {
            sender.terminate(schema::TerminationReason::InvalidService);
            return None;
        }

        if connection.senders[0].is_some() {
            // The previous connection is still connected.
            sender.terminate(schema::TerminationReason::ConnectionInUse);
            return None;
        }

        connection.senders[0] = Some(sender);

        Some(ProcessHandshakeResult {
            connection_id,
            perm: TransportPermission::Primary,
        })
    }

    /// Join a connection using a access token.
    fn join_connection_secondary(
        &self,
        sender: StaticSender,
        access_token: [u8; 48],
    ) -> Option<ProcessHandshakeResult> {
        let (connection_id, randomizer) = split_access_token(&access_token);

        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            sender.terminate(schema::TerminationReason::InvalidToken);
            return None;
        };

        if connection.randomizer.as_ref() != randomizer {
            sender.terminate(schema::TerminationReason::InvalidToken);
            return None;
        }

        if connection.expiry_time <= now() {
            sender.terminate(schema::TerminationReason::InvalidToken);
            return None;
        }

        if connection.senders[1].is_some() {
            // The previous connection is still connected.
            sender.terminate(schema::TerminationReason::ConnectionInUse);
            return None;
        }

        connection.senders[1] = Some(sender);

        Some(ProcessHandshakeResult {
            connection_id,
            perm: TransportPermission::Secondary,
        })
    }

    /// Process a handshake request and returns the assigned connection id as well as a handle
    /// to the service.
    ///
    /// This function does not verify proof of possession sent. Right now the transport should to
    /// that.
    pub fn process_handshake(
        &self,
        frame: schema::HandshakeRequestFrame,
        sender: StaticSender,
    ) -> Option<ProcessHandshakeResult> {
        match frame {
            schema::HandshakeRequestFrame::Handshake {
                retry: Some(conn_id),
                service,
                pk,
                ..
            } => self.join_connection_primary(sender, pk, conn_id, service),
            schema::HandshakeRequestFrame::Handshake { pk, service, .. } => {
                self.create_new_connection(sender, pk, service)
            },
            schema::HandshakeRequestFrame::JoinRequest { access_token } => {
                self.join_connection_secondary(sender, access_token)
            },
        }
    }

    fn handle_payload_frame(&self, _perm: TransportPermission, connection_id: u64, bytes: Bytes) {
        let Some(connection) = self.connections.get(&connection_id) else {
            log::error!("payload for connection '{connection_id}' dropped.");
            return;
        };

        // TODO(qti3e): Should we disallow primary to send payload to a service after handover.

        // Send payloads directly to the client.
        connection.handle.message(fn_sdk::internal::OnMessageArgs {
            connection_id,
            payload: bytes.to_vec(),
        });
    }

    fn handle_access_token_frame(&self, perm: TransportPermission, connection_id: u64, _ttl: u64) {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            log::error!("AccessToken frame for connection '{connection_id}' dropped.");
            return;
        };

        if perm != TransportPermission::Primary {
            let Some(sender) = connection.senders[perm as usize].take() else {
                return;
            };

            sender.terminate(schema::TerminationReason::WrongPermssion);
            return;
        }

        todo!()
    }

    fn handle_refresh_access_token_frame(
        &self,
        _perm: TransportPermission,
        _cid: u64,
        _access_token: [u8; 48],
    ) {
        todo!()
    }

    fn handle_delivery_acknowledgment_frame(&self, _perm: TransportPermission, _cid: u64) {
        todo!()
    }

    /// Called by any of the transports connected to a trance
    pub fn on_request_frame(
        &self,
        perm: TransportPermission,
        connection_id: u64,
        frame: schema::RequestFrame,
    ) {
        match frame {
            schema::RequestFrame::ServicePayload { bytes } => {
                self.handle_payload_frame(perm, connection_id, bytes)
            },
            schema::RequestFrame::AccessToken { ttl } => {
                self.handle_access_token_frame(perm, connection_id, ttl)
            },
            schema::RequestFrame::RefreshAccessToken { access_token } => {
                self.handle_refresh_access_token_frame(perm, connection_id, *access_token)
            },
            schema::RequestFrame::DeliveryAcknowledgment {} => {
                self.handle_delivery_acknowledgment_frame(perm, connection_id)
            },
        }
    }

    pub fn on_transport_closed(&self, _perm: TransportPermission, _connection_id: u64) {
        todo!()
    }

    fn handle_send_work(&self, _connection_id: u64, _payload: Vec<u8>) {
        todo!()
    }

    fn handle_close_work(&self, _connection_id: u64) {
        todo!()
    }

    pub fn process_work(&self, request: ConnectionWork) {
        match request {
            ConnectionWork::Send {
                connection_id,
                payload,
            } => {
                self.handle_send_work(connection_id, payload);
            },
            ConnectionWork::Close { connection_id } => {
                self.handle_close_work(connection_id);
            },
        }
    }
}

#[inline(always)]
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Generate an access token from connection id and randomizer.
#[inline(always)]
fn to_access_token(connection_id: u64, randomizer: &[u8; 40]) -> [u8; 48] {
    let mut value = [0; 48];
    value[0..8].copy_from_slice(connection_id.to_be_bytes().as_slice());
    value[8..].copy_from_slice(randomizer);
    value
}

/// Split an access token to connection id and randomizer.
#[inline(always)]
fn split_access_token(token: &[u8; 48]) -> (u64, &[u8; 40]) {
    let connection_id_be = arrayref::array_ref!(token, 0, 8);
    let connection_id = u64::from_be_bytes(*connection_id_be);
    let randomizer = arrayref::array_ref!(token, 8, 40);
    (connection_id, randomizer)
}
