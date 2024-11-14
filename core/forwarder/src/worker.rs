/*
   The purpose of the forwarder is to provide a socket that other processess can send signed transactions too. The forwarder
   will then forward the transaction to an active narwhal committee member, prefering its own worker if the node is currently
   on the committee. Retry logic should be done on the proccess that has the sender side of the socket.
*/
use std::cmp;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use affair::AsyncWorker;
use anyhow::Result;
use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use lightning_interfaces::types::{Epoch, EpochInfo, ForwarderError, NodeInfo, TransactionRequest};
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_utils::application::QueryRunnerExt;
use narwhal_types::{TransactionProto, TransactionsClient};
use rand::seq::SliceRandom;
use tokio::time::{timeout, Duration};
use tonic::transport::channel::Channel;

const TARGETED_CONNECTION_NUM: usize = 10;
const TIMEOUT_DURATION: Duration = Duration::from_secs(4);

pub struct Worker<Q: SyncQueryRunnerInterface> {
    /// Query runner used to read application state
    query_runner: Q,
    /// The consensus public key of this node.
    primary_name: ConsensusPublicKey,
    /// The node public key of this node.
    node_public_key: NodePublicKey,
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

impl<Q: SyncQueryRunnerInterface> Worker<Q> {
    pub fn new(
        primary_name: ConsensusPublicKey,
        node_public_key: NodePublicKey,
        query_runner: Q,
    ) -> Self {
        Self {
            query_runner,
            primary_name,
            node_public_key,
            epoch: 0,
            committee: Vec::new(),
            cursor: 0,
            max_connections: 0,
            min_connections: 0,
            active_connections: HashMap::with_capacity(TARGETED_CONNECTION_NUM),
        }
    }

    async fn handle_forward(&mut self, req: &TransactionRequest) -> Result<(), ForwarderError> {
        // Grab the epoch
        let epoch = self.query_runner.get_current_epoch();

        // If the epoch is different then the last time we grabbed the committee refresh
        if epoch != self.epoch || epoch == 0 {
            self.refresh_epoch();
        }

        // If there is no open connections try and make a connection with the targeted number of
        // peers or the rest of committee we have not tried yet
        if self.active_connections.len() < self.min_connections {
            self.make_connections().await?;
        }

        // serialize transaction
        let txn_bytes: Vec<u8> = req.try_into().map_err(|e: anyhow::Error| {
            ForwarderError::FailedToSerializeTransaction(e.to_string())
        })?;

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
                timeout(TIMEOUT_DURATION, e.1.submit_transaction(request.clone()))
                    .await
                    .ok()
                    .and_then(|res| res.ok())
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
            return Err(ForwarderError::FailedToSendToAnyConnection);
        }

        Ok(())
    }

    fn refresh_epoch(&mut self) {
        let EpochInfo {
            committee, epoch, ..
        } = self.query_runner.get_epoch_info();

        // If our node is on the committee and has sufficient stake to be active, return a vec with
        // just our node. Or return a vec of all the committee members
        let is_valid_node = self
            .query_runner
            .has_sufficient_stake(&self.node_public_key)
            && self
                .query_runner
                .is_participating_node(&self.node_public_key);
        let maybe_node = committee
            .iter()
            .find(|x| x.consensus_key == self.primary_name);
        if is_valid_node && maybe_node.is_some() {
            // Reset cursor
            self.cursor = 0;
            self.committee = vec![maybe_node.unwrap().clone()];
        } else {
            // shuffle the order with thread_range so nodes accross the network are not all
            // connecting to the same workers
            let mut committee = committee
                .into_iter()
                .filter(|i| i.consensus_key != self.primary_name)
                .collect::<Vec<_>>();
            committee.shuffle(&mut rand::thread_rng());
            self.committee = committee;
            self.cursor = 0;
        }

        // Set the epoch info to the newest epoch
        self.epoch = epoch;

        self.max_connections = cmp::min(TARGETED_CONNECTION_NUM, self.committee.len());
        self.min_connections = self.max_connections / 3 * 2 + 1;
    }

    async fn make_connections(&mut self) -> Result<(), ForwarderError> {
        let start_index = self.cursor;

        while self.active_connections.len() < self.min_connections {
            // Only try to make a connection with this worker if we dont already have one
            if let Entry::Vacant(e) = self.active_connections.entry(self.cursor) {
                let mempool_port = &self.committee[self.cursor].ports.mempool;
                let address = &self.committee[self.cursor].worker_domain;

                if let Ok(client) =
                    TransactionsClient::connect(format!("http://{address}:{mempool_port}")).await
                {
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
            return Err(ForwarderError::NoActiveConnections);
        }

        Ok(())
    }
}

impl<Q: SyncQueryRunnerInterface + 'static> AsyncWorker for Worker<Q> {
    type Request = TransactionRequest;
    type Response = Result<(), ForwarderError>;

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.handle_forward(&req).await
    }
}
