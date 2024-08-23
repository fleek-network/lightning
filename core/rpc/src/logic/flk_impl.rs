use std::sync::Arc;
use std::time::Duration;

use fleek_crypto::{EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use jsonrpsee::core::{RpcResult, SubscriptionResult};
use jsonrpsee::{PendingSubscriptionSink, SubscriptionMessage};
use lightning_application::state::ApplicationMerklizeProvider;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Epoch,
    EpochInfo,
    EventType,
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeInfoWithIndex,
    NodeServed,
    OriginProvider,
    ProtocolParams,
    PublicKeys,
    ReportedReputationMeasurements,
    TotalServed,
    TransactionRequest,
    Value,
};
use lightning_interfaces::PagingParams;
use lightning_types::{StateProofKey, StateProofValue};
use lightning_utils::application::QueryRunnerExt;
use merklize::{MerklizeProvider, StateRootHash};

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

    async fn get_flk_balance(&self, pk: EthAddress, epoch: Option<u64>) -> RpcResult<HpUfixed<18>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_account_info::<HpUfixed<18>>(&pk, |a| a.flk_balance)
            .unwrap_or(HpUfixed::zero()))
    }

    async fn get_bandwidth_balance(&self, pk: EthAddress, epoch: Option<u64>) -> RpcResult<u128> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_account_info::<u128>(&pk, |a| a.bandwidth_balance)
            .unwrap_or(0))
    }

    async fn get_locked(&self, pk: NodePublicKey, epoch: Option<u64>) -> RpcResult<HpUfixed<18>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| {
                self.data
                    .query_runner
                    .get_node_info::<HpUfixed<18>>(&node_idx, |n| n.stake.locked)
            })
            .unwrap_or(HpUfixed::<18>::zero()))
    }

    async fn get_staked(&self, pk: NodePublicKey, epoch: Option<u64>) -> RpcResult<HpUfixed<18>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| {
                self.data
                    .query_runner
                    .get_node_info::<HpUfixed<18>>(&node_idx, |n| n.stake.staked)
            })
            .unwrap_or(HpUfixed::<18>::zero()))
    }

    async fn get_stables_balance(
        &self,
        pk: EthAddress,
        epoch: Option<u64>,
    ) -> RpcResult<HpUfixed<6>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_account_info::<HpUfixed<6>>(&pk, |a| a.stables_balance)
            .unwrap_or(HpUfixed::<6>::zero()))
    }

    async fn get_stake_locked_until(
        &self,
        pk: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<u64> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| {
                self.data
                    .query_runner
                    .get_node_info::<Epoch>(&node_idx, |n| n.stake.stake_locked_until)
            })
            .unwrap_or(0))
    }

    async fn get_locked_time(&self, pk: NodePublicKey, epoch: Option<u64>) -> RpcResult<u64> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| {
                self.data
                    .query_runner
                    .get_node_info::<Epoch>(&node_idx, |n| n.stake.locked_until)
            })
            .unwrap_or(0))
    }

    async fn get_node_info(
        &self,
        pk: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<Option<NodeInfo>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| {
                self.data
                    .query_runner
                    .get_node_info::<NodeInfo>(&node_idx, |n| n)
            }))
    }

    async fn get_node_info_epoch(&self, pk: NodePublicKey) -> RpcResult<(Option<NodeInfo>, Epoch)> {
        Ok((
            self.data
                .query_runner
                .pubkey_to_index(&pk)
                .and_then(|node_idx| {
                    self.data
                        .query_runner
                        .get_node_info::<NodeInfo>(&node_idx, |n| n)
                }),
            self.data.query_runner.get_epoch_info().epoch,
        ))
    }

    async fn get_public_keys(&self) -> RpcResult<PublicKeys> {
        Ok(PublicKeys {
            node_public_key: self.data.node_public_key,
            consensus_public_key: self.data.consensus_public_key,
        })
    }

    async fn get_node_uptime(&self, pk: NodePublicKey) -> RpcResult<Option<u8>> {
        let uptime = self
            .data
            .query_runner
            .pubkey_to_index(&pk)
            .and_then(|node_idx| self.data.query_runner.get_node_uptime(&node_idx));
        Ok(uptime)
    }

    async fn get_account_info(
        &self,
        pk: EthAddress,
        epoch: Option<u64>,
    ) -> RpcResult<Option<AccountInfo>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_account_info::<AccountInfo>(&pk, |a| a))
    }

    async fn get_staking_amount(&self) -> RpcResult<u128> {
        Ok(self.data.query_runner.get_staking_amount())
    }

    async fn get_committee_members(&self, epoch: Option<u64>) -> RpcResult<Vec<NodePublicKey>> {
        Ok(self.data.query_runner(epoch).await?.get_committee_members())
    }

    async fn get_genesis_committee(&self) -> RpcResult<Vec<(NodeIndex, NodeInfo)>> {
        Ok(self.data.query_runner.get_genesis_committee())
    }

    async fn get_epoch(&self) -> RpcResult<u64> {
        let epoch = match self.data.query_runner.get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };
        Ok(epoch)
    }

    async fn get_epoch_info(&self) -> RpcResult<EpochInfo> {
        Ok(self.data.query_runner.get_epoch_info())
    }

    async fn get_total_supply(&self, epoch: Option<u64>) -> RpcResult<HpUfixed<18>> {
        let total_supply = match self
            .data
            .query_runner(epoch)
            .await?
            .get_metadata(&Metadata::TotalSupply)
        {
            Some(Value::HpUfixed(supply)) => supply,
            _ => panic!("TotalSupply is set genesis and should never be empty"),
        };

        Ok(total_supply)
    }

    async fn get_year_start_supply(&self, epoch: Option<u64>) -> RpcResult<HpUfixed<18>> {
        let year_start_supply = match self
            .data
            .query_runner(epoch)
            .await?
            .get_metadata(&Metadata::SupplyYearStart)
        {
            Some(Value::HpUfixed(supply)) => supply,
            _ => panic!("SupplyYearStart is set genesis and should never be empty"),
        };
        Ok(year_start_supply)
    }

    async fn get_protocol_fund_address(&self) -> RpcResult<EthAddress> {
        let protocol_fund_address = match self
            .data
            .query_runner
            .get_metadata(&Metadata::ProtocolFundAddress)
        {
            Some(Value::AccountPublicKey(address)) => address,
            _ => panic!("AccountPublicKey is set genesis and should never be empty"),
        };

        Ok(protocol_fund_address)
    }

    async fn get_protocol_params(&self, protocol_params: ProtocolParams) -> RpcResult<u128> {
        Ok(self
            .data
            .query_runner
            .get_protocol_param(&protocol_params)
            .unwrap_or(0))
    }

    async fn get_total_served(&self, epoch: Epoch) -> RpcResult<TotalServed> {
        Ok(self
            .data
            .query_runner
            .get_total_served(&epoch)
            .unwrap_or_default())
    }

    async fn get_node_served(
        &self,
        pk: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<NodeServed> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| self.data.query_runner.get_current_epoch_served(&node_idx))
            .unwrap_or_default())
    }

    async fn is_valid_node(&self, pk: NodePublicKey) -> RpcResult<bool> {
        Ok(self.data.query_runner.is_valid_node(&pk))
    }

    async fn is_valid_node_epoch(&self, pk: NodePublicKey) -> RpcResult<(bool, Epoch)> {
        Ok((
            self.data.query_runner.is_valid_node(&pk),
            self.data.query_runner.get_epoch_info().epoch,
        ))
    }

    async fn get_node_registry(&self, paging: Option<PagingParams>) -> RpcResult<Vec<NodeInfo>> {
        Ok(self
            .data
            .query_runner
            .get_node_registry(paging)
            .into_iter()
            .map(|n| n.info)
            .collect())
    }

    async fn get_node_registry_index(
        &self,
        paging: Option<PagingParams>,
    ) -> RpcResult<Vec<NodeInfoWithIndex>> {
        Ok(self.data.query_runner.get_node_registry(paging))
    }

    async fn get_reputation(&self, pk: NodePublicKey, epoch: Option<u64>) -> RpcResult<Option<u8>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| self.data.query_runner.get_reputation_score(&node_idx)))
    }

    async fn get_reputation_measurements(
        &self,
        pk: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<Vec<ReportedReputationMeasurements>> {
        let rep_measurements = self
            .data
            .query_runner(epoch)
            .await?
            .pubkey_to_index(&pk)
            .and_then(|node_idx| {
                self.data
                    .query_runner
                    .get_reputation_measurements(&node_idx)
            })
            .unwrap_or_default();
        Ok(rep_measurements)
    }

    async fn get_latencies(
        &self,
        epoch: Option<u64>,
    ) -> RpcResult<Vec<((NodePublicKey, NodePublicKey), Duration)>> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_current_latencies()
            .into_iter()
            .collect())
    }

    async fn get_last_epoch_hash(&self) -> RpcResult<([u8; 32], Epoch)> {
        let last_epoch_hash = match self
            .data
            .query_runner
            .get_metadata(&Metadata::LastEpochHash)
        {
            Some(Value::Hash(hash)) => hash,
            _ => [0; 32],
        };
        Ok((
            last_epoch_hash,
            self.data.query_runner.get_epoch_info().epoch,
        ))
    }

    async fn get_sub_dag_index(&self) -> RpcResult<(u64, Epoch)> {
        let sub_dag_index = match self.data.query_runner.get_metadata(&Metadata::SubDagIndex) {
            Some(Value::SubDagIndex(index)) => index,
            _ => 0,
        };
        Ok((sub_dag_index, self.data.query_runner.get_epoch_info().epoch))
    }

    async fn send_txn(&self, tx: TransactionRequest) -> RpcResult<()> {
        Ok(self
            .data
            .mempool_socket
            .enqueue(tx)
            .await
            .map_err(|e| RPCError::socket(e.to_string()))?)
    }

    async fn get_state_root(&self, epoch: Option<u64>) -> RpcResult<StateRootHash> {
        Ok(self
            .data
            .query_runner(epoch)
            .await?
            .get_state_root()
            .map_err(|e| RPCError::custom(e.to_string()))?)
    }

    async fn get_state_proof(
        &self,
        key: StateProofKey,
        epoch: Option<u64>,
    ) -> RpcResult<(
        Option<StateProofValue>,
        <ApplicationMerklizeProvider as MerklizeProvider>::Proof,
    )> {
        let (value, proof) = self
            .data
            .query_runner(epoch)
            .await?
            .get_state_proof(key)
            .map_err(|e| RPCError::custom(e.to_string()))?;
        Ok((value, proof))
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
            Err(
                RPCError::custom("Put returned a fetch response, this is a bug.".to_string())
                    .into(),
            )
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

    async fn handle_subscription(
        &self,
        pending: PendingSubscriptionSink,
        event_type: Option<EventType>,
    ) -> SubscriptionResult {
        let sink = pending.accept().await?;

        let mut rx = self.data.events.subscribe();

        while let Ok(events) = rx.recv().await {
            for event in events {
                if let Some(ref typee) = event_type {
                    if &event.event_type() != typee {
                        continue;
                    }
                }
                tracing::trace!(?event, "sending event to subscriber");

                if sink
                    .send(SubscriptionMessage::from_json(&event)?)
                    .await
                    .is_err()
                {
                    tracing::trace!("flk subscription closed");
                    break;
                }
            }
        }

        Ok(())
    }
}
