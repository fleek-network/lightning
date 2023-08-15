use std::{fs, sync::Arc};

use axum::{Extension, Json};
use fleek_crypto::{EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use jsonrpc_v2::{Data, Error, MapRouter, Params, RequestObject, ResponseObjects, Server};
#[cfg(feature = "e2e-test")]
use lightning_interfaces::types::{DhtRequest, DhtResponse, KeyPrefix, TableEntry};
use lightning_interfaces::{
    types::{EpochInfo, NodeInfo, NodeServed, ProtocolParams, TotalServed, UpdateRequest},
    SyncQueryRunnerInterface,
};

#[cfg(feature = "e2e-test")]
use crate::types::{DhtGetParam, DhtPutParam};
use crate::{
    server::RpcData,
    types::{NodeKeyParam, PublicKeyParam},
};
static OPEN_RPC_DOCS: &str = "../../docs/rpc/openrpc.json";

pub type Result<T> = anyhow::Result<T, Error>;

pub async fn rpc_handler(
    Extension(rpc): Extension<RpcServer>,
    Json(req): Json<RequestObject>,
) -> Json<ResponseObjects> {
    let res = rpc.0.handle(req).await;
    Json(res)
}

#[derive(Clone)]
pub struct RpcServer(Arc<Server<MapRouter>>);

impl RpcServer {
    pub fn new<Q>(interface: Arc<RpcData<Q>>) -> Self
    where
        Q: SyncQueryRunnerInterface + 'static,
    {
        #[allow(unused_mut)]
        let mut server = Server::new()
            .with_data(Data::new(interface))
            .with_method("rpc.discover", discovery_handler::<Q>)
            .with_method("flk_ping", ping_handler::<Q>)
            .with_method("flk_get_flk_balance", get_flk_balance_handler::<Q>)
            .with_method(
                "flk_get_bandwidth_balance",
                get_bandwidth_balance_handler::<Q>,
            )
            .with_method("flk_get_locked", get_locked_handler::<Q>)
            .with_method("flk_get_staked", get_staked_handler::<Q>)
            .with_method("flk_get_stables_balance", get_stables_balance_handler::<Q>)
            .with_method(
                "flk_get_stake_locked_until",
                get_stake_locked_until_handler::<Q>,
            )
            .with_method("flk_get_locked_time", get_locked_time_handler::<Q>)
            .with_method("flk_get_node_info", get_node_info_handler::<Q>)
            .with_method("flk_get_staking_amount", get_staking_amount_handler::<Q>)
            .with_method(
                "flk_get_committee_members",
                get_committee_members_handler::<Q>,
            )
            .with_method("flk_get_epoch", get_epoch_handler::<Q>)
            .with_method("flk_get_epoch_info", get_epoch_info_handler::<Q>)
            .with_method("flk_get_total_supply", get_total_supply_handler::<Q>)
            .with_method(
                "flk_get_year_start_supply",
                get_year_start_supply_handler::<Q>,
            )
            .with_method(
                "flk_get_protocol_fund_address",
                get_protocol_fund_address_handler::<Q>,
            )
            .with_method("flk_get_protocol_params", get_protocol_params_handler::<Q>)
            .with_method("flk_get_total_served", get_total_served_handler::<Q>)
            .with_method("flk_get_node_served", get_node_served_handler::<Q>)
            .with_method("flk_is_valid_node", is_valid_node_handler::<Q>)
            .with_method("flk_get_node_registry", get_node_registry_handler::<Q>)
            .with_method("flk_get_reputation", get_reputation_handler::<Q>)
            .with_method("flk_send_txn", send_txn::<Q>);

        #[cfg(feature = "e2e-test")]
        {
            server = server.with_method("flk_dht_put", dht_put::<Q>);
            server = server.with_method("flk_dht_get", dht_get::<Q>);
        }

        RpcServer(server.finish())
    }
}

pub async fn discovery_handler<Q: SyncQueryRunnerInterface>() -> Result<String> {
    match fs::read_to_string(OPEN_RPC_DOCS) {
        Ok(contents) => Ok(contents),
        Err(e) => Err(Error::internal(e)),
    }
}
pub async fn ping_handler<Q: SyncQueryRunnerInterface>() -> Result<String> {
    Ok("pong".to_string())
}

pub async fn get_flk_balance_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_flk_balance(&params.public_key))
}

pub async fn get_stables_balance_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfixed<6>> {
    Ok(data.0.query_runner.get_stables_balance(&params.public_key))
}

pub async fn get_bandwidth_balance_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<u128> {
    Ok(data.0.query_runner.get_account_balance(&params.public_key))
}

pub async fn get_staked_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_staked(&params.public_key))
}

pub async fn get_locked_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_locked(&params.public_key))
}

pub async fn get_stake_locked_until_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u64> {
    Ok(data
        .0
        .query_runner
        .get_stake_locked_until(&params.public_key))
}

pub async fn get_locked_time_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u64> {
    Ok(data.0.query_runner.get_locked_time(&params.public_key))
}

pub async fn get_node_info_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<Option<NodeInfo>> {
    Ok(data.0.query_runner.get_node_info(&params.public_key))
}

pub async fn get_reputation_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<Option<u8>> {
    Ok(data.0.query_runner.get_reputation(&params.public_key))
}

pub async fn get_staking_amount_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<u128> {
    Ok(data.0.query_runner.get_staking_amount())
}

pub async fn get_committee_members_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<Vec<NodePublicKey>> {
    Ok(data.0.query_runner.get_committee_members())
}

pub async fn get_epoch_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<u64> {
    Ok(data.0.query_runner.get_epoch())
}

pub async fn get_epoch_info_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<EpochInfo> {
    Ok(data.0.query_runner.get_epoch_info())
}

pub async fn get_total_supply_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_total_supply())
}

pub async fn get_year_start_supply_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<HpUfixed<18>> {
    Ok(data.0.query_runner.get_year_start_supply())
}

pub async fn get_protocol_fund_address_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<EthAddress> {
    Ok(data.0.query_runner.get_protocol_fund_address())
}

pub async fn get_protocol_params_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<ProtocolParams>,
) -> Result<u128> {
    Ok(data.0.query_runner.get_protocol_params(params))
}

pub async fn get_total_served_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<u64>,
) -> Result<TotalServed> {
    Ok(data.0.query_runner.get_total_served(params))
}

pub async fn get_node_served_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<NodeServed> {
    Ok(data.0.query_runner.get_node_served(&params.public_key))
}

pub async fn is_valid_node_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<bool> {
    Ok(data.0.query_runner.is_valid_node(&params.public_key))
}

pub async fn get_node_registry_handler<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
) -> Result<Vec<NodeInfo>> {
    Ok(data.0.query_runner.get_node_registry())
}

pub async fn send_txn<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
    Params(param): Params<UpdateRequest>,
) -> Result<()> {
    data.0
        .mempool_socket
        .run(param)
        .await
        .map_err(Error::internal)

    // println!("{:?}", param);
    //Ok(())
}

#[cfg(feature = "e2e-test")]
pub async fn dht_put<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
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
pub async fn dht_get<Q: SyncQueryRunnerInterface>(
    data: Data<Arc<RpcData<Q>>>,
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
