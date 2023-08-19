use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use fastcrypto::hash::Hash;
use log::error;
use narwhal_types::{Batch, BatchDigest};
use tokio::sync::Notify;
use typed_store::rocks::DBMap;
use typed_store::Map;

/// A batch pool can be used to resolve batches.
#[derive(Clone)]
pub struct BatchPool {
    store: DBMap<BatchDigest, Batch>,
    pending_futures: Arc<DashMap<BatchDigest, (Arc<Notify>, Instant)>>,
}

impl BatchPool {
    /// Create a new batch pool.
    pub fn new(store: DBMap<BatchDigest, Batch>) -> Self {
        Self {
            store,
            pending_futures: Arc::new(DashMap::with_capacity(512)),
        }
    }

    /// Receive a batch by its digest. If the batch does not exist returns a future
    /// that will be resolved once someone inserts the batch into the database making
    /// it available.
    pub async fn get(&self, digest: BatchDigest) -> Batch {
        loop {
            // Get the lock on the digest entry so a parallel store would not have access
            // to get the data, since it might get `None` right before we get to insert the
            // notifier.
            let entry = self.pending_futures.entry(digest);

            // TODO(qti3e): Maybe handle the error.
            if let Ok(Some(batch)) = self.store.get(&digest) {
                return batch;
            }

            let v_ref = entry.or_insert_with(|| (Arc::new(Notify::const_new()), Instant::now()));
            let notify = v_ref.0.clone();

            // Don't hold the lock anymore.
            drop(v_ref);

            // Wait until the wake up, which would indicate an store has completed.
            notify.notified().await;
        }
    }

    /// Store a batch into the database and resolve possible waiters.
    pub fn store(&self, batch: Batch) {
        let digest = batch.digest();

        // todo(dalton): This unwrap basically means rocksdb is messing up, lets retry a few times
        // and figure out whats going on here if it fails. Shouldnt fail though
        if let Err(e) = self.store.insert(&digest, &batch) {
            error!("Failed inserting digest in pool store, trying again : {e:?}");
            self.store.insert(&digest, &batch).unwrap();
        }

        // Wake up anyone who was interested in this batch.
        if let Some(v_ref) = self.pending_futures.get(&digest) {
            v_ref.0.notify_waiters();
        }
    }
}
