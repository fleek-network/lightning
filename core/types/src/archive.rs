use ethers::types::BlockNumber;

use crate::{Block, BlockExecutionResponse, BlockReceipt, TransactionReceipt};

pub enum ArchiveRequest {
    GetBlockByHash([u8; 32]),
    GetBlockByNumber(BlockNumber),
    GetTransactionReceipt([u8; 32]),
    GetTransaction([u8; 32]),
}

pub enum ArchiveResponse {
    Block(BlockReceipt),
    TransactionReceipt(TransactionReceipt),
    None,
}

pub struct IndexRequest {
    pub block: Block,
    pub receipt: BlockExecutionResponse,
}
