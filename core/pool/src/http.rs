use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use tokio::sync::Notify;

pub async fn spawn_http_server(addr: SocketAddr, shutdown: Arc<Notify>) -> anyhow::Result<()> {
    let app = Router::new().route("/pool-state", axum::routing::get(state));

    // Todo: set up with_graceful_shutdown.
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown.notified())
        .await
        .context("failed to run http server")
        .map(Into::into)
}

async fn state() -> String {
    "state".to_string()
}
