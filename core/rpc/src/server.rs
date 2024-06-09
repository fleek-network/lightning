use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::{Body, Request, Response};
use tower::Service as TowerService;

use crate::{health, metrics};

type BoxedError = Box<dyn std::error::Error + Sync + Send + 'static>;

/// A struct that implements the Service trait for the RPC server.
pub struct RpcService<MainModule, AdminModule> {
    pub main_server: MainModule,
    pub admin_server: AdminModule,
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
        }
    }
}

impl<MainModule, AdminModule> RpcService<MainModule, AdminModule> {
    pub fn new(main_server: MainModule, admin_server: AdminModule) -> Self {
        Self {
            main_server,
            admin_server,
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
        MainModule: TowerService<
                Request<Body>,
                Response = Response<Body>,
                Error = BoxedError,
                Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>,
            > + Send
            + Clone
            + Unpin,
        AdminModule: TowerService<
                Request<Body>,
                Response = Response<Body>,
                Error = BoxedError,
                Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>,
            > + Send
            + Clone
            + Unpin,
        <MainModule as TowerService<Request<Body>>>::Future: Send,
        <AdminModule as TowerService<Request<Body>>>::Future: Send,
    {
        let path = req.uri().path().to_string().to_ascii_lowercase();
        let method = req.method();

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
            "/admin" => {
                if method == hyper::Method::POST {
                    // todo auth (hmac)
                    self.admin_server.call(req)
                } else {
                    Box::pin(async move {
                        hyper::Response::builder()
                            .status(hyper::StatusCode::METHOD_NOT_ALLOWED)
                            .body(hyper::Body::empty())
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
    MainModule: TowerService<
            Request<Body>,
            Response = Response<Body>,
            Error = BoxedError,
            Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>,
        > + Send
        + Clone
        + Unpin,
    AdminModule: TowerService<
            Request<Body>,
            Response = Response<Body>,
            Error = BoxedError,
            Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxedError>> + Send>>,
        > + Send
        + Clone
        + Unpin,
    <MainModule as TowerService<Request<Body>>>::Future: Send,
    <AdminModule as TowerService<Request<Body>>>::Future: Send,
{
    type Response = Response<Body>;
    type Error = BoxedError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.route(req)
    }
}
