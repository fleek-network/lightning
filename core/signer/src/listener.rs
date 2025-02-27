use std::sync::Arc;

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::TransactionReceipt;
use quick_cache::sync::Cache;

pub struct BlockListener<C: NodeComponents> {
    notifier: C::NotifierInterface,
    receipt_cache: Arc<Cache<[u8; 32], TransactionReceipt>>,
}

impl<C: NodeComponents> BlockListener<C> {
    pub fn new(
        receipt_cache: Arc<Cache<[u8; 32], TransactionReceipt>>,
        notifier: C::NotifierInterface,
    ) -> Self {
        Self {
            notifier,
            receipt_cache,
        }
    }

    pub async fn start(&self) {
        let mut block_sub = self.notifier.subscribe_block_executed();

        loop {
            let notification = block_sub.recv().await;

            // Check that the notifier is still running.
            let Some(notification) = notification else {
                tracing::debug!("Notifier is not running, shutting down");
                break;
            };

            for receipt in notification.response.txn_receipts {
                let hash = receipt.transaction_hash;
                self.receipt_cache.insert(hash, receipt);
                // We get the item from the cache to turn the eviction policy into LRU.
                // This way the oldest entries will get evicted first.
                self.receipt_cache.get(&hash);
            }
        }
    }
}
