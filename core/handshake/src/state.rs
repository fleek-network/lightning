use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use fleek_crypto::{ClientPublicKey, NodeSecretKey};
use lightning_interfaces::{ConnectionWork, ExecutorProviderInterface, ServiceHandleInterface};
use rand::{thread_rng, Rng};
use tracing::error;
use triomphe::Arc;

use crate::gc::{Gc, GcSender};
use crate::schema;
use crate::shutdown::ShutdownWaiter;
use crate::transports::{StaticSender, TransportSender};
use crate::utils::{now, split_access_token, to_access_token};

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
    pub fn new(
        gc_timeout: Duration,
        waiter: ShutdownWaiter,
        sk: NodeSecretKey,
        provider: P,
    ) -> Self {
        let gc = Gc::default();

        let reference = Self(Arc::new(StateData {
            sk,
            next_connection_id: AtomicU64::new(0),
            shutdown: waiter,
            provider,
            connections: Default::default(),
            gc_sender: gc.sender(),
        }));

        gc.spawn(reference.clone(), gc_timeout);

        reference
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
    pub gc_sender: GcSender,
}

pub struct AccessTokenInfo {
    pub connection_id: u64,
    pub expiry_time: u64,
}

/// A connection object contains all the information we hold for an active connection. An active
/// connection does not necessarily have active transports attached to it.
///
/// Since we allow transports to retry connecting to the same connection. But there can only ever
/// be 2 transports attached to a connection at any given time.
///
/// These two attached transports have different permissions on the connection, one is the primary
/// and the other we call secondary.
///
/// A primary connection is a user opening the connection initially using a [`ClientPublicKey`],
/// this user is usually capable of providing proof of deliveries. And can a have a balance in
/// the network.
///
/// After connection a primary, can handover the connection to the secondary client by providing
/// an access token to them.
///
/// A new connection can then be made by this access key, which will take over connection stream
/// when a secondary is available, all the payload sent from a service to a connection will go to
/// the secondary and not the primary.
///
/// The primary at that point can even drop the connection if it wishes to do so. The connection
/// timeout is not to be confused by access key expiry time.
///
/// If a connection does not have any attached transport after `keep alive` timeout, the connection
/// will be dropped, regardless of the expiry time over the access key.
pub struct Connection<H: ServiceHandleInterface> {
    /// Public key of the client who made the first connection. If we expect many
    /// connections from the same public key, this can be interned. Currently this
    /// field takes almost half of the connection struct size.
    pk: ClientPublicKey,
    /// The two possible senders attached to this connection. The index
    /// is for permission. Look at [`TransportPermission`].
    senders: [Option<StaticSender>; 2],
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
    /// The time the previous connection was established.
    last_connection_time: u64,
    /// Cached handle to the service.
    handle: H,
    /// Sequence id for outgoing payloads
    sequence_id: u16,
    /// Buffer for outgoing payloads
    payload_buffer: BTreeMap<u16, Vec<u8>>,
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
            randomizer,
            expiry_time: 0,
            last_connection_time: now(),
            handle: handle.clone(),
            sequence_id: 0,
            payload_buffer: BTreeMap::new(),
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
        connection.last_connection_time = now();

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
        connection.last_connection_time = now();

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

        // TODO(qti3e): send handshake response.
    }

    fn handle_payload_frame(&self, _perm: TransportPermission, connection_id: u64, bytes: Bytes) {
        let Some(connection) = self.connections.get(&connection_id) else {
            error!("payload for connection '{connection_id}' dropped.");
            return;
        };

        // TODO(qti3e): Should we disallow primary to send payload to a service after handover.

        // Send payloads directly to the client.
        connection.handle.message(fn_sdk::internal::OnMessageArgs {
            connection_id,
            payload: bytes.to_vec(),
        });
    }

    fn handle_access_token_frame(&self, perm: TransportPermission, connection_id: u64, ttl: u64) {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            error!("AccessToken frame for connection '{connection_id}' dropped.");
            return;
        };

        if perm != TransportPermission::Primary {
            let Some(sender) = connection.senders[perm as usize].take() else {
                return;
            };

            sender.terminate(schema::TerminationReason::WrongPermssion);
            return;
        }

        let now = now();
        connection.expiry_time = now.saturating_add(ttl);
        // TODO(qti3e): Maybe we need to support max TTL as config.

        let ttl = connection.expiry_time - now; // compute the agreed upon TTL.
        let access_token = to_access_token(connection_id, &connection.randomizer);

        let Some(sender) = &mut connection.senders[0] else {
            // In the unlikely case that the sender does not exists (not totally impossible)
            // just return without sending the response frame back.
            return;
        };

        sender.send(schema::ResponseFrame::AccessToken {
            ttl,
            access_token: Box::new(access_token),
        });
    }

    fn handle_extend_access_token_frame(
        &self,
        perm: TransportPermission,
        connection_id: u64,
        ttl: u64,
    ) {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            error!("ExtendAccessToken frame for connection '{connection_id}' dropped.");
            return;
        };

        if perm != TransportPermission::Primary {
            let Some(sender) = connection.senders[perm as usize].take() else {
                return;
            };

            sender.terminate(schema::TerminationReason::WrongPermssion);
            return;
        }

        connection.expiry_time = now().saturating_add(ttl);
    }

    fn handle_delivery_acknowledgment_frame(&self, _perm: TransportPermission, _cid: u64) {
        error!("TODO: Delivery Acknowledgment");
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
            schema::RequestFrame::ExtendAccessToken { ttl } => {
                self.handle_extend_access_token_frame(perm, connection_id, ttl)
            },
            schema::RequestFrame::DeliveryAcknowledgment {} => {
                self.handle_delivery_acknowledgment_frame(perm, connection_id)
            },
        }
    }

    pub async fn on_transport_closed(&self, perm: TransportPermission, connection_id: u64) {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            return;
        };

        connection.senders[perm as usize] = None;

        if connection.is_empty() {
            self.gc_sender.insert(connection_id).await;
        }
    }

    fn handle_send_work(&self, connection_id: u64, mut sequence_id: u16, payload: Vec<u8>) {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            return;
        };

        if connection.sequence_id != sequence_id {
            // This is not the next message
            connection.payload_buffer.insert(sequence_id, payload);
            return;
        }

        // Collect any next payloads available to send
        sequence_id = sequence_id.wrapping_add(1);
        let mut messages = vec![payload];
        while let Some(payload) = connection.payload_buffer.remove(&sequence_id) {
            messages.push(payload);
            sequence_id = sequence_id.wrapping_add(1);
        }

        // Set the next sequence_id for the next call to this function
        connection.sequence_id = sequence_id;

        // Finally, send all the messages we have
        if let Some(sender) = connection.senders.iter_mut().flatten().last() {
            // Always send the service payload to the connection with lowest permission,
            // if there is no transport connected, we drop the data.
            for payload in messages {
                sender.send(schema::ResponseFrame::ServicePayload {
                    bytes: payload.into(),
                });
            }
        }
    }

    fn handle_close_work(&self, connection_id: u64) {
        let Some((_, connection)) = self.connections.remove(&connection_id) else {
            return;
        };

        for sender in connection.senders.into_iter().flatten() {
            sender.terminate(schema::TerminationReason::ServiceTerminated);
        }

        connection
            .handle
            .disconnected(fn_sdk::internal::OnDisconnectedArgs { connection_id });
    }

    pub fn process_work(&self, request: ConnectionWork) {
        match request {
            ConnectionWork::Send {
                connection_id,
                sequence_id,
                payload,
            } => {
                self.handle_send_work(connection_id, sequence_id, payload);
            },
            ConnectionWork::Close { connection_id } => {
                self.handle_close_work(connection_id);
            },
        }
    }

    #[inline]
    pub fn gc_tick(&self, disconnected_at: u64, connection_id: u64) {
        if let Entry::Occupied(entry) = self.connections.entry(connection_id) {
            let connection = entry.get();

            // This is not really reliable given we use ms precision. We should use versions.
            // Increase version on each new connection. Let gc tick contain the version at which
            // gc was triggered and compare that.
            if connection.last_connection_time > disconnected_at {
                // Only remove the connection if there has not been a new connection
                // since the last disconnect that triggered GC.
                return;
            }

            let connection = entry.remove();
            connection
                .handle
                .disconnected(fn_sdk::internal::OnDisconnectedArgs { connection_id });
        }
    }
}

impl<H: ServiceHandleInterface> Connection<H> {
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        matches!(self.senders, [None, None])
    }
}
