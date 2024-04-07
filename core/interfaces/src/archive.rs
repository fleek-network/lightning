use ethers::types::BlockNumber;
use fdi::BuildGraph;
use infusion::c;

use crate::infu_collection::Collection;
use crate::types::{BlockReceipt, TransactionReceipt, TransactionRequest};
use crate::ApplicationInterface;

#[infusion::service]
pub trait ArchiveInterface<C: Collection>: BuildGraph + Clone + Send + Sync {
    /// Returns true if the current node is being run as an archive node.
    fn is_active(&self) -> bool;

    async fn get_block_by_hash(&self, hash: [u8; 32]) -> Option<BlockReceipt>;

    async fn get_block_by_number(&self, number: BlockNumber) -> Option<BlockReceipt>;

    async fn get_transaction_receipt(&self, hash: [u8; 32]) -> Option<TransactionReceipt>;

    async fn get_transaction(&self, hash: [u8; 32]) -> Option<TransactionRequest>;

    async fn get_historical_epoch_state(
        &self,
        epoch: u64,
    ) -> Option<c![C::ApplicationInterface::SyncExecutor]>;
}
