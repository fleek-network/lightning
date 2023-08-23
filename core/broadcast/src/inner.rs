#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use fleek_crypto::{NodePublicKey, NodeSecretKey, NodeSignature, PublicKey, SecretKey};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::broadcast::{BroadcastFrame, BroadcastMessage};
use lightning_interfaces::types::Topic;
use lightning_interfaces::{
    Blake3Hash,
    ConnectionPoolInterface,
    ConnectorInterface,
    PoolReceiver,
    PoolSender,
    ReceiverInterface,
    SenderInterface,
    SignerInterface,
    ToDigest,
    TopologyInterface,
};
use mini_moka::sync::Cache;
use tracing::error;

use crate::config;

pub struct BroadcastSender<S> {
    writer: S,
    pub seen: HashSet<Blake3Hash>,
}

impl<S: SenderInterface<BroadcastFrame>> BroadcastSender<S> {
    pub fn new(writer: S) -> Self {
        Self {
            seen: HashSet::new(),
            writer,
        }
    }

    pub async fn send(&mut self, message: BroadcastFrame) -> Result<()> {
        self.writer
            .send(message)
            .await
            .then_some(())
            .ok_or(anyhow!("shutting down while sending message"))
    }
}

#[allow(clippy::type_complexity)]
pub struct BroadcastInner<C: Collection> {
    topology: c![C::TopologyInterface],
    node_secret_key: Arc<NodeSecretKey>,
    // Current topology peers
    peers: Arc<tokio::sync::RwLock<Arc<Vec<Vec<NodePublicKey>>>>>,
    connector: Arc<c![C::ConnectionPoolInterface::Connector<BroadcastFrame>]>,
    // connection map
    connections: Arc<
        DashMap<
            NodePublicKey,
            BroadcastSender<PoolSender<C, c![C::ConnectionPoolInterface], BroadcastFrame>>,
        >,
    >,
    // received messages + signatures
    message_cache: Cache<Blake3Hash, Option<(BroadcastMessage, NodeSignature)>>,
    // incoming channels for pubsub messages
    channels: Arc<DashMap<Topic, tokio::sync::broadcast::Sender<Vec<u8>>>>,
}

impl<C: Collection> Clone for BroadcastInner<C> {
    fn clone(&self) -> Self {
        Self {
            topology: self.topology.clone(),
            node_secret_key: self.node_secret_key.clone(),
            peers: self.peers.clone(),
            connector: self.connector.clone(),
            connections: self.connections.clone(),
            message_cache: self.message_cache.clone(),
            channels: self.channels.clone(),
        }
    }
}

impl<C: Collection> BroadcastInner<C> {
    pub fn new<Signer: SignerInterface<C>>(
        config: config::Config,
        topology: c![C::TopologyInterface],
        signer: &Signer,
        connector: c![C::ConnectionPoolInterface::Connector<BroadcastFrame>],
        channels: Arc<DashMap<Topic, tokio::sync::broadcast::Sender<Vec<u8>>>>,
    ) -> Self {
        let peers = tokio::sync::RwLock::new(topology.suggest_connections()).into();
        let (_, node_key) = signer.get_sk();
        Self {
            topology,
            peers,
            channels,
            node_secret_key: node_key.into(),
            connector: connector.into(),
            connections: DashMap::new().into(),
            message_cache: Cache::builder()
                .time_to_live(Duration::from_secs(config.time_to_live_mins * 60))
                .time_to_idle(Duration::from_secs(config.time_to_idle_mins * 60))
                .build(),
        }
    }

    /// Run and apply the current topology, dropping old connections and creating new ones.
    pub async fn apply_topology(&self) {
        let mut peers = self.peers.write().await;
        let new_peers = self.topology.suggest_connections();
        let old_set: HashSet<_> = peers.iter().flatten().cloned().collect();
        let new_set: HashSet<_> = new_peers.iter().flatten().cloned().collect();
        let difference: Vec<_> = old_set.difference(&new_set).collect();

        // remove old peers
        for peer in difference {
            // drop senders from the connection map
            // TODO: Verify if anything else is needed to gracefully close the connection
            self.connections.remove(peer);
        }

        // connect to any new peers without an existing connection
        let our_pk = self.node_secret_key.to_pk();
        for peer in new_set.iter().filter(|pk| *pk != &our_pk) {
            if !self.connections.contains_key(peer) {
                if let Some((sender, receiver)) = self.connector.connect(peer).await {
                    self.handle_connection_task(receiver, sender);
                } else {
                    error!("failed to connect to {peer}");
                }
            }
        }

        // store the new topology
        *peers = new_peers;
    }

    /// Spawn a task to broadcast a new message to all active connections
    pub fn broadcast_task(&self, topic: Topic, payload: Vec<u8>) {
        let inner = self.clone();
        tokio::spawn(async move {
            // construct the message
            let message = BroadcastMessage {
                topic,
                payload,
                originator: inner.node_secret_key.to_pk(),
            };

            // compute the digest and sign it with the networking key
            let digest = message.to_digest();
            let signature = inner.node_secret_key.sign(&digest);

            // insert the message into the message cache for future wants
            inner
                .message_cache
                .insert(digest, Some((message, signature)));

            // Send an advertisement to all current connections
            for mut conn in inner.connections.iter_mut() {
                if let Err(e) = conn.send(BroadcastFrame::Advertise { digest }).await {
                    // gracefully error when sending to multiple connections
                    error!("failed to send broadcast message: {e}");
                }
            }
        });
    }

    /// Spawn a task to handle a new connection
    pub fn handle_connection_task(
        &self,
        mut receiver: PoolReceiver<C, c![C::ConnectionPoolInterface], BroadcastFrame>,
        sender: PoolSender<C, c![C::ConnectionPoolInterface], BroadcastFrame>,
    ) {
        let inner = self.clone();
        tokio::spawn(async move {
            let pubkey = *receiver.pk();

            // insert sender to connections
            inner
                .connections
                .insert(pubkey, BroadcastSender::new(sender));

            // read loop
            while let Some(message) = receiver.recv().await {
                match message {
                    BroadcastFrame::Advertise { digest } => {
                        if let Err(e) = inner.handle_advertise(&pubkey, &digest).await {
                            error!("failed to handle advertisement: {e}");
                        };
                    },
                    BroadcastFrame::Want { digest } => {
                        if let Err(e) = inner.handle_want(&pubkey, &digest).await {
                            error!("failed to handle want: {e}");
                        }
                    },
                    BroadcastFrame::Message { message, signature } => {
                        if let Err(e) = inner.handle_message(&pubkey, message, signature).await {
                            error!("failed to handle message: {e}");
                        }
                    },
                }
            }

            inner.connections.remove(&pubkey);
        });
    }

    pub async fn handle_advertise(
        &self,
        pubkey: &NodePublicKey,
        digest: &Blake3Hash,
    ) -> anyhow::Result<()> {
        // check if we have the message already
        if self.message_cache.contains_key(digest) {
            return Ok(());
        }

        // send the message
        self.connections
            .get_mut(pubkey)
            .ok_or(anyhow!("connection not found while sending want"))?
            .send(BroadcastFrame::Want { digest: *digest })
            .await?;

        // mark the digest as wanted
        self.message_cache.insert(*digest, None);
        Ok(())
    }

    pub async fn handle_want(
        &self,
        pubkey: &NodePublicKey,
        digest: &Blake3Hash,
    ) -> anyhow::Result<()> {
        // check if we have the message
        let (message, signature) = self
            .message_cache
            .get(digest)
            // these errors will never actually happen, unless someone is misbehaving and sending
            // wants for invalid digests.
            .ok_or(anyhow!("recieved want but digest was unknown"))?
            .ok_or(anyhow!("recieved want but content was missing"))?;

        let Some(mut conn) = self.connections.get_mut(pubkey) else {
            return Ok(())
        };

        // if so, send it
        conn.send(BroadcastFrame::Message { message, signature })
            .await?;

        conn.seen.insert(*digest);

        Ok(())
    }

    pub async fn handle_message(
        &self,
        pubkey: &NodePublicKey,
        message: BroadcastMessage,
        signature: NodeSignature,
    ) -> anyhow::Result<()> {
        // compute message digest
        let digest = message.to_digest();

        // check if the digest is (at a minimum) marked as wanted
        if let Some(entry) = self.message_cache.get(&digest) {
            if entry.is_some() {
                return Ok(());
            }
        };

        // check if the signature is valid for the originator
        if !message.originator.verify(&signature, &digest) {
            return Err(anyhow!("invalid signature"));
        }

        // mark the node as seen for the content
        self.connections
            .get_mut(pubkey)
            .map(|mut c| c.seen.insert(digest));

        // send the content to the corresponding pubsub broadcast channel
        // TODO: validate the payload can be decoded, somehow
        if let Some(channel) = self.channels.get(&message.topic) {
            channel.send(message.payload.clone())?;
        }

        // store the content for future wants
        self.message_cache
            .insert(digest, Some((message, signature)));

        // Advertise the digest to other peers
        for mut conn in self
            .connections
            .iter_mut()
            .skip_while(|c| c.seen.contains(&digest))
        {
            if let Err(e) = conn.send(BroadcastFrame::Advertise { digest }).await {
                // don't return hard errors here, only log them
                error!("error sending broadcast advertisement: {e}");
            }
        }

        Ok(())
    }
}
