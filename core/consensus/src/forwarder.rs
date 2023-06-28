/*
   The purpose of the forwarder is to provide a socket that other processess can send signed transactions too. The forwarder
   will then forward the transaction to an active narwhal committee member, prefering its own worker if the node is currently
   on the committee. Retry logic should be done on the proccess that has the sender side of the socket.
*/
use affair::AsyncWorker;
use async_trait::async_trait;
use draco_interfaces::{
    types::{Epoch, NodeInfo, UpdateRequest},
    SyncQueryRunnerInterface,
};
use fastcrypto::bls12381::min_sig::BLS12381PublicKey;
use fleek_crypto::NodePublicKey;
use narwhal_types::{TransactionProto, TransactionsClient};

pub struct Forwarder<Q: SyncQueryRunnerInterface> {
    /// Query runner used to read application state
    query_runner: Q,
    /// The public key of this node
    primary_name: NodePublicKey,
    /// Current Epoch
    epoch: Epoch,
    /// List of the committee members
    committee: Vec<NodeInfo>,
}

impl<Q: SyncQueryRunnerInterface> Forwarder<Q> {
    pub fn new(query_runner: Q, primary_name: BLS12381PublicKey) -> Self {
        Self {
            query_runner,
            primary_name: primary_name.into(),
            epoch: 0,
            committee: Vec::new(),
        }
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + 'static> AsyncWorker for Forwarder<Q> {
    type Request = UpdateRequest;
    type Response = ();

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        // Grab the epoch
        let epoch = self.query_runner.get_epoch();

        // If the epoch is different then the last time we grabbed the committee, or if we dont have
        // any committee info repull the committee info from application
        if epoch != self.epoch || self.committee.is_empty() {
            let committee = self.query_runner.get_epoch_info().committee;

            if committee.iter().any(|x| x.public_key == self.primary_name) {
                self.committee = committee
                    .iter()
                    .filter(|x| x.public_key == self.primary_name)
                    .cloned()
                    .collect()
            } else {
                self.committee = committee;
            }
            self.epoch = epoch;
        }

        if self.committee.is_empty() {
            return;
        }

        let mempool_address = self.committee[0].workers[0].mempool.to_string();

        // serialize transaction
        let txn_bytes = match bincode::serialize(&req) {
            Ok(bytes) => bytes,
            _ => return,
        };

        let request = TransactionProto {
            transaction: txn_bytes.into(),
        };

        // Send to committee
        let mut client = match TransactionsClient::connect(mempool_address).await {
            Ok(client) => client,
            _ => return,
        };

        let _ = client.submit_transaction(request).await;
    }
}
