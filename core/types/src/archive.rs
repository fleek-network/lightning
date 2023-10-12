use ethers::types::BlockNumber;

use crate::{Block, BlockExecutionResponse, TransactionReceipt};

pub enum ArchiveRequest {
    GetBlockByHash([u8; 32]),
    GetBlockByNumber(BlockNumber),
    GetTransactionReceipt([u8; 32]),
}

pub enum ArchiveResponse {
    Block(BlockExecutionResponse),
    TransactionReceipt(TransactionReceipt),
    None,
}

pub struct IndexRequest {
    pub block: Block,
    pub receipt: BlockExecutionResponse,
}
