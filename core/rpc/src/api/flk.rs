use std::time::Duration;

use fleek_crypto::{EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use jsonrpsee::core::{RpcResult, SubscriptionResult};
use jsonrpsee::proc_macros::rpc;
use lightning_application::env::ApplicationStateTree;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Epoch,
    EpochInfo,
    Event,
    EventType,
    NodeIndex,
    NodeInfo,
    NodeInfoWithIndex,
    NodeServed,
    ProtocolParams,
    PublicKeys,
    ReportedReputationMeasurements,
    TotalServed,
    TransactionRequest,
};
use lightning_interfaces::PagingParams;
use lightning_openrpc_macros::open_rpc;
use lightning_types::{StateProofKey, StateProofValue};
use merklize::{StateRootHash, StateTree};

#[open_rpc(namespace = "flk", tag = "1.0.0")]
#[rpc(client, server, namespace = "flk")]
pub trait FleekApi {
    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<String> {
        Ok("pong".to_string())
    }

    #[method(name = "get_flk_balance")]
    async fn get_flk_balance(
        &self,
        public_key: EthAddress,
        epoch: Option<u64>,
    ) -> RpcResult<HpUfixed<18>>;

    #[method(name = "get_bandwidth_balance")]
    async fn get_bandwidth_balance(
        &self,
        public_key: EthAddress,
        epoch: Option<u64>,
    ) -> RpcResult<u128>;

    #[method(name = "get_locked")]
    async fn get_locked(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<HpUfixed<18>>;

    #[method(name = "get_staked")]
    async fn get_staked(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<HpUfixed<18>>;

    #[method(name = "get_stables_balance")]
    async fn get_stables_balance(
        &self,
        public_key: EthAddress,
        epoch: Option<u64>,
    ) -> RpcResult<HpUfixed<6>>;

    #[method(name = "get_stake_locked_until")]
    async fn get_stake_locked_until(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<u64>;

    #[method(name = "get_locked_time")]
    async fn get_locked_time(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<u64>;

    #[method(name = "get_node_info")]
    async fn get_node_info(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<Option<NodeInfo>>;

    #[method(name = "get_node_info_epoch")]
    async fn get_node_info_epoch(
        &self,
        public_key: NodePublicKey,
    ) -> RpcResult<(Option<NodeInfo>, Epoch)>;

    #[method(name = "get_public_keys")]
    async fn get_public_keys(&self) -> RpcResult<PublicKeys>;

    #[method(name = "get_node_uptime")]
    async fn get_node_uptime(&self, public_key: NodePublicKey) -> RpcResult<Option<u8>>;

    #[method(name = "get_account_info")]
    async fn get_account_info(
        &self,
        public_key: EthAddress,
        epoch: Option<u64>,
    ) -> RpcResult<Option<AccountInfo>>;

    #[method(name = "get_staking_amount")]
    async fn get_staking_amount(&self) -> RpcResult<u128>;

    #[method(name = "get_committee_members")]
    async fn get_committee_members(&self, epoch: Option<u64>) -> RpcResult<Vec<NodePublicKey>>;

    #[method(name = "get_genesis_committee")]
    async fn get_genesis_committee(&self) -> RpcResult<Vec<(NodeIndex, NodeInfo)>>;

    #[method(name = "get_epoch")]
    async fn get_epoch(&self) -> RpcResult<u64>;

    #[method(name = "get_epoch_info")]
    async fn get_epoch_info(&self) -> RpcResult<EpochInfo>;

    #[method(name = "get_total_supply")]
    async fn get_total_supply(&self, epoch: Option<u64>) -> RpcResult<HpUfixed<18>>;

    #[method(name = "get_year_start_supply")]
    async fn get_year_start_supply(&self, epoch: Option<u64>) -> RpcResult<HpUfixed<18>>;

    #[method(name = "get_protocol_fund_address")]
    async fn get_protocol_fund_address(&self) -> RpcResult<EthAddress>;

    #[method(name = "get_protocol_params")]
    async fn get_protocol_params(&self, protocol_params: ProtocolParams) -> RpcResult<u128>;

    #[method(name = "get_total_served")]
    async fn get_total_served(&self, epoch: Epoch) -> RpcResult<TotalServed>;

    #[method(name = "get_node_served")]
    async fn get_node_served(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<NodeServed>;

    #[method(name = "is_valid_node")]
    async fn is_valid_node(&self, public_key: NodePublicKey) -> RpcResult<bool>;

    #[method(name = "is_valid_node_epoch")]
    async fn is_valid_node_epoch(&self, public_key: NodePublicKey) -> RpcResult<(bool, Epoch)>;

    #[method(name = "get_node_registry")]
    async fn get_node_registry(&self, paging: Option<PagingParams>) -> RpcResult<Vec<NodeInfo>>;

    #[method(name = "get_node_registry_index")]
    async fn get_node_registry_index(
        &self,
        paging: Option<PagingParams>,
    ) -> RpcResult<Vec<NodeInfoWithIndex>>;

    #[method(name = "get_reputation")]
    async fn get_reputation(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<Option<u8>>;

    #[method(name = "get_reputation_measurements")]
    async fn get_reputation_measurements(
        &self,
        public_key: NodePublicKey,
        epoch: Option<u64>,
    ) -> RpcResult<Vec<ReportedReputationMeasurements>>;

    #[method(name = "get_latencies")]
    async fn get_latencies(
        &self,
        epoch: Option<u64>,
    ) -> RpcResult<Vec<((NodePublicKey, NodePublicKey), Duration)>>;

    #[method(name = "get_last_epoch_hash")]
    async fn get_last_epoch_hash(&self) -> RpcResult<([u8; 32], Epoch)>;

    #[method(name = "get_sub_dag_index")]
    async fn get_sub_dag_index(&self) -> RpcResult<(u64, Epoch)>;

    #[method(name = "get_state_root")]
    async fn get_state_root(&self, epoch: Option<u64>) -> RpcResult<StateRootHash>;

    #[method(name = "get_state_proof")]
    async fn get_state_proof(
        &self,
        key: StateProofKey,
        epoch: Option<u64>,
    ) -> RpcResult<(
        Option<StateProofValue>,
        <ApplicationStateTree as StateTree>::Proof,
    )>;

    #[method(name = "send_txn")]
    async fn send_txn(&self, tx: TransactionRequest) -> RpcResult<()>;

    #[method(name = "put")]
    async fn put(&self, data: Vec<u8>) -> RpcResult<Blake3Hash>;

    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;

    #[method(name = "metrics")]
    async fn metrics(&self) -> RpcResult<String>;

    #[subscription(name = "subscribe", item = Event)]
    async fn handle_subscription(&self, event_type: Option<EventType>) -> SubscriptionResult;
}
