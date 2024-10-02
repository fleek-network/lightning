use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{TransactionReceipt, TxHash};
use tokio::sync::{oneshot, Mutex};

/// The transaction receipt listener is responsible for listening for transaction receipts from the
/// network via the notifier executed blocks subscription, and sending them to the registered
/// channel.
pub(crate) struct TransactionReceiptListener<C: NodeComponents> {
    pending: Arc<Mutex<HashMap<TxHash, oneshot::Sender<TransactionReceipt>>>>,
    _components: PhantomData<C>,
}

impl<C: NodeComponents> TransactionReceiptListener<C> {
    pub fn new() -> Self {
        let pending = Arc::new(Mutex::new(HashMap::<
            TxHash,
            oneshot::Sender<TransactionReceipt>,
        >::new()));
        Self {
            pending: pending.clone(),
            _components: PhantomData,
        }
    }

    /// Register a new transaction hash that we should listen for and a channel to send the receipt.
    pub async fn register(&self, tx: TxHash) -> oneshot::Receiver<TransactionReceipt> {
        let (receipt_tx, receipt_rx) = oneshot::channel();
        self.pending.lock().await.insert(tx, receipt_tx);
        receipt_rx
    }

    /// Create and spawn a new transaction receipt listener, that's responsible for listening for
    /// transaction receipts from the network via the notifier executed blocks subscription, and
    /// sending them to the registered channel.
    ///
    /// This method is non-blocking and returns immediately after spawning the listener.
    ///
    /// The listener will run until the notifier subscription is closed, or the listener is
    /// explicitly shut down.
    pub async fn spawn(notifier: C::NotifierInterface) -> Self {
        let listener = Self::new();
        let pending = listener.pending.clone();

        spawn!(
            async move {
                let mut block_sub = notifier.subscribe_block_executed();
                let pending = pending.clone();
                loop {
                    let notification = block_sub.recv().await;
                    if notification.is_none() {
                        tracing::debug!("block subscription stream ended");
                        break;
                    }
                    let response = notification.unwrap().response;

                    for receipt in response.txn_receipts {
                        let mut pending = pending.lock().await;
                        if pending.contains_key(&receipt.transaction_hash) {
                            if let Some(sender) = pending.remove(&receipt.transaction_hash) {
                                let _ = sender.send(receipt);
                            }
                        }
                    }
                }
            },
            "TRANSACTION-CLIENT: listener"
        );

        listener
    }
}
