use std::net::SocketAddr;

use axum::Router;
use tower_http::cors::CorsLayer;

use crate::shutdown::ShutdownWaiter;

pub async fn spawn_http_server(
    addr: SocketAddr,
    router: Router,
    waiter: ShutdownWaiter,
) -> anyhow::Result<()> {
    axum::Server::bind(&addr)
        .serve(router.layer(CorsLayer::permissive()).into_make_service())
        .with_graceful_shutdown(waiter.wait_for_shutdown())
        .await
        .map_err(|e| e.into())
}
