use std::net::SocketAddr;

use anyhow::Context;
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Response};
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use fleek_crypto::NodePublicKey;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::config::HttpsConfig;
use crate::shutdown::ShutdownWaiter;

pub const FLEEK_NODE_HEADER: &str = "x-fleek-node";

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

pub async fn spawn_https_server(
    router: Router,
    https_config: HttpsConfig,
    handle: Handle,
) -> anyhow::Result<()> {
    let app = router
        .layer(CorsLayer::permissive())
        .into_make_service_with_connect_info::<SocketAddr>();

    let config =
        RustlsConfig::from_pem_file(https_config.cert.as_path(), https_config.key.as_path())
            .await?;
    axum_server::bind_rustls(https_config.address, config)
        .handle(handle)
        .serve(app)
        .await
        .context("failed to run http server")
}

pub fn fleek_node_response_header(
    pk: NodePublicKey,
) -> SetResponseHeaderLayer<impl FnMut(&Response<Body>) -> Option<HeaderValue> + Clone> {
    let pk = pk.to_string();
    SetResponseHeaderLayer::overriding(HeaderName::from_static(FLEEK_NODE_HEADER), move |_: &_| {
        Some(HeaderValue::from_str(&pk).expect("Base58 alphabet contains only valid characters"))
    })
}
