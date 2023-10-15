use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use autometrics::autometrics;
use axum::{http, Extension, Json};
use fleek_crypto::{EthAddress, NodePublicKey, SecretKey, TransactionSender, TransactionSignature};
use hp_fixed::unsigned::HpUfixed;
use http::StatusCode;
use jsonrpc_v2::{Data, Error, MapRouter, Params, RequestObject, ResponseObjects, Server};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    EpochInfo,
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    NodeIndex,
    NodeInfo,
    NodeServed,
    OriginProvider,
    ProtocolParams,
    ReportedReputationMeasurements,
    TotalServed,
    TransactionRequest,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::{SyncQueryRunnerInterface, ToDigest};
use tracing::error;

use crate::eth::{
    eth_accounts,
    eth_author,
    eth_balance,
    eth_block_by_hash,
    eth_block_by_number,
    eth_block_number,
    eth_chain_id,
    eth_estimate_gas,
    eth_fee_history,
    eth_gas_price,
    eth_get_code,
    eth_get_work,
    eth_hashrate,
    eth_is_mining,
    eth_max_priority_fee_per_gas,
    eth_protocol_version,
    eth_send_raw_transaction,
    eth_send_transaction,
    eth_sign,
    eth_sign_transaction,
    eth_submit_hashrate,
    eth_submit_work,
    eth_syncing,
    eth_transaction_count,
    eth_transaction_receipt,
    net_listening,
    net_peer_count,
    net_version,
};
use crate::server::RpcData;
use crate::types::{NodeKeyParam, PublicKeyParam};
static OPEN_RPC_DOCS: &str = "../../docs/rpc/openrpc.json";

pub type Result<T> = anyhow::Result<T, Error>;

pub async fn rpc_handler(
    Extension(rpc): Extension<RpcServer>,
    Json(req): Json<RequestObject>,
) -> Json<ResponseObjects> {
    error!("Someone called: {}", req.method_ref());
    let res = rpc.0.handle(req).await;
    Json(res)
}

/// Export metrics for Prometheus to scrape
pub async fn get_metrics() -> (StatusCode, String) {
    match autometrics::prometheus_exporter::encode_to_string() {
        Ok(metrics) => (StatusCode::OK, metrics),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")),
    }
}

#[derive(Clone)]
pub struct RpcServer(Arc<Server<MapRouter>>);

impl RpcServer {
    pub fn new<C>(interface: Arc<RpcData<C>>) -> Self
    where
        C: Collection + 'static,
    {
        let server = Server::new()
            .with_data(Data::new(interface))
            // ONLY TESTNET
            .with_method("flk_mint", mint_handler::<C>)
            .with_method("rpc.discover", rpc_discovery_handler::<C>)
            .with_method("flk_ping", ping_handler::<C>)
            .with_method("flk_get_flk_balance", get_flk_balance_handler::<C>)
            .with_method(
                "flk_get_bandwidth_balance",
                get_bandwidth_balance_handler::<C>,
            )
            .with_method("flk_get_locked", get_locked_handler::<C>)
            .with_method("flk_get_staked", get_staked_handler::<C>)
            .with_method("flk_get_stables_balance", get_stables_balance_handler::<C>)
            .with_method(
                "flk_get_stake_locked_until",
                get_stake_locked_until_handler::<C>,
            )
            .with_method("flk_get_locked_time", get_locked_time_handler::<C>)
            .with_method("flk_get_node_info", get_node_info_handler::<C>)
            .with_method("flk_get_account_info", get_account_info_handler::<C>)
            .with_method("flk_get_staking_amount", get_staking_amount_handler::<C>)
            .with_method(
                "flk_get_committee_members",
                get_committee_members_handler::<C>,
            )
            .with_method("flk_get_epoch", get_epoch_handler::<C>)
            .with_method("flk_get_epoch_info", get_epoch_info_handler::<C>)
            .with_method("flk_get_total_supply", get_total_supply_handler::<C>)
            .with_method(
                "flk_get_year_start_supply",
                get_year_start_supply_handler::<C>,
            )
            .with_method(
                "flk_get_protocol_fund_address",
                get_protocol_fund_address_handler::<C>,
            )
            .with_method("flk_get_protocol_params", get_protocol_params_handler::<C>)
            .with_method("flk_get_total_served", get_total_served_handler::<C>)
            .with_method("flk_get_node_served", get_node_served_handler::<C>)
            .with_method("flk_is_valid_node", is_valid_node_handler::<C>)
            .with_method("flk_get_node_registry", get_node_registry_handler::<C>)
            .with_method("flk_get_reputation", get_reputation_handler::<C>)
            .with_method(
                "flk_get_reputation_measurements",
                get_reputation_measurements_handler::<C>,
            )
            .with_method("flk_get_latencies", get_latencies_handler::<C>)
            .with_method("flk_get_last_epoch_hash", get_last_epoch_hash_handler::<C>)
            .with_method("flk_send_txn", send_txn::<C>)
            .with_method("flk_put", put::<C>)
            .with_method("eth_protocolVersion", eth_protocol_version::<C>)
            .with_method("eth_getTransactionCount", eth_transaction_count::<C>)
            .with_method("eth_chainId", eth_chain_id::<C>)
            .with_method("eth_syncing", eth_syncing::<C>)
            .with_method("eth_coinbase", eth_author::<C>)
            .with_method("eth_accounts", eth_accounts::<C>)
            .with_method("eth_getBalance", eth_balance::<C>)
            .with_method("net_version", net_version::<C>)
            .with_method("net_peerCount", net_peer_count::<C>)
            .with_method("net_listening", net_listening::<C>)
            .with_method("eth_blockNumber", eth_block_number::<C>)
            .with_method("eth_getBlockByNumber", eth_block_by_number::<C>)
            .with_method("eth_getBlockByHash", eth_block_by_hash::<C>)
            .with_method("eth_gasPrice", eth_gas_price::<C>)
            .with_method("eth_estimateGas", eth_estimate_gas::<C>)
            .with_method("eth_sendRawTransaction", eth_send_raw_transaction::<C>)
            .with_method("eth_getCode", eth_get_code::<C>)
            .with_method(
                "eth_maxPriorityFeePerGas",
                eth_max_priority_fee_per_gas::<C>,
            )
            .with_method("eth_feeHistory", eth_fee_history::<C>)
            .with_method("eth_mining", eth_is_mining::<C>)
            .with_method("eth_hashrate", eth_hashrate::<C>)
            .with_method("eth_getWork", eth_get_work::<C>)
            .with_method("eth_submitHashrate", eth_submit_hashrate::<C>)
            .with_method("eth_submitWork", eth_submit_work::<C>)
            .with_method("eth_sendTransaction", eth_send_transaction::<C>)
            .with_method("eth_sign", eth_sign::<C>)
            .with_method("eth_signTransaction", eth_sign_transaction::<C>)
            .with_method("eth_getTransactionReceipt", eth_transaction_receipt::<C>);

        RpcServer(server.finish())
    }
}

pub async fn rpc_discovery_handler<C: Collection>() -> Result<String> {
    match fs::read_to_string(OPEN_RPC_DOCS) {
        Ok(contents) => Ok(contents),
        Err(e) => Err(Error::internal(e)),
    }
}

pub async fn put<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<Vec<u8>>,
) -> Result<Blake3Hash> {
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: params,
    };

    let res = data
        .fetcher_socket
        .run(FetcherRequest::Put { pointer })
        .await
        .expect("sending put request failed.");
    if let FetcherResponse::Put(Ok(hash)) = res {
        Ok(hash)
    } else {
        Err(Error::INTERNAL_ERROR)
    }
}

// ONLY TESTNET
pub async fn mint_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<String> {
    if data.query_runner.get_allow_mint() {
        let balance = data.query_runner.get_account_balance(&params.public_key);
        if balance > 1010 {
            return Ok("already minted".to_string());
        }
        let nonce = data.query_runner.get_account_nonce(&params.public_key) + 1;
        let method = UpdateMethod::Mint {
            recipient: params.public_key,
        };
        let payload = UpdatePayload { method, nonce };
        let digest = payload.to_digest();
        let signature = data.node_secret_key.sign(&digest);
        let request = UpdateRequest {
            payload,
            sender: TransactionSender::NodeMain(data.node_secret_key.to_pk()),
            signature: TransactionSignature::NodeMain(signature),
        };

        if let Err(e) = data
            .0
            .mempool_socket
            .run(TransactionRequest::UpdateRequest(request))
            .await
        {
            error!("Failed to send mint transaction: {e:?}");
            return Ok("failed to mint".to_string());
        }
        Ok("minted".to_string())
    } else {
        Ok("minting deactivated".to_string())
    }
}

#[autometrics]
pub async fn ping_handler<C: Collection>() -> Result<String> {
    Ok("pong".to_string())
}

pub async fn get_flk_balance_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_flk_balance(&params.public_key))
}

pub async fn get_stables_balance_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfixed<6>> {
    Ok(data.0.query_runner.get_stables_balance(&params.public_key))
}

pub async fn get_bandwidth_balance_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<u128> {
    Ok(data.0.query_runner.get_account_balance(&params.public_key))
}

pub async fn get_staked_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_staked(&params.public_key))
}

pub async fn get_locked_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_locked(&params.public_key))
}

pub async fn get_stake_locked_until_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u64> {
    Ok(data
        .0
        .query_runner
        .get_stake_locked_until(&params.public_key))
}

pub async fn get_locked_time_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u64> {
    Ok(data.0.query_runner.get_locked_time(&params.public_key))
}

pub async fn get_node_info_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<Option<NodeInfo>> {
    Ok(data.0.query_runner.get_node_info(&params.public_key))
}

pub async fn get_account_info_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<Option<AccountInfo>> {
    Ok(data.0.query_runner.get_account_info(&params.public_key))
}

pub async fn get_reputation_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(node): Params<NodeIndex>,
) -> Result<Option<u8>> {
    Ok(data.0.query_runner.get_reputation(&node))
}

pub async fn get_latencies_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<HashMap<(NodePublicKey, NodePublicKey), Duration>> {
    Ok(data.0.query_runner.get_latencies())
}

pub async fn get_reputation_measurements_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(node): Params<NodeIndex>,
) -> Result<Vec<ReportedReputationMeasurements>> {
    Ok(data.0.query_runner.get_rep_measurements(&node))
}

pub async fn get_staking_amount_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<u128> {
    Ok(data.0.query_runner.get_staking_amount())
}

pub async fn get_committee_members_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<Vec<NodePublicKey>> {
    Ok(data.0.query_runner.get_committee_members())
}

pub async fn get_epoch_handler<C: Collection>(data: Data<Arc<RpcData<C>>>) -> Result<u64> {
    Ok(data.0.query_runner.get_epoch())
}

pub async fn get_epoch_info_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<EpochInfo> {
    Ok(data.0.query_runner.get_epoch_info())
}

pub async fn get_total_supply_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_total_supply())
}

pub async fn get_year_start_supply_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_year_start_supply())
}

pub async fn get_protocol_fund_address_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<EthAddress> {
    Ok(data.0.query_runner.get_protocol_fund_address())
}

pub async fn get_protocol_params_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<ProtocolParams>,
) -> Result<u128> {
    Ok(data.0.query_runner.get_protocol_params(params))
}

pub async fn get_total_served_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<u64>,
) -> Result<TotalServed> {
    Ok(data.0.query_runner.get_total_served(params))
}

pub async fn get_node_served_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<NodeServed> {
    Ok(data.0.query_runner.get_node_served(&params.public_key))
}

pub async fn is_valid_node_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<bool> {
    Ok(data.0.query_runner.is_valid_node(&params.public_key))
}

pub async fn get_node_registry_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<Vec<NodeInfo>> {
    Ok(data.0.query_runner.get_node_registry())
}

pub async fn get_last_epoch_hash_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
) -> Result<[u8; 32]> {
    Ok(data.0.query_runner.get_last_epoch_hash())
}

pub async fn send_txn<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(param): Params<TransactionRequest>,
) -> Result<()> {
    data.0
        .mempool_socket
        .run(param)
        .await
        .map_err(Error::internal)
}
