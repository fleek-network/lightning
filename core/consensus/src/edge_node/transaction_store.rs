use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{PubSub, ToDigest};

use crate::consensus::PubSubMsg;
use crate::execution::{AuthenticStampedParcel, Digest, Execution};

#[derive(Clone)]
pub struct TransactionStore {
    parcels: HashMap<Digest, AuthenticStampedParcel>,
    attestations: HashMap<Digest, Vec<NodeIndex>>,
    executed: HashSet<Digest>,
    head: Digest,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            parcels: HashMap::with_capacity(512),
            attestations: HashMap::with_capacity(512),
            executed: HashSet::with_capacity(512),
            head: [0; 32],
        }
    }

    pub fn store_parcel(&mut self, parcel: AuthenticStampedParcel) {
        let digest = parcel.to_digest();
        self.parcels.insert(digest, parcel);
    }

    pub fn add_attestation(&mut self, digest: Digest, node: NodeIndex) {
        let attestation_list = self.attestations.entry(digest).or_default();
        if !attestation_list.contains(&node) {
            attestation_list.push(node);
        }
    }

    // Threshold should be 2f + 1 of the committee
    pub async fn try_execute<P: PubSub<PubSubMsg>>(
        &mut self,
        digest: Digest,
        threshold: usize,
        execution: &Arc<Execution<P>>,
    ) -> bool {
        if self.executed.contains(&digest) {
            // if we executed before return false
            false
        } else if let Some(x) = self.attestations.get(&digest) {
            // If it is in our attestation table return true if our attestations is >= our threshold
            // else false
            if x.len() >= threshold {
                // if we should execute we need to make sure we can connect this to our transaction
                // chain
                self.try_execute_chain(digest, execution).await.is_ok()
            } else {
                false
            }
        } else {
            // If we have no attestations return false
            false
        }
    }

    async fn try_execute_chain<P: PubSub<PubSubMsg>>(
        &mut self,
        digest: Digest,
        execution: &Arc<Execution<P>>,
    ) -> Result<()> {
        let mut txn_chain = VecDeque::new();
        let mut last_digest = digest;
        let mut parcel_chain = Vec::new();

        while let Some(parcel) = self.parcels.get(&last_digest) {
            parcel_chain.push(last_digest);
            for batch in &parcel.batches {
                txn_chain.push_front(batch.clone());
            }
            if parcel.last_executed == self.head {
                // We connected the chain now execute all the transactions
                execution.submit_batch(txn_chain.into()).await;

                // mark all parcels in chain as executed
                for digest in parcel_chain {
                    self.executed.insert(digest);
                }
                // set head as top of chain
                self.head = digest;

                return Ok(());
            } else {
                last_digest = parcel.last_executed;
            }
        }
        Err(anyhow!("Cannot connect chain did not execute"))
    }
}
