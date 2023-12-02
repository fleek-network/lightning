use ethers::types::{
    Address,
    Bytes,
    TransactionReceipt,
    TransactionRequest,
    H256,
    U256,
    BlockNumber,
    Block,
};
use fleek_crypto::EthAddress;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use crate::api_types::{StateOverride, CallRequest};

#[rpc(client, server, namespace = "eth")]
pub trait EthApi {
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> RpcResult<U256>;

    #[method(name = "getTransactionCount")]
    async fn transaction_count(&self, address: EthAddress, block_number: Option<BlockNumber>) -> RpcResult<U256>;

    #[method(name = "getBalance")]
    async fn balance(&self, address: EthAddress, block_number: Option<BlockNumber>) -> RpcResult<U256>;

    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<u64>;

    #[method(name = "chainId")]
    async fn chain_id(&self) -> RpcResult<u32>;

    #[method(name = "syncing")]
    async fn syncing(&self) -> RpcResult<bool>;

    #[method(name = "coinbase")]
    async fn coinbase(&self) -> RpcResult<Address>;

    #[method(name = "accounts")]
    async fn accounts(&self) -> RpcResult<Vec<Address>>;

    #[method(name = "getBlockByNumber")]
    async fn block_by_number(&self, block_number: BlockNumber, hydrated: bool) -> RpcResult<Option<H256>>;

    #[method(name = "getBlockByHash")]
    async fn block_by_hash(&self, hash: H256, hydrated: bool) -> RpcResult<Option<Block<H256>>>;

    #[method(name = "gasPrice")]
    async fn gas_price(&self) -> RpcResult<U256>;

    #[method(name = "estimateGas")]
    async fn estimate_gas(&self, request: CallRequest) -> RpcResult<U256>;

    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, transaction: Bytes) -> RpcResult<H256>;

    #[method(name = "getCode")]
    async fn code(
        &self,
        address: EthAddress,
        block_number: Option<BlockNumber>,
    ) -> RpcResult<Bytes>;

    #[method(name = "getStorageAt")]
    async fn storage_at(
        &self,
        address: EthAddress,
        index: U256,
        block_number: Option<BlockNumber>,
    ) -> RpcResult<Bytes>;

    #[method(name = "getTransactionByHash")]
    async fn transaction_by_hash(&self, hash: H256) -> RpcResult<String>;

    #[method(name = "maxPriorityFeePerGas")]
    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256>;

    #[method(name = "feeHistory")]
    async fn fee_history(&self) -> RpcResult<String>;

    #[method(name = "mining")]
    async fn mining(&self) -> RpcResult<bool>;

    #[method(name = "hashrate")]
    async fn hashrate(&self) -> RpcResult<U256>;

    #[method(name = "getWork")]
    async fn get_work(&self) -> RpcResult<Vec<String>>;

    #[method(name = "submitHashrate")]
    async fn submit_hashrate(&self, hash_rate: U256, client_id: String) -> RpcResult<bool>;

    #[method(name = "submitWork")]
    async fn submit_work(&self, nonce: U256, pow_hash: H256, mix_digest: H256) -> RpcResult<bool>;

    #[method(name = "sendTransaction")]
    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<H256>;

    #[method(name = "call")]
    async fn call(
        &self,
        tx: TransactionRequest,
        block_number: Option<BlockNumber>,
        state_overrides: Option<StateOverride>
    ) -> RpcResult<Bytes>;

    #[method(name = "sign")]
    async fn sign(&self, address: EthAddress, message: Bytes) -> RpcResult<Bytes>;

    #[method(name = "signTransaction")]
    async fn sign_transaction(&self, transaction: TransactionRequest) -> RpcResult<Bytes>;

    #[method(name = "getTransactionReceipt")]
    async fn transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>>;
}