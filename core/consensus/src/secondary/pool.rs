use std::{sync::Arc, time::Instant};

use fastcrypto::hash::Hash;
use narwhal_types::{Batch, BatchDigest};
use typed_store::rocks::DBMap;

/// A batch pool can be used to resolve batches.
#[derive(Clone)]
pub struct BatchPool {
    store: DBMap<BatchDigest, Batch>,
}

pub struct PendingBatch {
    pub digest: BatchDigest,
    pub time_requested: Instant,
}

impl BatchPool {
    pub fn new(store: DBMap<BatchDigest, Batch>) -> Self {
        Self { store }
    }

    pub async fn recv(&self, digest: &BatchDigest) -> Batch {
        todo!()
    }

    /// Return a list of pending batches that we're interested in.
    pub async fn get_pending(&self) -> Vec<PendingBatch> {
        todo!()
    }

    pub async fn store(&self, batch: Batch) {
        let digest = batch.digest();
        todo!()
    }
}
