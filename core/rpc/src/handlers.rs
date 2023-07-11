use std::sync::Arc;

use axum::{Extension, Json};
use draco_interfaces::{
    types::{EpochInfo, NodeInfo, ProtocolParams},
    RpcInterface, SyncQueryRunnerInterface,
};
use fleek_crypto::{EthAddress, NodePublicKey};
use hp_float::unsigned::HpUfloat;
use jsonrpc_v2::{Data, Error, MapRouter, Params, RequestObject, ResponseObjects, Server};

use crate::types::{NodeKeyParam, PublicKeyParam};

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
    pub fn new<I, Q>(interface: Arc<I>) -> Self
    where
        Q: SyncQueryRunnerInterface + 'static,
        I: RpcInterface<Q> + 'static,
    {
        let server = Server::new()
            .with_data(Data::new(interface))
            .with_method("flk_ping", ping_handler::<Q, I>)
            .with_method("flk_get_flk_balance", get_flk_balance_handler::<Q, I>)
            .with_method(
                "flk_get_bandwidth_balance",
                get_bandwidth_balance_handler::<Q, I>,
            )
            .with_method("flk_get_locked", get_locked_handler::<Q, I>)
            .with_method("flk_get_staked", get_staked_handler::<Q, I>)
            .with_method(
                "flk_get_stables_balance",
                get_stables_balance_handler::<Q, I>,
            )
            .with_method(
                "flk_get_stake_locked_until",
                get_stake_locked_until_handler::<Q, I>,
            )
            .with_method("flk_get_locked_time", get_locked_time_handler::<Q, I>)
            .with_method("flk_get_node_info", get_node_info_handler::<Q, I>)
            .with_method("flk_get_staking_amount", get_staking_amount_handler::<Q, I>)
            .with_method(
                "flk_get_committee_members",
                get_committee_members_handler::<Q, I>,
            )
            .with_method("flk_get_epoch", get_epoch_handler::<Q, I>)
            .with_method("flk_get_epoch_info", get_epoch_info_handler::<Q, I>)
            .with_method("flk_get_total_supply", get_total_supply_handler::<Q, I>)
            .with_method(
                "flk_get_year_start_supply",
                get_year_start_supply_handler::<Q, I>,
            )
            .with_method(
                "flk_get_protocol_fund_address",
                get_protocol_fund_address_handler::<Q, I>,
            )
            .with_method(
                "flk_get_protocol_params",
                get_protocol_params_handler::<Q, I>,
            )
            .with_method("flk_get_reputation", get_reputation_handler::<Q, I>);

        RpcServer(server.finish())
    }
}

pub async fn ping_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>() -> Result<String> {
    Ok("pong".to_string())
}

pub async fn get_flk_balance_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfloat<18>> {
    Ok(data.0.query_runner().get_flk_balance(&params.public_key))
}

pub async fn get_bandwidth_balance_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<u128> {
    Ok(data
        .0
        .query_runner()
        .get_account_balance(&params.public_key))
}

pub async fn get_locked_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<HpUfloat<18>> {
    Ok(data.0.query_runner().get_locked(&params.public_key))
}

pub async fn get_staked_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<HpUfloat<18>> {
    Ok(data.0.query_runner().get_staked(&params.public_key))
}

pub async fn get_reputation_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<Option<u8>> {
    Ok(data.0.query_runner().get_reputation(&params.public_key))
}

pub async fn get_stables_balance_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfloat<6>> {
    Ok(data
        .0
        .query_runner()
        .get_stables_balance(&params.public_key))
}

pub async fn get_stake_locked_until_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u64> {
    Ok(data
        .0
        .query_runner()
        .get_stake_locked_until(&params.public_key))
}

pub async fn get_locked_time_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u64> {
    Ok(data.0.query_runner().get_locked_time(&params.public_key))
}

pub async fn get_node_info_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<Option<NodeInfo>> {
    Ok(data.0.query_runner().get_node_info(&params.public_key))
}

pub async fn get_staking_amount_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<u128> {
    Ok(data.0.query_runner().get_staking_amount())
}

pub async fn get_committee_members_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<Vec<NodePublicKey>> {
    Ok(data.0.query_runner().get_committee_members())
}

pub async fn get_epoch_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<u64> {
    Ok(data.0.query_runner().get_epoch())
}

pub async fn get_epoch_info_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<EpochInfo> {
    Ok(data.0.query_runner().get_epoch_info())
}

pub async fn get_total_supply_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<HpUfloat<18>> {
    Ok(data.0.query_runner().get_total_supply())
}

pub async fn get_year_start_supply_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<HpUfloat<18>> {
    Ok(data.0.query_runner().get_year_start_supply())
}

pub async fn get_protocol_fund_address_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
) -> Result<EthAddress> {
    Ok(data.0.query_runner().get_protocol_fund_address())
}

pub async fn get_protocol_params_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<ProtocolParams>,
) -> Result<u128> {
    Ok(data.0.query_runner().get_protocol_params(params))
}
