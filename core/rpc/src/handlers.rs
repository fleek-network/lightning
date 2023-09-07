use std::fs;
use std::str::FromStr;
use std::sync::Arc;

use autometrics::autometrics;
use axum::{http, Extension, Json};
use fleek_crypto::{AccountOwnerSignature, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use http::StatusCode;
use jsonrpc_v2::{Data, Error, MapRouter, Params, RequestObject, ResponseObjects, Server};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    AccountInfo,
    EpochInfo,
    ImmutablePointer,
    NodeInfo,
    NodeServed,
    OriginProvider,
    ProtocolParams,
    TotalServed,
    UpdateRequest,
};
#[cfg(feature = "e2e-test")]
use lightning_interfaces::types::{DhtRequest, DhtResponse, KeyPrefix, TableEntry};
use lightning_interfaces::{Blake3Hash, FetcherInterface, SyncQueryRunnerInterface};

use crate::server::RpcData;
#[cfg(feature = "e2e-test")]
use crate::types::{DhtGetParam, DhtPutParam};
use crate::types::{NodeKeyParam, PublicKeyParam, VersionedNodeKeyParam};
static OPEN_RPC_DOCS: &str = "../../docs/rpc/openrpc.json";

pub const RPC_VERSION: u8 = 2;

pub type Result<T> = anyhow::Result<T, Error>;

pub async fn rpc_handler(
    Extension(rpc): Extension<RpcServer>,
    Json(req): Json<RequestObject>,
) -> Json<ResponseObjects> {
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
        #[allow(unused_mut)]
        let mut server = Server::new()
            .with_data(Data::new(interface))
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
            .with_method("flk_get_last_epoch_hash", get_last_epoch_hash_handler::<C>)
            .with_method("flk_send_txn", send_txn::<C>)
            .with_method("flk_put", put::<C>)
            .with_method("testnet_only_kill", kill::<C>);

        #[cfg(feature = "e2e-test")]
        {
            server = server
                .with_method("flk_dht_put", dht_put::<C>)
                .with_method("flk_dht_get", dht_get::<C>);
        }

        RpcServer(server.finish())
    }
}

pub async fn rpc_discovery_handler<C: Collection>() -> Result<String> {
    match fs::read_to_string(OPEN_RPC_DOCS) {
        Ok(contents) => Ok(contents),
        Err(e) => Err(Error::internal(e)),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct KillParam(AccountOwnerSignature, [u8; 32]);
/// TESTNET ONLY Kill signal for manually stopping nodes.
pub async fn kill<C: Collection>(
    Params(KillParam(signature, digest)): Params<KillParam>,
) -> Result<()> {
    let governance_pk = EthAddress::from_str("0x2a8cf657769c264b0c7f88e3a716afdeaec1c318").unwrap();
    // verify the signature is from the governance key.
    if governance_pk.verify(&signature, &digest) {
        eprintln!("--- RECEIVED GOVERNANCE KILL SIGNAL ---");
        std::process::exit(1);
    }

    Err(jsonrpc_v2::Error::Provided {
        code: 401,
        message: "invalid governance signature",
    })
}

pub async fn put<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<Vec<u8>>,
) -> Result<Blake3Hash> {
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: params,
    };

    data.fetcher.put(pointer).await.map_err(Error::internal)
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
    Params(params): Params<VersionedNodeKeyParam>,
) -> Result<Option<NodeInfo>> {
    if params.version != RPC_VERSION {
        Err(Error::Provided {
            code: 69,
            message: "Version Mismatch",
        })
    } else {
        Ok(data.0.query_runner.get_node_info(&params.public_key))
    }
}

pub async fn get_account_info_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<Option<AccountInfo>> {
    Ok(data.0.query_runner.get_account_info(&params.public_key))
}

pub async fn get_reputation_handler<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<Option<u8>> {
    Ok(data.0.query_runner.get_reputation(&params.public_key))
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
    Params(param): Params<UpdateRequest>,
) -> Result<()> {
    data.0
        .mempool_socket
        .run(param)
        .await
        .map_err(Error::internal)
}

#[cfg(feature = "e2e-test")]
pub async fn dht_put<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(param): Params<DhtPutParam>,
) -> Result<()> {
    let dht_socket = match data.0.dht_socket.lock().unwrap().clone() {
        Some(dht_socket) => dht_socket,
        None => panic!("Dht socket not provided"),
    };

    let res = dht_socket
        .run(DhtRequest::Put {
            prefix: KeyPrefix::ContentRegistry,
            key: param.key,
            value: param.value,
        })
        .await
        .expect("sending put request failed.");
    if let DhtResponse::Put(()) = res {
        Ok(())
    } else {
        Err(Error::INTERNAL_ERROR)
    }
}

#[cfg(feature = "e2e-test")]
pub async fn dht_get<C: Collection>(
    data: Data<Arc<RpcData<C>>>,
    Params(param): Params<DhtGetParam>,
) -> Result<Option<TableEntry>> {
    let dht_socket = match data.0.dht_socket.lock().unwrap().clone() {
        Some(dht_socket) => dht_socket,
        None => panic!("Dht socket not provided"),
    };

    let res = dht_socket
        .run(DhtRequest::Get {
            prefix: KeyPrefix::ContentRegistry,
            key: param.key,
        })
        .await
        .expect("sending get request failed.");
    if let DhtResponse::Get(value) = res {
        Ok(value)
    } else {
        Err(Error::INTERNAL_ERROR)
    }
}
