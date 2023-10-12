use ethers::types::BlockNumber;

use crate::{Block, BlockExecutionResponse, BlockReceipt, TransactionReceipt, TransactionRequest};

#[derive(Clone, Debug)]
pub enum ArchiveRequest {
    GetBlockByHash([u8; 32]),
    GetBlockByNumber(BlockNumber),
    GetTransactionReceipt([u8; 32]),
    GetTransaction([u8; 32]),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ArchiveResponse {
    Block(BlockReceipt),
    TransactionReceipt(TransactionReceipt),
    Transaction(TransactionRequest),
    None,
}

#[derive(Clone, Debug)]
pub struct IndexRequest {
    pub block: Block,
    pub receipt: BlockExecutionResponse,
}
