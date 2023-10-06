use std::collections::hash_map::Entry;
use std::collections::HashMap;

use anyhow::{Error, Result};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use lightning_interfaces::types::NodeIndex;
use tokio_util::sync::CancellationToken;

use crate::endpoint::NodeAddress;
use crate::muxer::{ConnectionInterface, MuxerInterface};

pub struct Connector<M>
where
    M: MuxerInterface,
{
    /// Ongoing incoming and outgoing connection set-up tasks.
    connecting: FuturesUnordered<BoxFuture<'static, ConnectionResult<M::Connection>>>,
    /// Pending dialing tasks.
    pending_dial: HashMap<NodeIndex, CancellationToken>,
}

impl<M> Connector<M>
where
    M: MuxerInterface,
{
    pub fn new() -> Self {
        Self {
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
        let fut = async move {
            match connecting.await {
                Ok(conn) => ConnectionResult::Success {
                    incoming: true,
                    conn,
                },
                Err(e) => ConnectionResult::Failed {
                    peer: None,
                    error: e.into(),
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
    },
    Failed {
        // Always Some when connection attempt
        // was outgoing and None otherwise.
        peer: Option<NodeIndex>,
        error: Error,
    },
}
