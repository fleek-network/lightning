use ethers::types::BlockNumber;
use fdi::BuildGraph;

use crate::components::NodeComponents;
use crate::types::{BlockReceipt, TransactionReceipt, TransactionRequest};
use crate::{c, ApplicationInterface};

#[interfaces_proc::blank]
pub trait ArchiveInterface<C: NodeComponents>: BuildGraph + Clone + Send + Sync {
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
