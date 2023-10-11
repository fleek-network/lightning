use crate::{BlockExecutionResponse, TransactionRequest};

pub enum ArchiveRequest {
    GetBlock([u8; 32]),
    GetTransactionReceipt([u8; 32]),
}

pub enum ArchiveResponse {
    Block,
    TransactionReceipt,
}

pub enum IndexRequest {
    BlockReciept(BlockExecutionResponse),
    Transactions(Vec<TransactionRequest>),
}
