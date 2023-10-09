use std::net::SocketAddr;

use anyhow::Context;
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::shutdown::ShutdownWaiter;

pub async fn spawn_http_server(
    addr: SocketAddr,
    router: Router,
    waiter: ShutdownWaiter,
) -> anyhow::Result<()> {
    let app = router
        .layer(CorsLayer::permissive())
        .into_make_service_with_connect_info::<SocketAddr>();

    axum::Server::bind(&addr)
        .serve(app)
        .with_graceful_shutdown(waiter.wait_for_shutdown())
        .await
        .context("failed to run http server")
}
