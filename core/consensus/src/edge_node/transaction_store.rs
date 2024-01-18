use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};
use lightning_interfaces::{Emitter, SyncQueryRunnerInterface};
use lightning_utils::application::QueryRunnerExt;

use super::ring_buffer::RingBuffer;
use crate::execution::{AuthenticStampedParcel, Digest, Execution};

// Exponentially moving average parameter for estimating the time between executions of parcels.
// This parameter must be in range [0, 1].
const TBE_EMA: f64 = 0.125;

// Bounds for the estimated time between executions.
const MIN_TBE: Duration = Duration::from_secs(30);
const MAX_TBE: Duration = Duration::from_secs(40);

pub struct TransactionStore {
    parcels: RingBuffer,
    executed: HashSet<Digest>,
    pending: HashSet<Digest>,
    last_executed_timestamp: Option<SystemTime>,
    estimated_tbe: Duration,
    deviation_tbe: Duration,
}

#[derive(Clone)]
pub struct Parcel {
    pub inner: AuthenticStampedParcel,
    // The originator of this parcel.
    pub originator: NodeIndex,
    // This is the digest from the broadcast message that contained the parcel.
    // At the moment, both broadcast and consensus use [u8; 32] for the digests, but we should
    // treat them as different types nonetheless.
    pub message_digest: Option<BroadcastDigest>,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            parcels: RingBuffer::new(),
            executed: HashSet::with_capacity(512),
            pending: HashSet::with_capacity(512),
            last_executed_timestamp: None,
            // TODO(matthias): do some napkin math for these initial estimates
            estimated_tbe: Duration::from_secs(30),
            deviation_tbe: Duration::from_secs(5),
        }
    }

    pub fn get_parcel(&self, digest: &Digest) -> Option<&Parcel> {
        self.parcels.get_parcel(digest)
    }

    pub fn get_attestations(&self, digest: &Digest) -> Option<&HashSet<NodeIndex>> {
        self.parcels.get_attestations(digest)
    }

    // Store a parcel and optionally provide the digest of the broadcast message that delivered
    // this parcel.
    // If we already store the parcel, we won't overwrite it again. If we already store the parcel,
    // but don't store the broadcast message yet, we will insert the message.
    pub fn store_parcel(
        &mut self,
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: Option<BroadcastDigest>,
    ) {
        self.parcels
            .store_parcel(parcel, originator, message_digest);
    }

    // Store a parcel from the next epoch. After the epoch change we have to verify if this
    // parcel originated from a committee member.
    pub fn store_pending_parcel(
        &mut self,
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: BroadcastDigest,
    ) {
        self.parcels
            .store_pending_parcel(parcel, originator, Some(message_digest));
    }

    pub fn add_attestation(&mut self, digest: Digest, node_index: NodeIndex) {
        self.parcels.add_attestation(digest, node_index);
    }

    // Stores an attestation from the next epoch. After the epoch change we have to verify if this
    // attestation originated from a committee member.
    pub fn add_pending_attestation(&mut self, digest: Digest, node_index: NodeIndex) {
        self.parcels.add_pending_attestation(digest, node_index);
    }

    pub fn change_epoch(&mut self, committee: &[NodeIndex]) {
        self.parcels.change_epoch(committee);
    }

    pub fn get_timeout(&self) -> Duration {
        let mut timeout = 4 * self.deviation_tbe + self.estimated_tbe;
        timeout = timeout.max(MIN_TBE);
        timeout = timeout.min(MAX_TBE);
        timeout
    }

    // Threshold should be 2f + 1 of the committee
    // Returns true if the epoch has changed
    pub async fn try_execute<Q: SyncQueryRunnerInterface, NE: Emitter>(
        &mut self,
        digest: Digest,
        threshold: usize,
        query_runner: &Q,
        execution: &Arc<Execution<Q, NE>>,
    ) -> Result<bool, NotExecuted> {
        // get the current chain head
        let head = query_runner.get_last_block();
        let mut epoch_changed = match self
            .try_execute_internal(digest, threshold, execution, head)
            .await
        {
            Ok(epoch_changed) => epoch_changed,
            Err(NotExecuted::MissingAttestations(_)) => false,
            Err(e) => return Err(e),
        };

        let digests: Vec<Digest> = self.pending.iter().copied().collect();
        for digest in digests {
            if self.pending.contains(&digest) {
                // get the current chain head
                let head = query_runner.get_last_block();
                if let Ok(epoch_changed_) = self
                    .try_execute_internal(digest, threshold, execution, head)
                    .await
                {
                    epoch_changed = epoch_changed || epoch_changed_;
                }
            }
        }
        Ok(epoch_changed)
    }

    async fn try_execute_internal<Q: SyncQueryRunnerInterface, NE: Emitter>(
        &mut self,
        digest: Digest,
        threshold: usize,
        execution: &Arc<Execution<Q, NE>>,
        head: Digest,
    ) -> Result<bool, NotExecuted> {
        if self.executed.contains(&digest) {
            // we already executed this parcel
            return Ok(false);
        } else if let Some(x) = self.get_attestations(&digest) {
            // we need a quorum of attestations in order to execute the parcel
            if x.len() >= threshold {
                // if we should execute we need to make sure we can connect this to our transaction
                // chain
                return self.try_execute_chain(digest, execution, head).await;
            }
        }
        Err(NotExecuted::MissingAttestations(digest))
    }

    async fn try_execute_chain<Q: SyncQueryRunnerInterface, NE: Emitter>(
        &mut self,
        digest: Digest,
        execution: &Arc<Execution<Q, NE>>,
        head: Digest,
    ) -> Result<bool, NotExecuted> {
        let mut txn_chain = VecDeque::new();
        let mut last_digest = digest;
        let mut parcel_chain = Vec::new();

        while let Some(parcel) = self.get_parcel(&last_digest) {
            parcel_chain.push(last_digest);

            txn_chain.push_front((parcel.inner.transactions.clone(), last_digest));

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
                    self.pending.remove(&digest);
                    self.executed.insert(digest);
                }

                // TODO(matthias): technically this call should be inside the for loop where we
                // call `submit_batch`, but I think this might bias the estimate to be too low.
                self.update_estimated_tbe();

                return Ok(epoch_changed);
            } else {
                last_digest = parcel.inner.last_executed;
            }
        }
        for digest in parcel_chain {
            self.pending.insert(digest);
        }
        Err(NotExecuted::MissingParcel(last_digest))
    }

    // This method should be called whenever we execute a parcel.
    fn update_estimated_tbe(&mut self) {
        if let Some(timestamp) = self.last_executed_timestamp {
            if let Ok(sample_tbe) = timestamp.elapsed() {
                let sample_tbe = sample_tbe.as_millis() as f64;
                let estimated_tbe = self.estimated_tbe.as_millis() as f64;
                let new_estimated_tbe = (1.0 - TBE_EMA) * estimated_tbe + TBE_EMA * sample_tbe;
                self.estimated_tbe = Duration::from_millis(new_estimated_tbe as u64);

                let deviation_tbe = self.deviation_tbe.as_millis() as f64;
                let new_deviation_tbe = (1.0 - TBE_EMA) * deviation_tbe
                    + TBE_EMA * (new_estimated_tbe - sample_tbe).abs();
                self.deviation_tbe = Duration::from_millis(new_deviation_tbe as u64);
            }
        }
        self.last_executed_timestamp = Some(SystemTime::now());
    }
}

#[derive(Debug)]
pub enum NotExecuted {
    MissingParcel(Digest),
    MissingAttestations(Digest),
}
