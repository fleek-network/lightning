use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::{FutureExt, TryFutureExt};
use hmac::{Hmac, Mac};
use hyper::{Body, Request, Response};
use sha2::Sha256;
use tower::Service as TowerService;

use crate::{health, metrics, HMAC_SALT, VERSION};

type BoxedError = Box<dyn std::error::Error + Sync + Send + 'static>;

/// A struct that implements the Service trait for the RPC server.
pub struct RpcService<MainModule, AdminModule> {
    pub main_server: MainModule,
    pub admin_server: AdminModule,
    secret: Arc<[u8; 32]>,
    nonce: Arc<AtomicU32>,
}

impl<X, Y> Clone for RpcService<X, Y>
where
    X: Clone,
    Y: Clone,
{
    fn clone(&self) -> Self {
        Self {
            main_server: self.main_server.clone(),
            admin_server: self.admin_server.clone(),
            secret: self.secret.clone(),
            nonce: self.nonce.clone(),
        }
    }
}

pub trait ServiceMarker
where
    Self: Send + 'static,
    Self: TowerService<
            Request<Body>,
            Response = Response<Body>,
            Error = BoxedError,
            Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>,
        > + Send
        + Clone
        + Unpin,
{
}

impl<X> ServiceMarker for X
where
    X: Send + 'static,
    X: TowerService<
            Request<Body>,
            Response = Response<Body>,
            Error = BoxedError,
            Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>,
        > + Send
        + Clone
        + Unpin,
{
}

impl<MainModule, AdminModule> RpcService<MainModule, AdminModule> {
    pub fn new(main_server: MainModule, admin_server: AdminModule, secret: [u8; 32]) -> Self {
        Self {
            main_server,
            admin_server,
            secret: Arc::new(secret),
            nonce: Arc::new(0.into()),
        }
    }
}

///////////////// Router Service ///////////////////////

/// Use concrete types for alot of stuff here so we can pull the data out mid request
impl<MainModule, AdminModule> RpcService<MainModule, AdminModule> {
    pub fn route(
        &mut self,
        req: Request<Body>,
    ) -> Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>
    where
        Self: Send + 'static,
        MainModule: ServiceMarker,
        AdminModule: ServiceMarker,
        <MainModule as TowerService<Request<Body>>>::Future: Send,
        <AdminModule as TowerService<Request<Body>>>::Future: Send,
    {
        let path = req.uri().path().to_string().to_ascii_lowercase();
        let method = req.method();

        // todo(n)
        // in the future the struct could be change to suppport a "main" and a auxillary services
        // and you define new routes using a builder
        //
        //
        // ```
        // fn with_route<S>(self, path: &str, service: S) -> Self
        // where
        //     S: TowerService<Request<Body>, Respose = Response<Body>>,
        // {
        //     // box and add to vec
        // }
        // ```
        match path.as_str() {
            "/health" => {
                let fut = async {
                    let res = health().await;

                    hyper::Response::builder()
                        .status(hyper::StatusCode::OK)
                        .body(hyper::Body::from(res))
                        .map_err(|e| e.into()) // box
                };

                Box::pin(fut)
            },
            "/version" => {
                let fut = async {
                    hyper::Response::builder()
                        .status(hyper::StatusCode::OK)
                        .body(hyper::Body::from(VERSION.clone()))
                        .map_err(|e| e.into()) // box
                };

                Box::pin(fut)
            },
            "/metrics" => {
                let fut = async {
                    let (status, res) = metrics().await;

                    hyper::Response::builder()
                        .status(status)
                        .body(hyper::Body::from(res))
                        .map_err(|e| e.into()) // box
                };

                Box::pin(fut)
            },
            "/admin/nonce" => {
                let nonce = self.nonce.load(std::sync::atomic::Ordering::Acquire);

                Box::pin(async move {
                    hyper::Response::builder()
                        .status(hyper::StatusCode::OK)
                        .body(hyper::Body::from(nonce.to_string()))
                        .map_err(|e| e.into())
                })
            },
            "/admin" => {
                // todo this will be cleaner when we have a better router setup
                let mut svc_clone = self.admin_server.clone();

                if method == hyper::Method::POST {
                    match (
                        req.headers().get("X-Lightning-HMAC"),
                        req.headers().get("X-Lightning-Timestamp"),
                        req.headers().get("X-Lightning-Nonce"),
                    ) {
                        (Some(hmac), Some(ts), Some(nonce)) => {
                            match (hmac.to_str(), ts.to_str(), nonce.to_str()) {
                                (Ok(hmac), Ok(ts), Ok(nonce)) => {
                                    let (hmac, ts, nonce) = (
                                        hmac.trim().to_string(),
                                        ts.trim().to_string(),
                                        nonce.trim().to_string(),
                                    );

                                    extract_body_and_verify_hmac(
                                        req,
                                        self.secret.clone(),
                                        self.nonce.clone(),
                                        hmac,
                                        ts,
                                        nonce,
                                    )
                                    .and_then(move |req| svc_clone.call(req))
                                    .boxed()
                                },
                                _ => bad_request(
                                    "Invalid HMAC/Timestamp/Nonce charecters, couldnt serialize",
                                ),
                            }
                        },
                        _ => bad_request("Missing HMAC/Timestamp/Nonce"),
                    }
                } else {
                    Box::pin(async move {
                        hyper::Response::builder()
                            .status(hyper::StatusCode::METHOD_NOT_ALLOWED)
                            .body(hyper::Body::from("Admin Module only accepts POST"))
                            .map_err(|e| e.into())
                    })
                }
            },
            _ => self.main_server.call(req),
        }
    }
}

impl<MainModule, AdminModule> TowerService<Request<Body>> for RpcService<MainModule, AdminModule>
where
    Self: Send + 'static,
    MainModule: ServiceMarker,
    AdminModule: ServiceMarker,
    <MainModule as TowerService<Request<Body>>>::Future: Send,
    <AdminModule as TowerService<Request<Body>>>::Future: Send,
{
    type Response = Response<Body>;
    type Error = BoxedError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // jsonrpsee does nothing in thier implementation
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.route(req)
    }
}

fn bad_request(
    b: impl Into<String>,
) -> Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>> {
    let b = b.into();

    Box::pin(async move {
        hyper::Response::builder()
            .status(hyper::StatusCode::BAD_REQUEST)
            .body(hyper::Body::from(b))
            .map_err(|e| e.into())
    })
}

async fn extract_body_and_verify_hmac(
    req: Request<Body>,
    secret: Arc<[u8; 32]>,
    sys_nonce: Arc<AtomicU32>,
    hmac: String,
    ts: String,
    nonce: String,
) -> Result<Request<Body>, BoxedError> {
    let (parts, body) = req.into_parts();

    // todo(n): size upper bound?
    let body = hyper::body::to_bytes(body).await?.to_vec();

    verify_hmac(&secret, &sys_nonce, &body, &hmac, &ts, &nonce)?;

    Ok(Request::from_parts(parts, body.into()))
}

fn verify_hmac(
    secret: &[u8; 32],
    sys_nonce: &AtomicU32,
    body: &[u8],
    hmac: &str,
    ts: &str,
    nonce: &str,
) -> Result<(), BoxedError> {
    if hmac.trim_start_matches("0x").len() != 64 {
        return Err("HMAC is not 32 bytes".into());
    }

    let our_nonce = sys_nonce.load(std::sync::atomic::Ordering::Acquire);

    let user_timestamp = ts.parse::<u64>()?;
    let user_nonce = nonce.parse::<u32>()?;

    if user_nonce != our_nonce {
        return Err(format!(
            "Nonce is not valid, expected {}, got {}",
            our_nonce, user_nonce
        )
        .into());
    }

    let sys = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(Box::new)?
        .as_secs();

    // it should be more than 5 seconsd in the past
    if sys - 5 > user_timestamp {
        return Err("Timestamp is too far in the past".into());
    }

    // it should not be in the future
    if user_timestamp > sys {
        return Err("Timestamp is too far in the future".into());
    }

    let correct_hmac = create_hmac(secret, body, user_timestamp, user_nonce);

    if hmac != correct_hmac {
        return Err("Recreated HMAC doesnt match".into());
    }

    if sys_nonce
        .compare_exchange(
            our_nonce,
            our_nonce + 1,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_err()
    {
        return Err("Nonce was invalidated during request".into());
    }

    Ok(())
}

/// Creates a hmac from a timestamp and nonce
///
/// ts: the unix timestamp in seconds
/// nonce: returned from the '/admin/nonce' route
///
/// # Panics
///
/// If the key is not valid for the HMAC
pub fn create_hmac(secret: &[u8; 32], body: &[u8], ts: u64, nonce: u32) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
    mac.update(HMAC_SALT);
    mac.update(ts.to_string().as_bytes());
    mac.update(nonce.to_string().as_bytes());
    mac.update(body);

    hex::encode(mac.finalize().into_bytes())
}
