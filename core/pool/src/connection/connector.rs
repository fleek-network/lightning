use std::collections::hash_map::Entry;
use std::collections::HashMap;

use anyhow::{Error, Result};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, SyncQueryRunnerInterface};
use tokio_util::sync::CancellationToken;

use crate::endpoint::NodeAddress;
use crate::muxer::{ConnectionInterface, MuxerInterface};

pub struct Connector<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    /// Used for getting peer information from state.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Ongoing incoming and outgoing connection set-up tasks.
    connecting: FuturesUnordered<BoxFuture<'static, ConnectionResult<M::Connection>>>,
    /// Pending dialing tasks.
    pending_dial: HashMap<NodeIndex, CancellationToken>,
}

impl<C, M> Connector<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    pub fn new(sync_query: c!(C::ApplicationInterface::SyncExecutor)) -> Self {
        Self {
            sync_query,
            connecting: FuturesUnordered::new(),
            pending_dial: HashMap::new(),
        }
    }

    #[inline]
    pub async fn advance(&mut self) -> Option<ConnectionResult<M::Connection>> {
        self.connecting.next().await
    }

    pub fn enqueue_dial_task(&mut self, address: NodeAddress, muxer: M) -> Result<()> {
        if let Entry::Vacant(entry) = self.pending_dial.entry(address.index) {
            let cancel = CancellationToken::new();
            entry.insert(cancel.clone());

            let fut = async move {
                let index = address.index;
                let connect = || async {
                    muxer
                        .connect(address, "localhost")
                        .await?
                        .await
                        .map_err(Into::into)
                };
                let connection = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return ConnectionResult::Failed {
                        peer: Some(index),
                        error: anyhow::anyhow!("dial was cancelled")
                    },
                    connection = connect() => connection,
                };
                match connection {
                    Ok(conn) => ConnectionResult::Success {
                        incoming: false,
                        conn,
                        peer: index,
                    },
                    Err(e) => ConnectionResult::Failed {
                        peer: Some(index),
                        error: e,
                    },
                }
            }
            .boxed();

            self.connecting.push(fut);
        }

        Ok(())
    }

    pub fn handle_incoming_connection(&mut self, connecting: M::Connecting) {
        let sync_query = self.sync_query.clone();
        let fut = async move {
            let connect = || async move {
                let connection = connecting.await?;
                let key = connection.peer_identity().ok_or(anyhow::anyhow!(
                    "failed to get peer identity from successful TLS handshake"
                ))?;
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

    /// Clears the state.
    pub fn clear(&mut self) {
        self.connecting.clear();
        self.pending_dial.clear();
    }
}

pub enum ConnectionResult<C: ConnectionInterface> {
    Success {
        incoming: bool,
        conn: C,
        peer: NodeIndex,
    },
    Failed {
        // Always Some when connection attempt
        // was outgoing and None otherwise.
        peer: Option<NodeIndex>,
        error: Error,
    },
}
