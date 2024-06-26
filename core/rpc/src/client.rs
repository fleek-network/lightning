use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::Context;
use futures::Stream;
use hyper::header::CONTENT_TYPE;
use hyper::Body;
use jsonrpsee::core::client::{BatchResponse, ClientT, Subscription, SubscriptionClientT};
use jsonrpsee::core::params::BatchRequestBuilder;
use jsonrpsee::core::traits::ToRpcParams;
use jsonrpsee::http_client::transport::HttpBackend;
use jsonrpsee::http_client::HttpClient;
use lightning_utils::rpc::get_timestamp;
use serde::de::DeserializeOwned;
use tower::ServiceBuilder;

use crate::server::{LIGHTINING_HMAC_HEADER, LIGHTINING_NONCE_HEADER, LIGHTINING_TIMESTAMP_HEADER};

pub fn make_plain_rpc_client(address: &str) -> anyhow::Result<HttpClient<HttpBackend>> {
    HttpClient::<HttpBackend>::builder()
        .build(address)
        .context(format!("Trying to build rpc client for {address}"))
}

pub enum RpcClient {
    WithHmac(HmacClient),
    Http(HttpClient<HttpBackend>),
}

impl RpcClient {
    pub async fn new(address: &str, key: Option<&[u8; 32]>) -> anyhow::Result<Self> {
        match key {
            Some(key) => {
                if !address.ends_with("/admin") {
                    return Err(anyhow::anyhow!(
                        "HMAC is only supported for /admin endpoints"
                    ));
                }

                Ok(Self::WithHmac(HmacClient::new(address, key).await?))
            },
            None => Ok(Self::Http(make_plain_rpc_client(address)?)),
        }
    }

    pub fn new_no_auth(address: &str) -> anyhow::Result<Self> {
        Ok(Self::Http(make_plain_rpc_client(address)?))
    }
}

pub struct HmacClient {
    client: HttpClient<HmacMiddleware<HttpBackend>>,
    nonce_client: reqwest::Client,
    address: String,
    nonce: Arc<AtomicU32>,
}

impl HmacClient {
    /// Create a new client with HMAC support, fetching the curernt nonce
    pub async fn new(address: &str, key: &[u8; 32]) -> anyhow::Result<Self> {
        if !address.ends_with("/admin") {
            return Err(anyhow::anyhow!(
                "HMAC is only supported for /admin endpoints"
            ));
        }

        // fetch the latest nonce
        let nonce_client = reqwest::Client::new();
        let nonce = Arc::new(AtomicU32::new(get_nonce(&nonce_client, address).await?));
        let middleware = HmacMiddlewareLayer::new(nonce.clone(), *key);

        // create the backend
        let backend = HttpClient::<HttpBackend>::builder()
            .set_http_middleware(ServiceBuilder::new().layer(middleware))
            .build(address)?;

        Ok(Self {
            client: backend,
            nonce_client,
            address: address.to_string(),
            nonce,
        })
    }

    /// Aligns the current nonce with the state of the server
    pub async fn sync_nonce(&mut self) -> anyhow::Result<()> {
        let nonce = get_nonce(&self.nonce_client, &self.address).await?;

        // Relaxed, we have a mut ref to self
        self.nonce.store(nonce, Ordering::Relaxed);
        Ok(())
    }
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

pub struct HmacMiddlewareLayer {
    nonce: Arc<AtomicU32>,
    key: [u8; 32],
}

impl HmacMiddlewareLayer {
    pub fn new(nonce: Arc<AtomicU32>, key: [u8; 32]) -> Self {
        Self { nonce, key }
    }
}

impl<S: Clone> tower::Layer<S> for HmacMiddlewareLayer {
    type Service = HmacMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HmacMiddleware {
            nonce: self.nonce.clone(),
            key: Arc::new(self.key),
            inner,
        }
    }
}

#[derive(Clone)]
pub struct HmacMiddleware<S: Clone> {
    nonce: Arc<AtomicU32>,
    key: Arc<[u8; 32]>,
    inner: S,
}

impl<S: Clone> HmacMiddleware<S> {
    fn add_hmac_headers(&self, req: hyper::Request<hyper::Body>) -> hyper::Request<hyper::Body> {
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

        // we need to poll to get the full body, if the body is in the normal form of a one time
        // blob of bytes then this should neven return pending.
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
                    let nonce = self.nonce.fetch_add(1, Ordering::AcqRel);
                    let hmac = super::create_hmac(&self.key, &buf, timestamp, nonce);

                    parts
                        .headers
                        .insert(LIGHTINING_HMAC_HEADER, hmac.parse().unwrap());
                    parts.headers.insert(
                        LIGHTINING_TIMESTAMP_HEADER,
                        timestamp.to_string().parse().unwrap(),
                    );
                    parts
                        .headers
                        .insert(LIGHTINING_NONCE_HEADER, nonce.to_string().parse().unwrap());

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

        #[cfg(debug_assertions)]
        {
            let dummy_waker = futures::task::noop_waker();
            let dummy_cx = &mut std::task::Context::from_waker(&dummy_waker);

            match Pin::new(&mut body).poll_next(dummy_cx) {
                std::task::Poll::Ready(Some(_)) => {
                    tracing::warn!(target = "debug_check", "bytes returned another some value");
                },
                std::task::Poll::Pending => {
                    tracing::warn!(target = "debug_check", "body is pending");
                },
                std::task::Poll::Ready(None) => {},
            }
        }

        hyper::Request::from_parts(parts, body)
    }
}

impl<S, Res> tower::Service<hyper::Request<Body>> for HmacMiddleware<S>
where
    S: tower::Service<hyper::Request<Body>, Response = Res>,
    S: Clone,
{
    type Error = S::Error;
    type Response = Res;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: hyper::Request<Body>) -> Self::Future {
        self.inner.call(self.add_hmac_headers(req))
    }
}

#[async_trait::async_trait]
impl ClientT for RpcClient
where
    HttpClient<HmacMiddleware<HttpBackend>>: ClientT,
{
    async fn notification<Params>(
        &self,
        method: &str,
        params: Params,
    ) -> Result<(), jsonrpsee::core::client::Error>
    where
        Params: ToRpcParams + Send,
    {
        match self {
            Self::Http(client) => client.notification(method, params).await,
            Self::WithHmac(client) => client.notification(method, params).await,
        }
    }

    async fn request<R, Params>(
        &self,
        method: &str,
        params: Params,
    ) -> Result<R, jsonrpsee::core::client::Error>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        match self {
            Self::Http(client) => client.request(method, params).await,
            Self::WithHmac(client) => client.request(method, params).await,
        }
    }

    async fn batch_request<'a, R>(
        &self,
        batch: BatchRequestBuilder<'a>,
    ) -> Result<BatchResponse<'a, R>, jsonrpsee::core::client::Error>
    where
        R: DeserializeOwned + std::fmt::Debug + 'a,
    {
        match self {
            Self::Http(client) => client.batch_request(batch).await,
            Self::WithHmac(client) => client.batch_request(batch).await,
        }
    }
}

/// Implementations needed to make it friendly with the jsonrpsee trait bounds
#[async_trait::async_trait]
impl ClientT for HmacClient
where
    HttpClient<HmacMiddleware<HttpBackend>>: ClientT,
{
    async fn notification<Params>(
        &self,
        method: &str,
        params: Params,
    ) -> Result<(), jsonrpsee::core::client::Error>
    where
        Params: ToRpcParams + Send,
    {
        self.client.notification(method, params).await
    }

    async fn request<R, Params>(
        &self,
        method: &str,
        params: Params,
    ) -> Result<R, jsonrpsee::core::client::Error>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        self.client.request(method, params).await
    }

    async fn batch_request<'a, R>(
        &self,
        batch: BatchRequestBuilder<'a>,
    ) -> Result<BatchResponse<'a, R>, jsonrpsee::core::client::Error>
    where
        R: DeserializeOwned + std::fmt::Debug + 'a,
    {
        self.client.batch_request(batch).await
    }
}

#[async_trait::async_trait]
impl SubscriptionClientT for HmacClient {
    async fn subscribe<'a, Notif, Params>(
        &self,
        subscribe_method: &'a str,
        params: Params,
        unsubscribe_method: &'a str,
    ) -> Result<Subscription<Notif>, jsonrpsee::core::client::Error>
    where
        Params: ToRpcParams + Send,
        Notif: DeserializeOwned,
    {
        self.client
            .subscribe(subscribe_method, params, unsubscribe_method)
            .await
    }

    async fn subscribe_to_method<'a, Notif>(
        &self,
        method: &'a str,
    ) -> Result<Subscription<Notif>, jsonrpsee::core::client::Error>
    where
        Notif: DeserializeOwned,
    {
        self.client.subscribe_to_method(method).await
    }
}

#[async_trait::async_trait]
impl SubscriptionClientT for RpcClient {
    async fn subscribe<'a, Notif, Params>(
        &self,
        subscribe_method: &'a str,
        params: Params,
        unsubscribe_method: &'a str,
    ) -> Result<Subscription<Notif>, jsonrpsee::core::client::Error>
    where
        Params: ToRpcParams + Send,
        Notif: DeserializeOwned,
    {
        match self {
            Self::Http(client) => {
                client
                    .subscribe(subscribe_method, params, unsubscribe_method)
                    .await
            },
            Self::WithHmac(client) => {
                client
                    .subscribe(subscribe_method, params, unsubscribe_method)
                    .await
            },
        }
    }

    async fn subscribe_to_method<'a, Notif>(
        &self,
        method: &'a str,
    ) -> Result<Subscription<Notif>, jsonrpsee::core::client::Error>
    where
        Notif: DeserializeOwned,
    {
        match self {
            Self::Http(client) => client.subscribe_to_method(method).await,
            Self::WithHmac(client) => client.subscribe_to_method(method).await,
        }
    }
}
