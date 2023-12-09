use std::net::SocketAddr;

use anyhow::Context;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use lightning_interfaces::types::NodeIndex;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::state::{Query, QuerySender};

pub async fn spawn_http_server(
    addr: SocketAddr,
    shutdown: CancellationToken,
    state_request: QuerySender,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/state", axum::routing::get(state))
        .route("/peer/:index", axum::routing::get(peer))
        .layer(Extension(state_request));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown.cancelled())
        .await
        .context("failed to run http server")
        .map(Into::into)
}

async fn state(Extension(state_request): Extension<QuerySender>) -> Result<Response, StatusCode> {
    let (respond_tx, respond_rx) = oneshot::channel();
    if state_request
        .send(Query::State {
            respond: respond_tx,
        })
        .await
        .is_ok()
    {
        let state = respond_rx
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let value = serde_json::to_string(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(value.into_response())
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

async fn peer(
    Path(index): Path<NodeIndex>,
    Extension(state_request): Extension<QuerySender>,
) -> Result<Response, StatusCode> {
    let (respond_tx, respond_rx) = oneshot::channel();
    if state_request
        .send(Query::Peer {
            index,
            respond: respond_tx,
        })
        .await
        .is_ok()
    {
        let state = respond_rx
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let value = serde_json::to_string(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(value.into_response())
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
