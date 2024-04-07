use std::sync::Arc;

use alloy_primitives::U64;
use ethers::types::{
    Address,
    Block,
    BlockNumber,
    Bytes,
    Transaction,
    TransactionReceipt,
    TransactionRequest,
    H256,
    U256,
};
use ethers::utils::rlp;
use fleek_crypto::EthAddress;
use hp_fixed::unsigned::HpUfixed;
use jsonrpsee::core::RpcResult;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ArchiveInterface, SyncQueryRunnerInterface};
use lightning_utils::application::QueryRunnerExt;
use tracing::trace;

use crate::api::EthApiServer;
use crate::api_types::{CallRequest, StateOverride};
use crate::error::RPCError;
use crate::Data;

const FLEEK_CONTRACT: EthAddress = EthAddress([6; 20]);

const FLEEK_CONTRACT_BYTES: &[u8; 172] = b"73000000000000000000000000000000000000000030146080604052600080fdfea264697066735822122012d3570051ca11eb882745693b7b2af91a10ad5074b3486da80280731d9af73164736f6c63430008120033";

use lightning_interfaces::types::{Metadata, Value};

pub struct EthApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl<C: Collection> EthApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> EthApiServer for EthApi<C> {
    async fn block_number(&self) -> RpcResult<U256> {
        trace!(target: "rpc::eth", "Serving eth_blockNumber");

        let block_number = match self.data.query_runner.get_metadata(&Metadata::BlockNumber) {
            Some(Value::BlockNumber(num)) => num,
            _ => 0,
        };

        Ok(U256::from(block_number))
    }

    /// todo this function always returns the current nonce
    async fn transaction_count(
        &self,
        address: EthAddress,
        _block: Option<BlockNumber>,
        epoch: Option<u64>,
    ) -> RpcResult<U256> {
        trace!(target: "rpc::eth", ?address, "Serving eth_getTransactionCount");

        Ok(U256::from(
            self.data
                .query_runner(epoch)
                .await?
                .get_account_info::<u64>(&address, |a| a.nonce)
                .unwrap_or(0)
                + 1,
        ))
    }

    async fn balance(
        &self,
        address: EthAddress,
        block_number: Option<BlockNumber>,
        epoch: Option<u64>,
    ) -> RpcResult<U256> {
        trace!(target: "rpc::eth", ?address, ?block_number, "Serving eth_getBalance");
        // Todo(dalton) direct safe conversion from hpfixed => u128
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_account_info::<HpUfixed<18>>(&address, |a| a.flk_balance)
            .unwrap_or(HpUfixed::<18>::zero())
            .into())
    }

    async fn protocol_version(&self) -> RpcResult<U64> {
        trace!(target: "rpc::eth", "Serving eth_protocolVersion");
        Ok(U64::from(0))
    }

    async fn chain_id(&self) -> RpcResult<Option<U64>> {
        trace!(target: "rpc::eth", "Serving eth_chainId");
        Ok(Some(U64::from(self.data.query_runner.get_chain_id())))
    }

    /// todo(n)
    async fn syncing(&self) -> RpcResult<bool> {
        trace!(target: "rpc::eth", "Serving eth_syncing");
        Ok(false)
    }

    async fn block_by_number(
        &self,
        number: BlockNumber,
        full: bool,
    ) -> RpcResult<Option<Block<H256>>> {
        trace!(target: "rpc::eth", ?number, ?full, "Serving eth_getBlockByNumber");

        if !self.data.archive.is_active() {
            return Err(RPCError::custom("Archive socket not initialized".to_string()).into());
        }

        Ok(self
            .data
            .archive
            .get_block_by_number(number)
            .await
            .map(|x| x.into()))
    }

    async fn block_by_hash(&self, hash: H256, full: bool) -> RpcResult<Option<Block<H256>>> {
        trace!(target: "rpc::eth", ?hash, ?full, "Serving eth_getBlockByHash");

        if !self.data.archive.is_active() {
            return Err(RPCError::custom("Archive socket not initialized".to_string()).into());
        }

        Ok(self
            .data
            .archive
            .get_block_by_hash(hash.into())
            .await
            .map(|x| x.into()))
    }

    async fn transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>> {
        trace!(target: "rpc::eth", ?hash, "Serving eth_getTransactionReceipt");

        if !self.data.archive.is_active() {
            return Err(RPCError::custom("Archive socket not initialized".to_string()).into());
        }

        Ok(self
            .data
            .archive
            .get_transaction_receipt(hash.into())
            .await
            .map(|x| x.into()))
    }

    async fn send_raw_transaction(&self, tx: Bytes) -> RpcResult<H256> {
        trace!(target: "rpc::eth", "Serving eth_sendRawTransaction");
        let tx_data = tx.as_ref();

        let mut transaction = rlp::decode::<Transaction>(tx_data).map_err(RPCError::from)?;

        transaction.recover_from_mut().map_err(RPCError::from)?;

        let hash = transaction.hash();

        self.data
            .mempool_socket
            .run(transaction.into())
            .await
            .map_err(RPCError::from)?;

        Ok(hash)
    }

    /// Todo(n)
    async fn gas_price(&self) -> RpcResult<U256> {
        trace!(target: "rpc::eth", "Serving eth_gasPrice");
        Ok(U256::zero())
    }

    /// todo(n)
    async fn estimate_gas(&self, tx: CallRequest) -> RpcResult<U256> {
        trace!(target: "rpc::eth", ?tx, "Serving eth_estimateGas");
        Ok(U256::from(1000000))
    }

    /// todo(n)
    async fn code(
        &self,
        address: EthAddress,
        block_number: Option<BlockNumber>,
    ) -> RpcResult<Bytes> {
        trace!(target: "rpc::eth", ?address, ?block_number, "Serving eth_getCode");

        if address == FLEEK_CONTRACT {
            Ok(Bytes::from(FLEEK_CONTRACT_BYTES))
        } else {
            Ok(Bytes::new())
        }
    }

    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        trace!(target: "rpc::eth", "Serving eth_maxPriorityFeePerGas");
        Ok(U256::zero())
    }

    async fn fee_history(&self) -> RpcResult<String> {
        Err(RPCError::unimplemented().into())
    }

    /// todo(n)
    async fn storage_at(
        &self,
        _address: EthAddress,
        _index: U256,
        _block_number: Option<BlockNumber>,
    ) -> RpcResult<Bytes> {
        Err(RPCError::unimplemented().into())
    }

    async fn transaction_by_hash(&self, _hash: H256) -> RpcResult<String> {
        Err(RPCError::unimplemented().into())
    }

    async fn mining(&self) -> RpcResult<bool> {
        Err(RPCError::unimplemented().into())
    }

    async fn hashrate(&self) -> RpcResult<U256> {
        Err(RPCError::unimplemented().into())
    }

    async fn get_work(&self) -> RpcResult<Vec<String>> {
        Err(RPCError::unimplemented().into())
    }

    async fn submit_hashrate(&self, _hash_rate: U256, _client_id: String) -> RpcResult<bool> {
        Err(RPCError::unimplemented().into())
    }

    async fn submit_work(&self, _nonce: U256, _pow_hash: H256, _mix_hash: H256) -> RpcResult<bool> {
        Err(RPCError::unimplemented().into())
    }

    async fn send_transaction(&self, _tx: TransactionRequest) -> RpcResult<H256> {
        Err(RPCError::unimplemented().into())
    }

    async fn coinbase(&self) -> RpcResult<Address> {
        Err(RPCError::unimplemented().into())
    }

    async fn accounts(&self) -> RpcResult<Vec<Address>> {
        Err(RPCError::unimplemented().into())
    }

    /// todo(n)
    async fn call(
        &self,
        _tx: TransactionRequest,
        _block_number: Option<BlockNumber>,
        _state_overrides: Option<StateOverride>,
    ) -> RpcResult<Bytes> {
        Err(RPCError::unimplemented().into())
    }

    async fn sign(&self, _address: EthAddress, _data: Bytes) -> RpcResult<Bytes> {
        Err(RPCError::unimplemented().into())
    }

    async fn sign_transaction(&self, _tx: TransactionRequest) -> RpcResult<Bytes> {
        Err(RPCError::unimplemented().into())
    }
}
