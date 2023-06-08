use std::sync::Arc;

use axum::{Extension, Json};
use draco_interfaces::{
    types::{ExecutionData, TransactionResponse},
    RpcInterface, RpcMethods,
};
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
    pub fn new<I>(interface: Arc<I>) -> Self
    where
        I: RpcMethods + RpcInterface,
    {
        let server = Server::new()
            .with_data(Data::new(interface))
            .with_method("flk_ping", ping_handler::<I>)
            .with_method("flk_get_balance", get_balance_handler::<I>)
            .with_method(
                "flk_get_bandwidth_balance",
                get_bandwidth_balance_handler::<I>,
            )
            .with_method("flk_get_locked", get_locked_handler::<I>);

        RpcServer(server.finish())
    }
}

pub async fn ping_handler<I: RpcMethods>(data: Data<Arc<I>>) -> Result<String> {
    match data.0.ping().await {
        Ok(pong) => Ok(pong),
        Err(e) => Err(Error::internal(e.to_string())),
    }
}

pub async fn get_balance_handler<I: RpcMethods>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<u128> {
    match data.0.get_balance(params.public_key).await {
        TransactionResponse::Success(ExecutionData::UInt(balance)) => Ok(balance),
        _ => Err(Error::INTERNAL_ERROR),
    }
}

pub async fn get_bandwidth_balance_handler<I: RpcMethods>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<u128> {
    match data.0.get_bandwidth(params.public_key).await {
        TransactionResponse::Success(ExecutionData::UInt(balance)) => Ok(balance),
        _ => Err(Error::INTERNAL_ERROR),
    }
}

pub async fn get_locked_handler<I: RpcMethods>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u128> {
    match data.0.get_locked(params.node_key).await {
        TransactionResponse::Success(ExecutionData::UInt(balance)) => Ok(balance),
        _ => Err(Error::INTERNAL_ERROR),
    }
}

pub async fn get_staked_handler<I: RpcMethods>(
    data: Data<Arc<I>>,
    Params(params): Params<NodeKeyParam>,
) -> Result<u128> {
    match data.0.get_staked(params.node_key).await {
        TransactionResponse::Success(ExecutionData::UInt(balance)) => Ok(balance),
        _ => Err(Error::INTERNAL_ERROR),
    }
}
