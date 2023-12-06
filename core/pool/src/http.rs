use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use tokio::sync::mpsc::Sender;
use tokio::sync::{oneshot, Notify};

use crate::state::{State, StateRequestSender};

pub async fn spawn_http_server(
    addr: SocketAddr,
    shutdown: Arc<Notify>,
    state_request: StateRequestSender,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/pool-state", axum::routing::get(state))
        .layer(Extension(state_request));

    // Todo: set up with_graceful_shutdown.
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown.notified())
        .await
        .context("failed to run http server")
        .map(Into::into)
}

async fn state(
    Extension(state_request): Extension<Sender<oneshot::Sender<State>>>,
) -> Result<Response, StatusCode> {
    let (respond_tx, respond_rx) = oneshot::channel();
    if state_request.send(respond_tx).await.is_ok() {
        let state = respond_rx
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let value = serde_json::to_string(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(value.into_response())
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
