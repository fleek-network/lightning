/*
   The purpose of the forwarder is to provide a socket that other processess can send signed transactions too. The forwarder
   will then forward the transaction to an active narwhal committee member, prefering its own worker if the node is currently
   on the committee. Retry logic should be done on the proccess that has the sender side of the socket.
*/
use std::{
    cmp,
    collections::{hash_map::Entry, HashMap},
};

use affair::AsyncWorker;
use anyhow::{bail, Result};
use async_trait::async_trait;
use draco_interfaces::{
    types::{Epoch, EpochInfo, NodeInfo, UpdateRequest},
    SyncQueryRunnerInterface,
};
use fastcrypto::bls12381::min_sig::BLS12381PublicKey;
use fleek_crypto::NodePublicKey;
use narwhal_types::{TransactionProto, TransactionsClient};
use rand::seq::SliceRandom;
use tonic::transport::channel::Channel;

const TARGETED_CONNECTION_NUM: usize = 10;

pub struct Forwarder<Q: SyncQueryRunnerInterface> {
    /// Query runner used to read application state
    query_runner: Q,
    /// The public key of this node
    primary_name: NodePublicKey,
    /// Current Epoch
    epoch: Epoch,
    /// List of the committee members
    committee: Vec<NodeInfo>,
    /// Cursor that keeps track of where we are in the committee vec, so we try connecting to new
    /// nodes and loop back if connections fail
    cursor: usize,
    /// maximum number of connections we will try and make to other workers, this epoch
    max_connections: usize,
    /// When our connections drop under this number we will try and connect to new workers
    min_connections: usize,
    /// Open connections to committee workers
    active_connections: HashMap<usize, TransactionsClient<Channel>>,
}

impl<Q: SyncQueryRunnerInterface> Forwarder<Q> {
    pub fn new(query_runner: Q, primary_name: BLS12381PublicKey) -> Self {
        Self {
            query_runner,
            primary_name: primary_name.into(),
            epoch: 0,
            committee: Vec::new(),
            cursor: 0,
            max_connections: 0,
            min_connections: 0,
            active_connections: HashMap::with_capacity(TARGETED_CONNECTION_NUM),
        }
    }

    async fn handle_forward(&mut self, req: &UpdateRequest) -> Result<()> {
        // Grab the epoch
        let epoch = self.query_runner.get_epoch();

        // If the epoch is different then the last time we grabbed the committee refresh
        if epoch != self.epoch {
            self.refresh_epoch();
        }

        // If there is no open connections try and make a connection with the targeted number of
        // peers or the rest of committee we have not tried yet
        if self.active_connections.len() < self.min_connections {
            self.make_connections().await?;
        }

        // serialize transaction
        let txn_bytes = bincode::serialize(&req)?;

        let request = TransactionProto {
            transaction: txn_bytes.into(),
        };

        // Here we take ownership of self.active_connections by swapping it with an empty vec, so we
        // can try and send the transaction to each worker we have a connection with. If the
        // send was successful we keep the connection, if the send was not successful we assume it a
        // bad connection and drop the client
        let cap = self.active_connections.capacity();
        let active_connections =
            std::mem::replace(&mut self.active_connections, HashMap::with_capacity(cap));

        self.active_connections.extend(
            futures::future::join_all(active_connections.into_iter().map(|mut e| async {
                e.1.submit_transaction(request.clone())
                    .await
                    .ok()
                    .map(|_| e)
            }))
            .await
            .into_iter()
            // Note that `Option<T>: Iter<T>`. So here we can use flatten. If the option is Some,
            // then the inner iterator will yield one item of type `T` and if the option is None
            // then it will be an empty iterator.
            .flatten(),
        );

        if self.active_connections.is_empty() {
            bail!("Failed sending transaction to any worker")
        }
        Ok(())
    }

    fn refresh_epoch(&mut self) {
        let EpochInfo {
            mut committee,
            epoch,
            ..
        } = self.query_runner.get_epoch_info();

        // If our node is on the committee, return a vec with just our node. Or return a vec of all
        // the committee members
        if let Some(item) = committee.iter().find(|x| x.public_key == self.primary_name) {
            self.committee = vec![item.clone()];
        } else {
            // shuffle the order with thread_range so nodes accross the network are not all
            // connecting to the same workers
            committee.shuffle(&mut rand::thread_rng());
            self.committee = committee;
        }

        // Set the epoch info to the newest epoch
        self.epoch = epoch;

        self.max_connections = cmp::min(TARGETED_CONNECTION_NUM, self.committee.len());
        self.min_connections = self.max_connections / 3 * 2 + 1;
    }

    async fn make_connections(&mut self) -> Result<()> {
        let start_index = self.cursor;

        while self.active_connections.len() < self.min_connections {
            // Only try to make a connection with this worker if we dont already have one
            if let Entry::Vacant(e) = self.active_connections.entry(self.cursor) {
                let mempool = self.committee[self.cursor].workers[0].mempool.to_string();
                if let Ok(client) = TransactionsClient::connect(mempool).await {
                    e.insert(client);
                }
            }

            // Increment cursor to modulo so we never try to access out of bounds index
            self.cursor = (self.cursor + 1) % self.committee.len();

            // If we have travelled the entire ring buffer break the loop...we tried our best
            if self.cursor == start_index {
                break;
            }
        }

        if self.active_connections.is_empty() {
            bail!("Couldnt get a connection with a committee member")
        }

        Ok(())
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + 'static> AsyncWorker for Forwarder<Q> {
    type Request = UpdateRequest;
    type Response = ();

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        // if it fails we should retry once to cover all edge cases
        let mut retried = 0;
        while retried < 2 {
            if let Err(e) = self.handle_forward(&req).await {
                println!("Failed to send transaction to a worker: {e}");
                retried += 1;
            } else {
                break;
            }
        }
    }
}
