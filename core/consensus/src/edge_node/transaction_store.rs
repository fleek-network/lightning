use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};
use lightning_interfaces::{SyncQueryRunnerInterface, ToDigest};

use crate::execution::{AuthenticStampedParcel, Digest, Execution};

#[derive(Clone)]
pub struct TransactionStore {
    parcels: HashMap<Digest, Parcel>,
    attestations: HashMap<Digest, Vec<NodeIndex>>,
    executed: HashSet<Digest>,
}

#[derive(Clone)]
pub(crate) struct Parcel {
    pub(crate) inner: AuthenticStampedParcel,
    // This is the digest from the broadcast message that contained the parcel.
    // At the moment, both broadcast and consensus use [u8; 32] for the digests, but we should
    // treat them as different types nonetheless.
    pub(crate) message_digest: Option<BroadcastDigest>,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            parcels: HashMap::with_capacity(512),
            attestations: HashMap::with_capacity(512),
            executed: HashSet::with_capacity(512),
        }
    }

    pub(crate) fn get_parcel(&self, digest: &Digest) -> Option<&Parcel> {
        self.parcels.get(digest)
    }

    // Store a parcel and optionally provide the digest of the broadcast message that delivered
    // this parcel.
    // If we already store the parcel, we won't overwrite it again. If we store the parcel, but
    // don't store a broadcast message yet, we will update it.
    pub fn store_parcel(
        &mut self,
        parcel: AuthenticStampedParcel,
        message_digest: Option<BroadcastDigest>,
    ) {
        let digest = parcel.to_digest();
        self.parcels
            .entry(digest)
            .and_modify(|parcel| {
                if parcel.message_digest.is_none() {
                    parcel.message_digest = message_digest;
                }
            })
            .or_insert(Parcel {
                inner: parcel,
                message_digest: None,
            });
    }

    pub fn add_attestation(&mut self, digest: Digest, node_index: NodeIndex) {
        let attestation_list = self.attestations.entry(digest).or_default();
        if !attestation_list.contains(&node_index) {
            attestation_list.push(node_index);
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
    ) -> Result<bool, NotExecuted> {
        if self.executed.contains(&digest) {
            // if we executed before return false
            Ok(false)
        } else if let Some(x) = self.attestations.get(&digest) {
            // If it is in our attestation table return true if our attestations is >= our threshold
            // else false
            if x.len() >= threshold {
                // if we should execute we need to make sure we can connect this to our transaction
                // chain
                self.try_execute_chain(digest, execution, head).await
            } else {
                //Ok(false)
                Err(NotExecuted::MissingAttestations(digest))
            }
        } else if self.parcels.contains_key(&digest) {
            Err(NotExecuted::MissingAttestations(digest))
        } else {
            Err(NotExecuted::MissingParcel(digest))
        }
    }

    async fn try_execute_chain<Q: SyncQueryRunnerInterface>(
        &mut self,
        digest: Digest,
        execution: &Arc<Execution<Q>>,
        head: Digest,
    ) -> Result<bool, NotExecuted> {
        let mut txn_chain = VecDeque::new();
        let mut last_digest = digest;
        let mut parcel_chain = Vec::new();

        while let Some(parcel) = self.parcels.get(&last_digest) {
            parcel_chain.push(last_digest);

            txn_chain.push_front((parcel.inner.transactions.clone(), last_digest));

            // for batch in &parcel.transactions {
            //     txn_chain.push_front(batch.clone());
            // }
            if parcel.inner.last_executed == head {
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

                return Ok(epoch_changed);
            } else {
                last_digest = parcel.inner.last_executed;
            }
        }
        Err(NotExecuted::MissingParcel(last_digest))
    }
}

#[derive(Debug)]
pub enum NotExecuted {
    MissingParcel(Digest),
    MissingAttestations(Digest),
}
