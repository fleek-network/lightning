use std::sync::Arc;

use ethers::types::{
    Address,
    Block,
    BlockId,
    BlockNumber,
    Bytes,
    FeeHistory,
    Transaction,
    TransactionRequest,
    H256,
    H256 as EthH256,
};
use ethers::utils::rlp;
use fleek_crypto::EthAddress;
use jsonrpc_v2::{Data, Error, Params};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::SyncQueryRunnerInterface;
use ruint::aliases::{B256, B64, U256, U64};
use tracing::trace;

use crate::handlers::Result;
use crate::server::RpcData;

/// Handler for: `eth_sendRawTransaction`
pub async fn eth_send_raw_transaction<C: Collection>(
    ///////
    data: Data<Arc<RpcData<C>>>,
    Params((tx,)): Params<(Bytes,)>,
) -> Result<H256> {
    trace!(target: "rpc::eth", "Serving eth_sendRawTransaction");
    let tx_data = tx.as_ref();

    let mut transaction = match rlp::decode::<Transaction>(tx_data) {
        Ok(transaction) => transaction,
        Err(_) => return Err(Error::from("Failed to decode signed transaction")),
    };

    transaction
        .recover_from_mut()
        .map_err(|_| Error::from("Invalid transaction signature"))?;

    let hash = transaction.hash();

    data.0
        .mempool_socket
        .run(transaction.into())
        .await
        .map_err(Error::internal)?;

    Ok(hash)
}

/// Handler for: `eth_protocolVersion`
pub async fn eth_protocol_version<C: Collection>() -> Result<U64> {
    trace!(target: "rpc::eth", "Serving eth_protocolVersion");
    Ok(U64::from(0x41))
}

/// Handler for: `eth_syncing`
pub async fn eth_syncing<C: Collection>() -> Result<bool> {
    trace!(target: "rpc::eth", "Serving eth_syncing");
    Ok(false)
}

/// Handler for: `eth_coinbase`
pub async fn eth_author<C: Collection>() -> Result<EthAddress> {
    // todo(dalton): Lets return the governance address here or something. Irrelevent but might play
    // nicer with tooling
    Err(Error::from("unimplemented"))
}

/// Handler for: `eth_accounts`
pub async fn eth_accounts<C: Collection>() -> Result<Vec<EthAddress>> {
    trace!(target: "rpc::eth", "Serving eth_accounts");
    Err(Error::from("unimplemented"))
}

/// Handler for: `eth_blockNumber`
pub async fn eth_block_number<C: Collection>(data: Data<Arc<RpcData<C>>>) -> Result<U256> {
    trace!(target: "rpc::eth", "Serving eth_blockNumber");
    log::error!(
        "the block number is {:?}",
        data.0.query_runner.get_block_number(),
    );
    Ok(U256::from(data.0.query_runner.get_block_number()))
}

/// Handler for: `eth_chainId`
pub async fn eth_chain_id<C: Collection>(data: Data<Arc<RpcData<C>>>) -> Result<Option<U64>> {
    trace!(target: "rpc::eth", "Serving eth_chainId");
    Ok(Some(U64::from(data.query_runner.get_chain_id())))
}

pub async fn net_version<C: Collection>(data: Data<Arc<RpcData<C>>>) -> Result<Option<String>> {
    trace!(target: "rpc::eth", "Serving eth_chainId");
    Ok(Some(data.query_runner.get_chain_id().to_string()))
}

pub async fn net_listening<C: Collection>() -> Result<bool> {
    trace!(target: "rpc::eth", "Serving Listening");
    Ok(true)
}

pub async fn net_peer_count<C: Collection>() -> Result<U64> {
    trace!(target: "rpc::eth", "Serving Count");
    // todo(dalton): Figure out what this is used for in ethereum instead of just mocking it here.
    // Shouldnt be relevent for us but may need to return something here for compatability
    Ok(U64::from(20))
}

/// Handler for: `eth_getCode`
pub async fn eth_get_code<C: Collection>(
    _data: Data<Arc<RpcData<C>>>,
    Params((address, block_number)): Params<(Address, Option<BlockId>)>,
) -> Result<Bytes> {
    trace!(target: "rpc::eth", ?address, ?block_number, "Serving eth_getCode");

    Ok(Bytes::default())
}

// /// Handler for: `eth_getBlockByNumber`
pub async fn eth_block_by_number<C: Collection>(
    Params((number, full)): Params<(BlockNumber, bool)>,
) -> Result<Option<Block<EthH256>>> {
    trace!(target: "rpc::eth", ?number, ?full, "Serving
  eth_getBlockByNumber");
    Ok(Some(Block::default()))
}

/// Handler for: `eth_getBalance` will return FLK balance
pub async fn eth_balance<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params((address, block_number)): Params<(EthAddress, BlockNumber)>,
) -> Result<U256> {
    trace!(target: "rpc::eth", ?address, ?block_number, "Serving eth_getBalance");
    // Todo(dalton) direct safe conversion from hpfixed => u128
    Ok(data
        .0
        .query_runner
        .get_flk_balance(&address)
        .try_into()
        .unwrap_or(U256::ZERO))
}

// /// Handler for: `eth_getTransactionCount`
pub async fn eth_transaction_count<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params((public_key, _tag)): Params<(EthAddress, String)>,
) -> Result<U256> {
    trace!(target: "rpc::eth", ?public_key, "Serving
  eth_getTransactionCount");
    Ok(U256::from(
        data.0.query_runner.get_account_nonce(&public_key),
    ))
}

// /// Handler for: `eth_estimateGas`
pub async fn eth_estimate_gas<C: Collection>(
    Params((request, block_number)): Params<(TransactionRequest, BlockId)>,
) -> Result<U256> {
    trace!(target: "rpc::eth", ?request, ?block_number, "Serving
  eth_estimateGas");
    Ok(U256::ZERO)
}

/// Handler for: `eth_gasPrice`
pub async fn eth_gas_price<C: Collection>() -> Result<U256> {
    trace!(target: "rpc::eth", "Serving eth_gasPrice");
    Ok(U256::ZERO)
}

// /// Handler for: `eth_maxPriorityFeePerGas`
pub async fn eth_max_priority_fee_per_gas<C: Collection>() -> Result<U256> {
    trace!(target: "rpc::eth", "Serving eth_maxPriorityFeePerGas");
    Ok(U256::ZERO)
}

// /// Handler for: `eth_feeHistory`
pub async fn eth_fee_history<C: Collection>(
    Params((block_count, newest_block, reward_percentiles)): Params<(
        U256,
        BlockNumber,
        Option<Vec<f64>>,
    )>,
) -> Result<FeeHistory> {
    trace!(target: "rpc::eth", ?block_count, ?newest_block,
  ?reward_percentiles, "Serving eth_feeHistory");
    Err(Error::from("unimplemented"))
}

// /// Handler for: `eth_mining`
pub async fn eth_is_mining<C: Collection>() -> Result<bool> {
    Err(Error::from("unimplemented"))
}

// /// Handler for: `eth_hashrate`
pub async fn eth_hashrate<C: Collection>() -> Result<U256> {
    Ok(U256::ZERO)
}

// /// Handler for: `eth_getWork`
pub async fn eth_get_work<C: Collection>() -> Result<()> {
    Err(Error::from("unimplemented"))
}

// /// Handler for: `eth_submitHashrate`
pub async fn eth_submit_hashrate<C: Collection>(
    Params((_hashrate, _id)): Params<(U256, B256)>,
) -> Result<bool> {
    Ok(false)
}

// /// Handler for: `eth_submitWork`
pub async fn eth_submit_work<C: Collection>(
    Params((_nonce, __pow_hash, _mix_digest)): Params<(B64, B256, B256)>,
) -> Result<bool> {
    Err(Error::from("unimplemented"))
}

/// Handler for: `eth_sendTransaction`
pub async fn eth_send_transaction<C: Collection>(
    Params(request): Params<TransactionRequest>,
) -> Result<B256> {
    trace!(target: "rpc::eth", ?request, "Serving eth_sendTransaction");
    // We shouldnt need to support this ever just eth_sendRawTransaction
    Err(Error::from("unimplemented"))
}

/// Handler for: `eth_sign`
pub async fn eth_sign<C: Collection>(
    Params((address, message)): Params<(Address, Bytes)>,
) -> Result<Bytes> {
    trace!(target: "rpc::eth", ?address, ?message, "Serving eth_sign");
    Err(Error::from("unimplemented"))
}

// /// Handler for: `eth_signTransaction`
pub async fn eth_sign_transaction<C: Collection>(
    Params(_transaction): Params<TransactionRequest>,
) -> Result<Bytes> {
    Err(Error::from("unimplemented"))
}

// /// Handler for: `eth_signTypedData`
// async fn sign_typed_data<C: Collection>(address: Address, data: Value) -> Result<Bytes> {
//     trace!(target: "rpc::eth", ?address, ?data, "Serving eth_signTypedData");
//     Err(Error::from("unimplemented"))
// }

// /// Handler for: `eth_getBlockReceipts`
// async fn block_receipts<C: Collection>(
//     number: BlockNumberOrTag,
// ) -> Result<Option<Vec<TransactionReceipt>>> { trace!(target: "rpc::eth", ?number, "Serving
//   eth_getBlockReceipts"); Ok(EthApi::block_receipts(self, number).await?)
// }

// /// Handler for: `eth_getTransactionReceipt`
// async fn transaction_receipt<C: Collection>(hash: B256) -> Result<Option<TransactionReceipt>> {
//     trace!(target: "rpc::eth", ?hash, "Serving eth_getTransactionReceipt");
//     Ok(EthTransactions::transaction_receipt(self, hash).await?)
// }

// /// Handler for: `eth_getTransactionByHash`
// async fn transaction_by_hash<C: Collection>(
//     hash: B256,
// ) -> Result<Option<reth_rpc_types::Transaction>> { trace!(target: "rpc::eth", ?hash, "Serving
//   eth_getTransactionByHash"); Ok(EthTransactions::transaction_by_hash(self, hash) .await?
//   .map(Into::into))
// }

// /// Handler for: `eth_call`
// async fn call<C: Collection>(
//     request: CallRequest,
//     block_number: Option<BlockId>,
//     state_overrides: Option<StateOverride>,
//     block_overrides: Option<Box<BlockOverrides>>,
// ) -> Result<Bytes> { trace!(target: "rpc::eth", ?request, ?block_number, ?state_overrides,
//   ?block_overrides, "Serving eth_call"); Ok(self .call( request, block_number,
//   EvmOverrides::new(state_overrides, block_overrides), ) .await?)
// }

// /// Handler for: `eth_callMany`
// async fn call_many<C: Collection>(
//     bundle: Bundle,
//     state_context: Option<StateContext>,
//     state_override: Option<StateOverride>,
// ) -> Result<Vec<EthCallResponse>> { trace!(target: "rpc::eth", ?bundle, ?state_context,
//   ?state_override, "Serving eth_callMany"); Ok(EthApi::call_many(self, bundle, state_context,
//   state_override).await?)
// }

// /// Handler for: `eth_getBlockByHash`
// async fn block_by_hash<C: Collection>(hash: B256, full: bool) -> Result<Option<RichBlock>> {
//     trace!(target: "rpc::eth", ?hash, ?full, "Serving eth_getBlockByHash");
//     Ok(EthApi::rpc_block(self, hash, full).await?)
// }

// /// Handler for: `eth_createAccessList`
// async fn create_access_list<C: Collection>(
//     mut request: CallRequest,
//     block_number: Option<BlockId>,
// ) -> Result<AccessListWithGasUsed> { trace!(target: "rpc::eth", ?request, ?block_number, "Serving
//   eth_createAccessList"); let block_id =
//   block_number.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest)); let access_list = self
//   .create_access_list_at(request.clone(), block_number) .await?; request.access_list =
//   Some(access_list.clone()); let gas_used = self.estimate_gas_at(request, block_id).await?;
//   Ok(AccessListWithGasUsed { access_list, gas_used, })
// }

// /// Handler for: `eth_getBlockTransactionCountByHash`
// async fn block_transaction_count_by_hash<C: Collection>(hash: B256) -> Result<Option<U256>> {
//     trace!(target: "rpc::eth", ?hash, "Serving eth_getBlockTransactionCountByHash");
//     Ok(EthApi::block_transaction_count(self, hash)
//         .await?
//         .map(U256::from))
// }

// /// Handler for: `eth_getBlockTransactionCountByNumber`
// async fn block_transaction_count_by_number<C: Collection>(
//     number: BlockNumberOrTag,
// ) -> Result<Option<U256>> { trace!(target: "rpc::eth", ?number, "Serving
//   eth_getBlockTransactionCountByNumber"); Ok(EthApi::block_transaction_count(self, number)
//   .await? .map(U256::from))
// }

// /// Handler for: `eth_getUncleCountByBlockHash`
// async fn block_uncles_count_by_hash<C: Collection>(hash: B256) -> Result<Option<U256>> {
//     trace!(target: "rpc::eth", ?hash, "Serving eth_getUncleCountByBlockHash");
//     Ok(EthApi::ommers(self, hash)?.map(|ommers| U256::from(ommers.len())))
// }

// /// Handler for: `eth_getUncleCountByBlockNumber`
// async fn block_uncles_count_by_number<C: Collection>(
//     number: BlockNumberOrTag,
// ) -> Result<Option<U256>> { trace!(target: "rpc::eth", ?number, "Serving
//   eth_getUncleCountByBlockNumber"); Ok(EthApi::ommers(self, number)?.map(|ommers|
//   U256::from(ommers.len())))
// }

// /// Handler for: `eth_getUncleByBlockHashAndIndex`
// async fn uncle_by_block_hash_and_index<C: Collection>(
//     hash: B256,
//     index: Index,
// ) -> Result<Option<RichBlock>> { trace!(target: "rpc::eth", ?hash, ?index, "Serving
//   eth_getUncleByBlockHashAndIndex"); Ok(EthApi::ommer_by_block_and_index(self, hash,
//   index).await?)
// }

// /// Handler for: `eth_getUncleByBlockNumberAndIndex`
// async fn uncle_by_block_number_and_index<C: Collection>(
//     number: BlockNumberOrTag,
//     index: Index,
// ) -> Result<Option<RichBlock>> { trace!(target: "rpc::eth", ?number, ?index, "Serving
//   eth_getUncleByBlockNumberAndIndex"); Ok(EthApi::ommer_by_block_and_index(self, number,
//   index).await?)
// }

// /// Handler for: `eth_getTransactionByBlockHashAndIndex`
// async fn transaction_by_block_hash_and_index<C: Collection>(
//     hash: B256,
//     index: Index,
// ) -> Result<Option<reth_rpc_types::Transaction>> { trace!(target: "rpc::eth", ?hash, ?index,
//   "Serving eth_getTransactionByBlockHashAndIndex");
//   Ok(EthApi::transaction_by_block_and_tx_index(self, hash, index).await?)
// }

// /// Handler for: `eth_getTransactionByBlockNumberAndIndex`
// async fn transaction_by_block_number_and_index<C: Collection>(
//     number: BlockNumberOrTag,
//     index: Index,
// ) -> Result<Option<reth_rpc_types::Transaction>> { trace!(target: "rpc::eth", ?number, ?index,
//   "Serving eth_getTransactionByBlockNumberAndIndex");
//   Ok(EthApi::transaction_by_block_and_tx_index(self, number, index).await?)
// }

// /// Handler for: `eth_getStorageAt`
// async fn storage_at<C: Collection>(
//     address: Address,
//     index: JsonStorageKey,
//     block_number: Option<BlockId>,
// ) -> Result<B256> { trace!(target: "rpc::eth", ?address, ?block_number, "Serving
//   eth_getStorageAt"); Ok(self .on_blocking_task(|this| async move { this.storage_at(address,
//   index, block_number) }) .await?)
// }
