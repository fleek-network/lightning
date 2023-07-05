use std::sync::Arc;

use axum::{Extension, Json};
use draco_interfaces::{RpcInterface, SyncQueryRunnerInterface};
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
            .with_method("flk_get_balance", get_balance_handler::<Q, I>)
            .with_method(
                "flk_get_bandwidth_balance",
                get_bandwidth_balance_handler::<Q, I>,
            )
            .with_method("flk_get_locked", get_locked_handler::<Q, I>);

        RpcServer(server.finish())
    }
}

pub async fn ping_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>() -> Result<String> {
    Ok("pong".to_string())
}

pub async fn get_balance_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<HpUfloat<18>> {
    Ok(data.0.query_runner().get_flk_balance(&params.public_key.into()))
}

pub async fn get_bandwidth_balance_handler<Q: SyncQueryRunnerInterface, I: RpcInterface<Q>>(
    data: Data<Arc<I>>,
    Params(params): Params<PublicKeyParam>,
) -> Result<u128> {
    Ok(data
        .0
        .query_runner()
        .get_account_balance(&params.public_key.into()))
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
    Ok(data.0.query_runner().get_locked(&params.public_key))
}
