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

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Now Axum requires future in `with_graceful_shutdown` to be static.
    let shutdown = async move {
        waiter.wait_for_shutdown().await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .context("failed to run http server")
}
