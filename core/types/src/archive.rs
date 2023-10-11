use crate::{Block, BlockExecutionResponse};

pub enum ArchiveRequest {
    GetBlock([u8; 32]),
    GetTransactionReceipt([u8; 32]),
}

pub enum ArchiveResponse {
    Block,
    TransactionReceipt,
}

pub struct IndexRequest {
    pub block: Block,
    pub receipt: BlockExecutionResponse,
}
