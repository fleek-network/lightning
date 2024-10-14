use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use lightning_interfaces::prelude::*;

use super::TransactionSigner;

/// The transaction nonce syncer is responsible for listening for new blocks from the notifier
/// and updating the next nonce.
pub(crate) struct TransactionClientNonceSyncer {}

impl TransactionClientNonceSyncer {
    /// Create and spawn a new transaction nonce syncer, that's responsible for updating the
    /// next nonce counter every time a new block is executed.
    ///
    /// This method is non-blocking and returns immediately after spawning the task.
    ///
    /// The spawned task will run until the notifier subscription is closed.
    pub async fn spawn<C: NodeComponents>(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        signer: TransactionSigner,
        next_nonce: Arc<AtomicU64>,
    ) {
        spawn!(
            async move {
                let mut block_sub = notifier.subscribe_block_executed();
                loop {
                    let Some(_) = block_sub.recv().await else {
                        tracing::debug!("block subscription stream ended");
                        break;
                    };

                    // Update the next nonce counter from application state.
                    next_nonce.store(signer.get_nonce(&app_query) + 1, Ordering::Relaxed);
                }
            },
            "TRANSACTION-CLIENT: syncer"
        );
    }
}
