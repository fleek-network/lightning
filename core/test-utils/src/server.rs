use axum::routing::get;
use axum::Router;

pub async fn spawn_server(port: u16) -> anyhow::Result<()> {
    // Mostly taken from:
    // https://github.com/fleek-network/ursa/blob/main/crates/ursa-rpc-service/src/tests/mod.rs
    let file: Vec<u8> = std::fs::read("../test-utils/files/index.ts")?;

    let router = Router::new().route("/bar/:filename", get(|| async move { file }));

    axum::Server::bind(&format!("127.0.0.1:{port}").parse().unwrap())
        .serve(router.into_make_service())
        .await
        .map_err(|e| e.into())
}
