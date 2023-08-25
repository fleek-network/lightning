mod ip_api;
mod types;

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;

use anyhow::{anyhow, Result};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{self, Extension, Json, Router};
use hyper::{Body, Client, Method, Request};
use hyper_tls::HttpsConnector;
use lazy_static::lazy_static;
use lightning_types::NodeInfo;
use log::{error, info};
use serde_json::{json, Value};

use crate::ip_api::get_ip_info;
use crate::types::PrometheusDiscoveryChunk;

lazy_static! {
    static ref NODE_REGISTRY_REQUEST: serde_json::Value = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_registry",
        "params":[],
        "id":1,
    });
}

#[tokio::main]
async fn main() {
    let token = env::var("IPINFO_TOKEN").expect("IPINFO_TOKEN is not set");

    let app = Router::new()
        .route("/http_sd", get(service_discovery))
        .layer(Extension(token));

    let addr = SocketAddr::from(([0, 0, 0, 0], 4000));
    info!("metrics service discovery listening on {addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

/// handler for HTTP-based service discovery for prometheus
async fn service_discovery(Extension(token): Extension<String>) -> (StatusCode, Json<Value>) {
    let nodes = match get_node_registry().await {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(e.to_string())),
            );
        },
    };
    let mut discovery_chunk = Vec::new();
    for node in nodes {
        // todo(Arslan): store data from api and avoid making multiple request to api
        if let Ok(ip_info) = get_ip_info(&token, node.domain.to_string()).await {
            let targets = vec![format!(
                "{}:{}",
                node.domain.to_string(),
                node.ports.rpc.to_string()
            )];
            let mut labels = HashMap::new();
            labels.insert("geohash".to_string(), ip_info.geo.clone());
            labels.insert("country_code".to_string(), ip_info.country.clone());
            labels.insert("timezone".to_string(), ip_info.timezone.clone());
            discovery_chunk.push(PrometheusDiscoveryChunk::new(targets, labels));
        } else {
            error!("Failed to lookup ip info for {}", node.domain);
            continue;
        }
    }

    (StatusCode::OK, Json(json!("")))
}

async fn get_node_registry() -> Result<Vec<NodeInfo>> {
    let client = Client::builder().build::<_, hyper::Body>(HttpsConnector::new());

    let address = env::var("ADDRESS").map_err(|_| anyhow!("ADDRESS is not set"))?;
    let port = env::var("RPC_PORT").map_err(|_| anyhow!("RPC_PORT is not set"))?;

    let uri = format!("https://{address}:{port}/rpc/v0");
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .body(Body::from(NODE_REGISTRY_REQUEST.to_string()))
        .map_err(|_| anyhow!("Failed to build the rpc request"))?;

    let response = client
        .request(req)
        .await
        .map_err(|_| anyhow!("Request to rpc service failed"))?;

    if !response.status().is_success() {
        return Err(anyhow!(format!(
            "Failed to get nodes registry with status: {}",
            response.status()
        )));
    }

    let data = hyper::body::to_bytes(response.into_body()).await?;
    Ok(serde_json::from_slice(&data)?)
}
