mod admin;
mod eth;
mod flk;
mod net;

use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub use admin::{AdminApiClient, AdminApiServer};
use anyhow::Context;
pub use eth::{EthApiClient, EthApiServer};
pub use flk::{FleekApiClient, FleekApiOpenRpc, FleekApiServer};
use futures::Stream;
use hyper::header::CONTENT_TYPE;
use hyper::Body;
use jsonrpsee::http_client::transport::HttpBackend;
use jsonrpsee::http_client::HttpClient;
use lightning_utils::rpc::get_timestamp;
pub use net::{NetApiClient, NetApiOpenRpc, NetApiServer};
use tower::util::MapRequest;
use tower::ServiceBuilder;

use crate::server::{LIGHTINING_HMAC_HEADER, LIGHTINING_NONCE_HEADER, LIGHTINING_TIMESTAMP_HEADER};

pub struct HmacClient<F> {
    client: HttpClient<MapRequest<HttpBackend, F>>,
    nonce_client: reqwest::Client,
    address: String,
    nonce: Arc<AtomicU32>,
}

impl<F> std::ops::Deref for HmacClient<F> {
    type Target = HttpClient<MapRequest<HttpBackend, F>>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<F> HmacClient<F> {
    /// Create a new client with HMAC support, fetching the curernt nonce
    fn new(
        client: HttpClient<MapRequest<HttpBackend, F>>,
        nonce_client: reqwest::Client,
        address: String,
        nonce: Arc<AtomicU32>,
    ) -> Self {
        Self {
            client,
            nonce_client,
            address,
            nonce,
        }
    }

    /// Aligns the current nonce with the state of the server
    pub async fn sync_nonce(&mut self) -> anyhow::Result<()> {
        let nonce = get_nonce(&self.nonce_client, &self.address).await?;

        // Relaxed, we have a mut ref to self
        self.nonce.store(nonce, Ordering::Relaxed);
        Ok(())
    }
}

pub fn rpc_client(address: &str) -> anyhow::Result<HttpClient<HttpBackend>> {
    HttpClient::<HttpBackend>::builder()
        .build(address)
        .context(format!("Trying to build rpc client for {address}"))
}

/// Create a new JSON_RPC client with HMAC support, fetching the curernt nonce
///
/// todo(n) could create our own layer and clean up this type sigature
pub async fn admin_hmac_client(
    address: &str,
    key: &[u8; 32],
) -> anyhow::Result<HmacClient<impl FnMut(hyper::Request<Body>) -> hyper::Request<Body> + Clone>> {
    if !address.ends_with("/admin") {
        return Err(anyhow::anyhow!(
            "HMAC is only supported for /admin endpoints"
        ));
    }

    let nonce_client = reqwest::Client::new();
    let nonce = Arc::new(AtomicU32::new(get_nonce(&nonce_client, address).await?));

    Ok(HmacClient::new(
        HttpClient::<HttpBackend>::builder()
            .set_http_middleware(ServiceBuilder::new().map_request({
                let nonce = nonce.clone();
                let key = *key;

                move |req: hyper::Request<hyper::Body>| {
                    let content_type = req.headers().get(CONTENT_TYPE).cloned();

                    // check that the content type is what we expct
                    if let Some(content_type) = content_type {
                        if content_type != "application/json" {
                            return req;
                        }
                    } else {
                        return req;
                    }

                    let (mut parts, mut body) = req.into_parts();

                    // we need to poll the body to get the full body
                    // if the body is in the form that we expect. then this should neven return
                    // pending
                    let dummy_waker = futures::task::noop_waker();
                    let dummy_cx = &mut std::task::Context::from_waker(&dummy_waker);
                    if let std::task::Poll::Ready(bytes) = Pin::new(&mut body).poll_next(dummy_cx) {
                        match bytes {
                            // we got a resposne the first poll and we check the content type
                            // already so we know this is the full body
                            // surely
                            Some(Ok(b)) => {
                                let buf = b.to_vec();

                                let timestamp = get_timestamp();
                                let nonce = nonce.fetch_add(1, Ordering::AcqRel);
                                let hmac = super::create_hmac(&key, &buf, timestamp, nonce);

                                parts
                                    .headers
                                    .insert(LIGHTINING_HMAC_HEADER, hmac.parse().unwrap());
                                parts.headers.insert(
                                    LIGHTINING_TIMESTAMP_HEADER,
                                    timestamp.to_string().parse().unwrap(),
                                );
                                parts.headers.insert(
                                    LIGHTINING_NONCE_HEADER,
                                    nonce.to_string().parse().unwrap(),
                                );

                                return hyper::Request::from_parts(parts, hyper::Body::from(b));
                            },
                            // theres no body, lets replace an empty body so we dont get any
                            // unexpected behavior
                            None => {
                                return hyper::Request::from_parts(parts, hyper::Body::empty());
                            },
                            // no op, its only the first poll, just pass this through
                            Some(Err(_)) => {},
                        }
                    };

                    hyper::Request::from_parts(parts, body)
                }
            }))
            .build(address)?,
        nonce_client,
        address.to_string(),
        nonce,
    ))
}

async fn get_nonce(client: &reqwest::Client, address: &str) -> anyhow::Result<u32> {
    Ok(client
        .get(format!("{}/nonce", address))
        .send()
        .await?
        .text()
        .await?
        .parse()?)
}
