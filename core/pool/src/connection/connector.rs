use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Error, Result};
use fleek_crypto::NodeSecretKey;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, SyncQueryRunnerInterface};
use quinn::{ClientConfig, Connecting, Connection, Endpoint};
use rustls::Certificate;
use tokio_util::sync::CancellationToken;

use crate::endpoint::NodeAddress;
use crate::tls;

pub struct Connector<C: Collection> {
    /// Used for getting peer information from state.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Ongoing incoming and outgoing connection set-up tasks.
    connecting: FuturesUnordered<BoxFuture<'static, ConnectionResult>>,
    /// Pending dialing tasks.
    pending_dial: HashMap<NodeIndex, CancellationToken>,
    /// Node secret key.
    sk: NodeSecretKey,
}

impl<C: Collection> Connector<C> {
    pub fn new(sync_query: c!(C::ApplicationInterface::SyncExecutor), sk: NodeSecretKey) -> Self {
        Self {
            sync_query,
            connecting: FuturesUnordered::new(),
            pending_dial: HashMap::new(),
            sk,
        }
    }

    #[inline]
    pub async fn advance(&mut self) -> Option<ConnectionResult> {
        self.connecting.next().await
    }

    pub fn enqueue_dial_task(&mut self, address: NodeAddress, endpoint: Endpoint) -> Result<()> {
        let cancel = CancellationToken::new();

        self.pending_dial.insert(address.index, cancel.clone());

        let tls_config = tls::make_client_config(&self.sk, Some(address.pk))?;
        let fut = async move {
            let client_config = ClientConfig::new(Arc::new(tls_config));
            let connect = || async move {
                endpoint
                    .connect_with(client_config, address.socket_address, "localhost")?
                    .await
                    .map_err(Into::into)
            };
            let connection = tokio::select! {
                biased;
                _ = cancel.cancelled() => return ConnectionResult::Failed {
                    peer: Some(address.index),
                    error: anyhow::anyhow!("dial was cancelled")
                },
                connection = connect() => connection,
            };
            match connection {
                Ok(conn) => ConnectionResult::Success {
                    incoming: false,
                    conn,
                    peer: address.index,
                },
                Err(e) => ConnectionResult::Failed {
                    peer: Some(address.index),
                    error: e,
                },
            }
        }
        .boxed();

        self.connecting.push(fut);

        Ok(())
    }

    pub fn handle_incoming_connection(&mut self, connecting: Connecting) {
        let sync_query = self.sync_query.clone();
        let fut = async move {
            let connect = || async move {
                let connection = connecting.await?;
                let key = match connection.peer_identity() {
                    None => {
                        anyhow::bail!("failed to get peer identity from successful TLS handshake")
                    },
                    Some(any) => {
                        let chain = any
                            .downcast::<Vec<Certificate>>()
                            .map_err(|_| anyhow::anyhow!("invalid peer certificate"))?;
                        let certificate = chain
                            .first()
                            .ok_or_else(|| anyhow::anyhow!("invalid certificate chain"))?;
                        tls::parse_unverified(certificate.as_ref())?.peer_pk()
                    },
                };
                Ok((key, connection))
            };

            match connect().await {
                Ok((key, conn)) => {
                    // Todo: does this prove that they're staked?
                    match sync_query.pubkey_to_index(key) {
                        Some(peer) => ConnectionResult::Success {
                            incoming: true,
                            conn,
                            peer,
                        },
                        None => ConnectionResult::Failed {
                            peer: None,
                            error: anyhow::anyhow!(
                                "rejecting connection: index not found for peer ({key:?}, {:?})",
                                conn.remote_address()
                            ),
                        },
                    }
                },
                Err(e) => ConnectionResult::Failed {
                    peer: None,
                    error: e,
                },
            }
        }
        .boxed();
        self.connecting.push(fut);
    }

    #[inline]
    pub fn cancel_dial(&mut self, peer: &NodeIndex) {
        if let Some(cancel) = self.pending_dial.remove(peer) {
            cancel.cancel();
        }
    }

    #[inline]
    pub fn remove_pending_dial(&mut self, peer: &NodeIndex) {
        self.pending_dial.remove(peer);
    }
}

pub enum ConnectionResult {
    Success {
        incoming: bool,
        conn: Connection,
        peer: NodeIndex,
    },
    Failed {
        // Always Some when connection attempt
        // was outgoing and None otherwise.
        peer: Option<NodeIndex>,
        error: Error,
    },
}
