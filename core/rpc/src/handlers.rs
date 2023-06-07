use std::sync::Arc;

use axum::{Extension, Json};
use draco_interfaces::{
    types::{ExecutionData, TransactionResponse},
    RpcInterface, RpcMethods,
};
use jsonrpc_v2::{Data, Error, MapRouter, Params, RequestObject, ResponseObjects, Server};

pub type Result<T> = anyhow::Result<T, Error>;
use fleek_crypto::AccountOwnerPublicKey;

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
            .with_method("flk_get_balance", get_balance_handler::<I>);

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
    Params(pub_key): Params<AccountOwnerPublicKey>,
) -> Result<u128> {
    match data.0.get_balance(pub_key).await {
        TransactionResponse::Success(ExecutionData::UInt(balance)) => Ok(balance),
        _ => Err(Error::INTERNAL_ERROR),
    }
}
