use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::Router;

pub async fn spawn_server(port: u16) -> anyhow::Result<()> {
    // Mostly taken from:
    // https://github.com/fleek-network/ursa/blob/main/crates/ursa-rpc-service/src/tests/mod.rs
    let ts_file: Vec<u8> = std::fs::read("../test-utils/files/index.ts")?;

    let router = Router::new()
        .route("/ipfs/:cid", get(get_cid))
        .route("/bar/:filename", get(|| async move { ts_file.clone() }));

    axum::Server::bind(&format!("0.0.0.0:{port}").parse().unwrap())
        .serve(router.into_make_service())
        .await
        .map_err(|e| e.into())
}

async fn get_cid(Path(cid): Path<String>) -> Result<(HeaderMap, Vec<u8>), StatusCode> {
    if let Ok(file) = std::fs::read(format!("../test-utils/files/{cid}.car")) {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/vnd.ipld.car".parse().unwrap());
        Ok((headers, file.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
