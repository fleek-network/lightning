use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};
use lightning_interfaces::{SyncQueryRunnerInterface, ToDigest};

use crate::execution::{AuthenticStampedParcel, Digest, Execution};

#[derive(Clone)]
pub struct TransactionStore {
    parcels: HashMap<Digest, AuthenticStampedParcel>,
    attestations: HashMap<Digest, Vec<Attestation>>,
    executed: HashSet<Digest>,
}

#[derive(PartialEq, Eq, Clone)]
struct Attestation {
    node_index: NodeIndex,
    // This is the digest from the broadcast message that contained the attestation.
    // At the moment, both broadcast and consensus use [u8; 32] for the digests, but we should
    // treat them as different types nonetheless.
    message_digest: BroadcastDigest,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            parcels: HashMap::with_capacity(512),
            attestations: HashMap::with_capacity(512),
            executed: HashSet::with_capacity(512),
        }
    }

    pub fn store_parcel(&mut self, parcel: AuthenticStampedParcel) {
        let digest = parcel.to_digest();
        self.parcels.insert(digest, parcel);
    }

    pub fn add_attestation(
        &mut self,
        digest: Digest,
        node_index: NodeIndex,
        message_digest: BroadcastDigest,
    ) {
        let attestation_list = self.attestations.entry(digest).or_default();
        let attn = Attestation {
            node_index,
            message_digest,
        };
        if !attestation_list.contains(&attn) {
            attestation_list.push(attn);
        }
    }

    // Threshold should be 2f + 1 of the committee
    // Returns true if the epoch has changed
    pub async fn try_execute<Q: SyncQueryRunnerInterface>(
        &mut self,
        digest: Digest,
        threshold: usize,
        execution: &Arc<Execution<Q>>,
        head: Digest,
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
                self.try_execute_chain(digest, execution, head).await
            } else {
                false
            }
        } else {
            // If we have no attestations return false
            false
        }
    }

    async fn try_execute_chain<Q: SyncQueryRunnerInterface>(
        &mut self,
        digest: Digest,
        execution: &Arc<Execution<Q>>,
        head: Digest,
    ) -> bool {
        let mut txn_chain = VecDeque::new();
        let mut last_digest = digest;
        let mut parcel_chain = Vec::new();

        while let Some(parcel) = self.parcels.get(&last_digest) {
            parcel_chain.push(last_digest);

            txn_chain.push_front((parcel.transactions.clone(), last_digest));

            // for batch in &parcel.transactions {
            //     txn_chain.push_front(batch.clone());
            // }
            if parcel.last_executed == head {
                let mut epoch_changed = false;

                // We connected the chain now execute all the transactions
                for (batch, digest) in txn_chain {
                    if execution.submit_batch(batch, digest).await {
                        epoch_changed = true;
                    }
                }

                // mark all parcels in chain as executed
                for digest in parcel_chain {
                    self.executed.insert(digest);
                }

                return epoch_changed;
            } else {
                last_digest = parcel.last_executed;
            }
        }
        false
    }
}
