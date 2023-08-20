use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::{
    ListenerInterface,
    SenderReceiver,
    SignerInterface,
    SyncQueryRunnerInterface,
};
use quinn::{Connection, ConnectionError, Endpoint, RecvStream, SendStream};
use tokio::sync::mpsc;

use crate::connection::RegisterEvent;
use crate::pool::{ConnectionPool, ScopeHandle};
use crate::receiver::Receiver;
use crate::sender::Sender;

pub struct Listener<T> {
    register_tx: mpsc::Sender<RegisterEvent>,
    connection_event_rx: mpsc::Receiver<(NodePublicKey, SendStream, RecvStream)>,
    active_scope: Arc<DashMap<ServiceScope, ScopeHandle>>,
    _marker: PhantomData<T>,
}

impl<T> Listener<T> {
    pub fn new(
        register_tx: mpsc::Sender<RegisterEvent>,
        connection_event_rx: mpsc::Receiver<(NodePublicKey, SendStream, RecvStream)>,
        active_scope: Arc<DashMap<ServiceScope, ScopeHandle>>,
    ) -> Self {
        Self {
            register_tx,
            connection_event_rx,
            active_scope,
            _marker: PhantomData::default(),
        }
    }
}

#[async_trait]
impl<T> ListenerInterface<T> for Listener<T>
where
    T: LightningMessage,
{
    type Sender = Sender<T>;
    type Receiver = Receiver<T>;
    async fn accept(&mut self) -> Option<(Self::Sender, Self::Receiver)> {
        let (peer, tx, rx) = self.connection_event_rx.recv().await?;
        Some((Sender::new(tx, peer), Receiver::new(rx, peer)))
    }
}
