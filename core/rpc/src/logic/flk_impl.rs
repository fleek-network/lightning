use std::sync::Arc;
use std::time::Duration;

use fleek_crypto::{EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use jsonrpsee::core::RpcResult;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Epoch,
    EpochInfo,
    NodeIndex,
    NodeInfo,
    NodeInfoWithIndex,
    NodeServed,
    ProtocolParams,
    ReportedReputationMeasurements,
    TotalServed,
    TransactionRequest, FetcherRequest, FetcherResponse, ImmutablePointer, OriginProvider,
};
use lightning_interfaces::{PagingParams, SyncQueryRunnerInterface};

use crate::api::FleekApiServer;
use crate::error::RPCError;
use crate::Data;

pub struct FleekApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl<C: Collection> FleekApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> FleekApiServer for FleekApi<C> {
    async fn ping(&self) -> RpcResult<String> {
        Ok("pong".to_string())
    }

    async fn get_flk_balance(&self, pk: EthAddress) -> RpcResult<HpUfixed<18>> {
        Ok(self.data.query_runner.get_flk_balance(&pk))
    }

    async fn get_bandwidth_balance(&self, pk: EthAddress) -> RpcResult<u128> {
        Ok(self.data.query_runner.get_account_balance(&pk))
    }

    async fn get_locked(&self, pk: NodePublicKey) -> RpcResult<HpUfixed<18>> {
        Ok(self.data.query_runner.get_locked(&pk))
    }

    async fn get_staked(&self, pk: NodePublicKey) -> RpcResult<HpUfixed<18>> {
        Ok(self.data.query_runner.get_staked(&pk))
    }

    async fn get_stables_balance(&self, pk: EthAddress) -> RpcResult<HpUfixed<6>> {
        Ok(self.data.query_runner.get_stables_balance(&pk))
    }

    async fn get_stake_locked_until(&self, pk: NodePublicKey) -> RpcResult<u64> {
        Ok(self
            .data
            .query_runner
            .get_stake_locked_until(&pk))
    }

    async fn get_locked_time(&self, pk: NodePublicKey) -> RpcResult<u64> {
        Ok(self.data.query_runner.get_locked_time(&pk))
    }

    async fn get_node_info(&self, pk: NodePublicKey) -> RpcResult<Option<NodeInfo>> {
        Ok(self.data.query_runner.get_node_info(&pk))
    }

    async fn get_account_info(&self, pk: EthAddress) -> RpcResult<Option<AccountInfo>> {
        Ok(self.data.query_runner.get_account_info(&pk))
    }

    async fn get_staking_amount(&self) -> RpcResult<u128> {
        Ok(self.data.query_runner.get_staking_amount())
    }

    async fn get_committee_members(&self) -> RpcResult<Vec<NodePublicKey>> {
        Ok(self.data.query_runner.get_committee_members())
    }

    async fn get_genesis_committee(&self) -> RpcResult<Vec<(NodeIndex, NodeInfo)>> {
        Ok(self.data.query_runner.genesis_committee())
    }

    async fn get_epoch(&self) -> RpcResult<u64> {
        Ok(self.data.query_runner.get_epoch())
    }

    async fn get_epoch_info(&self) -> RpcResult<EpochInfo> {
        Ok(self.data.query_runner.get_epoch_info())
    }

    async fn get_total_supply(&self) -> RpcResult<HpUfixed<18>> {
        Ok(self.data.query_runner.get_total_supply())
    }

    async fn get_year_start_supply(&self) -> RpcResult<HpUfixed<18>> {
        Ok(self.data.query_runner.get_year_start_supply())
    }

    async fn get_protocol_fund_address(&self) -> RpcResult<EthAddress> {
        Ok(self.data.query_runner.get_protocol_fund_address())
    }

    async fn get_protocol_params(&self, protocol_params: ProtocolParams) -> RpcResult<u128> {
        Ok(self.data.query_runner.get_protocol_params(protocol_params))
    }

    async fn get_total_served(&self, epoch: Epoch) -> RpcResult<TotalServed> {
        Ok(self.data.query_runner.get_total_served(epoch))
    }

    async fn get_node_served(&self, pk: NodePublicKey) -> RpcResult<NodeServed> {
        Ok(self.data.query_runner.get_node_served(&pk))
    }

    async fn is_valid_node(&self, pk: NodePublicKey) -> RpcResult<bool> {
        Ok(self.data.query_runner.is_valid_node(&pk))
    }

    async fn get_node_registry(&self, paging: Option<PagingParams>) -> RpcResult<Vec<NodeInfo>> {
        Ok(self.data.query_runner.get_node_registry(paging))
    }

    async fn get_node_registry_index(
        &self,
        paging: Option<PagingParams>,
    ) -> RpcResult<Vec<NodeInfoWithIndex>> {
        Ok(self.data.query_runner.get_node_registry_with_index(paging))
    }

    async fn get_reputation(&self, pk: NodePublicKey) -> RpcResult<Option<u8>> {
        if let Some(index) = self.data.query_runner.pubkey_to_index(pk) {
            Ok(self.data.query_runner.get_reputation(&index))
        } else {
            Ok(None)
        }
    }

    async fn get_reputation_measurements(
        &self,
        pk: NodePublicKey,
    ) -> RpcResult<Vec<ReportedReputationMeasurements>> {
        if let Some(index) = self.data.query_runner.pubkey_to_index(pk) {
            Ok(self.data.query_runner.get_rep_measurements(&index))
        } else {
            Ok(vec![])
        }
    }

    async fn get_latencies(&self) -> RpcResult<Vec<((NodePublicKey, NodePublicKey), Duration)>> {
        Ok(self
            .data
            .query_runner
            .get_latencies()
            .into_iter()
            .map(|(key, val)| (key, val))
            .collect())
    }

    async fn get_last_epoch_hash(&self) -> RpcResult<[u8; 32]> {
        Ok(self.data.query_runner.get_last_epoch_hash())
    }

    async fn send_txn(&self, tx: TransactionRequest) -> RpcResult<()> {
        Ok(self
            .data
            .mempool_socket
            .run(tx)
            .await
            .map_err(RPCError::from)?)
    }

    async fn put(&self, data: Vec<u8>) -> RpcResult<Blake3Hash> {
        let pointer = ImmutablePointer {
            origin: OriginProvider::IPFS,
            uri: data,
        };

        let res = self
            .data
            .fetcher_socket
            .run(FetcherRequest::Put { pointer })
            .await
            .map_err(RPCError::from)?;
        
        if let FetcherResponse::Put(res) = res {
            match res {
                Ok(hash) => Ok(hash),
                Err(err) => Err(RPCError::custom(err.to_string()).into()),
            }
        } else {
            Err(RPCError::custom("Put returned a fetch response, this is a bug.".to_string()).into())
        }
    }

    async fn health(&self) -> RpcResult<String> {
        Ok("OK".to_string())
    }

    async fn metrics(&self) -> RpcResult<String> {
        match autometrics::prometheus_exporter::encode_to_string() {
            Ok(metrics) => Ok(metrics),
            Err(err) => Err(RPCError::custom(err.to_string()).into()),
        }
    }
}
