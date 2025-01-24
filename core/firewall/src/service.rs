use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Poll};

use tower::Service;

use crate::{Firewall, FirewallError};

pub type BoxedFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync>>;

/// A wrapper around a serivce that implements MakeService
/// for various types that we can extract an ip address from.
/// In this can be used so far with hyper and axum, adding new
/// types is easy just implement [`IntoIpAddr`] for the type
pub struct Firewalled<S> {
    firewall: Firewall,
    svc: S,
}

impl<S> Firewalled<S> {
    pub fn new(firewall: Firewall, svc: S) -> Self {
        Self { firewall, svc }
    }
}

/// Auto implements [`tower::MakeService``] for all types T: IntoIpAddr when S: Service
impl<T, S> Service<T> for Firewalled<S>
where
    S: Unpin + Clone,
    T: IntoIpAddr,
{
    type Response = ConnectedService<S>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<ConnectedService<S>, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        let ip = req.ip_addr();
        let svc = self.svc.clone();

        std::future::ready(Ok(ConnectedService {
            svc,
            ip,
            firewall: self.firewall.clone(),
        }))
    }
}

#[derive(Clone)]
pub struct ConnectedService<S> {
    svc: S,
    firewall: Firewall,
    ip: std::net::IpAddr,
}

/// A wrapper around the service so we can tell the firewall we dropped a connection
impl<S, Req, Res> Service<Req> for ConnectedService<S>
where
    S: Service<Req, Response = Res>,
    S: Unpin,
    S::Future: Send + Unpin,
    Req: Unpin,
    Res: From<FirewallError>,
{
    type Response = Res;
    type Error = S::Error;
    type Future = ServiceFuture<BoxedFuture<Result<(), FirewallError>>, S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.svc.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let firewall_fut = Box::pin({
            let firewall = self.firewall.clone();
            let ip = self.ip;
            async move { firewall.check(ip).await }
        });

        let svc_fut = self.svc.call(req);

        // `call` might get called again!
        ServiceFuture::new(firewall_fut, svc_fut)
    }
}

pub struct ServiceFuture<FirewallFut, SvcFut> {
    called: bool,
    svc_fut: SvcFut,
    firewall_fut: FirewallFut,
}

impl<FirewallFut, SvcFut> ServiceFuture<FirewallFut, SvcFut> {
    pub fn new(firewall_fut: FirewallFut, svc_fut: SvcFut) -> Self {
        Self {
            called: false,
            firewall_fut,
            svc_fut,
        }
    }
}

impl<FirewallFut, SvcFut, Res, Err> Future for ServiceFuture<FirewallFut, SvcFut>
where
    Self: Send,
    Res: From<FirewallError>,
    FirewallFut: Future<Output = Result<(), FirewallError>> + Unpin,
    SvcFut: Future<Output = Result<Res, Err>> + Unpin,
{
    type Output = Result<Res, Err>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // If we haven't called the firewall yet, do so
        if !self.called {
            let pinned = Pin::new(&mut self.firewall_fut);

            match ready!(pinned.poll(cx)) {
                Ok(_) => {},
                Err(e) => return Poll::Ready(Ok(e.into())),
            }

            self.called = true;
        }

        Pin::new(&mut self.svc_fut).poll(cx)
    }
}

/// A helper trait because every server implementation has a different way of getting the IP address
/// :)
pub trait IntoIpAddr {
    fn ip_addr(&self) -> std::net::IpAddr;
}

impl IntoIpAddr for &hyper::server::conn::AddrStream {
    fn ip_addr(&self) -> std::net::IpAddr {
        self.remote_addr().ip()
    }
}
