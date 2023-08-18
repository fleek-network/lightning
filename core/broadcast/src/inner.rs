#![allow(dead_code)]

use std::{collections::HashSet, sync::Arc};

use anyhow::{anyhow, Result};
use dashmap::{mapref::entry::Entry, DashMap};
use fleek_crypto::{NodePublicKey, NodeSecretKey, NodeSignature, PublicKey, SecretKey};
use lightning_interfaces::{
    infu_collection::{c, Collection},
    schema::broadcast::{BroadcastFrame, BroadcastMessage},
    types::Topic,
    Blake3Hash, ConnectionPoolInterface, ConnectorInterface, PoolReceiver, PoolSender,
    ReceiverInterface, SenderInterface, SignerInterface, ToDigest, TopologyInterface,
};

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
    /// PERF: This should be a cache. Maybe TTL = 24hrs, a simple LRU cache, or even prune this at
    /// each epoch
    messages: Arc<DashMap<Blake3Hash, Option<(BroadcastMessage, NodeSignature)>>>,
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
            messages: self.messages.clone(),
            channels: self.channels.clone(),
        }
    }
}

impl<C: Collection> BroadcastInner<C> {
    pub fn new<Signer: SignerInterface<C>>(
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
            messages: DashMap::new().into(),
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
            // TODO: Verify if anything else is needed to gracefully disconnect
            self.connections.remove(peer);
        }

        // connect to any new peers without an existing connection
        for peer in new_set {
            if !self.connections.contains_key(&peer) && self.node_secret_key.to_pk() != peer {
                if let Some((sender, receiver)) = self.connector.connect(&peer).await {
                    let inner = self.clone();
                    tokio::spawn(async move { inner.handle_connection(receiver, sender).await });
                } else {
                    eprintln!("failed to connect to {peer}");
                }
            }
        }

        // store the new topology
        *peers = new_peers;
    }

    /// broadcast a new message to all active connections
    pub async fn broadcast(&self, topic: Topic, payload: Vec<u8>) -> anyhow::Result<()> {
        // construct message
        let message = BroadcastMessage {
            topic,
            payload,
            originator: self.node_secret_key.to_pk(),
        };

        // compute the digest and sign it with the networking key
        let digest = message.to_digest();
        let signature = self.node_secret_key.sign(&digest);

        // insert the message into the map
        self.messages.insert(digest, Some((message, signature)));

        // construct an advertisement are there any helpers and send it to all current connection
        for mut conn in self.connections.iter_mut() {
            if let Err(e) = conn.send(BroadcastFrame::Advertise { digest }).await {
                // gracefully error when sending to multiple connections
                eprintln!("failed to send broadcast message: {e}");
            }
        }

        Ok(())
    }

    pub async fn handle_connection(
        &self,
        mut receiver: PoolReceiver<C, c![C::ConnectionPoolInterface], BroadcastFrame>,
        sender: PoolSender<C, c![C::ConnectionPoolInterface], BroadcastFrame>,
    ) -> anyhow::Result<()> {
        let pubkey = *receiver.pk();

        // insert sender to connections
        self.connections
            .insert(pubkey, BroadcastSender::new(sender));

        // read loop
        while let Some(message) = receiver.recv().await {
            match message {
                BroadcastFrame::Advertise { digest } => {
                    self.handle_advertise(&pubkey, &digest).await?
                },
                BroadcastFrame::Want { digest } => self.handle_want(&pubkey, &digest).await?,
                BroadcastFrame::Message { message, signature } => {
                    self.handle_message(&pubkey, message, signature).await?
                },
            }
        }

        self.connections.remove(&pubkey);

        Ok(())
    }

    pub async fn handle_advertise(
        &self,
        pubkey: &NodePublicKey,
        digest: &Blake3Hash,
    ) -> anyhow::Result<()> {
        // check if we have the message already
        let Entry::Vacant(entry) = self.messages.entry(*digest) else {
            return Ok(());
        };
        // if not, send a want, and mark the digest as wanted
        entry.insert(None);
        self.connections
            .get_mut(pubkey)
            // TODO: Implement a fallback here. We should probably retain a queue to attempt to
            //       find new nodes to get the message from.
            .ok_or(anyhow!("connection not found while sending want"))?
            .send(BroadcastFrame::Want { digest: *digest })
            .await
    }

    pub async fn handle_want(
        &self,
        pubkey: &NodePublicKey,
        digest: &Blake3Hash,
    ) -> anyhow::Result<()> {
        // check if we have the message
        let (message, signature) = self
            .messages
            .get(digest)
            // these errors should never actually happen, unless someone is being funny
            .ok_or(anyhow!("recieved want but digest was unknown"))?
            .clone()
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

        // check if the digest is marked as wanted
        let Some(mut entry) = self.messages.get_mut(&digest) else {
            return Ok(())
        };

        // check if the signature is valid for the originator
        if !message.originator.verify(&signature, &digest) {
            return Err(anyhow!("invalid signature"));
        }

        // mark the node as seen for the content
        self.connections
            .get_mut(pubkey)
            .map(|mut c| c.seen.insert(digest));

        // send the content to the corresponding pubsub channel
        //

        if let Some(channel) = self.channels.get(&message.topic) {
            channel.send(message.payload.clone())?;
        }

        // store the content for future wants
        *entry = Some((message, signature));

        // Advertise the digest to other peers
        for mut conn in self
            .connections
            .iter_mut()
            .skip_while(|c| c.seen.contains(&digest))
        {
            if let Err(e) = conn.send(BroadcastFrame::Advertise { digest }).await {
                // don't return hard errors here, only log them
                eprintln!("error sending broadcast advertisement: {e}");
            }
        }

        Ok(())
    }
}
