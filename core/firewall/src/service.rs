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
pub struct ConnectedService<S: Clone> {
    svc: S,
    firewall: Firewall,
    ip: std::net::IpAddr,
}

/// A wrapper around the service so we can tell the firewall we dropped a connection
impl<S, Req, Res> Service<Req> for ConnectedService<S>
where
    S: Service<Req, Response = Res>,
    S: Unpin,
    S: Clone,
    S::Future: Unpin,
    Req: Unpin,
    Res: From<FirewallError>,
{
    type Response = Res;
    type Error = S::Error;
    type Future = ServiceFuture<S, Req, BoxedFuture<Result<(), FirewallError>>, S::Future>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let fut = Box::pin({
            let firewall = self.firewall.clone();
            let ip = self.ip;
            async move { firewall.check(ip).await }
        });

        // This might get called again!
        ServiceFuture::new(self.svc.clone(), fut, req)
    }
}

pub struct ServiceFuture<S, Req, FirewallFut, SvcFut> {
    called: bool,
    svc: S,
    svc_fut: Option<SvcFut>,
    firewall_fut: FirewallFut,
    req: Option<Req>,
}

impl<S, Req, FirewallFut, SvcFut> ServiceFuture<S, Req, FirewallFut, SvcFut> {
    pub fn new(svc: S, firewall_fut: FirewallFut, req: Req) -> Self {
        Self {
            called: false,
            svc,
            svc_fut: None,
            firewall_fut,
            req: Some(req),
        }
    }
}

impl<S, Req, FirewallFut, SvcFut> Future for ServiceFuture<S, Req, FirewallFut, SvcFut>
where
    Req: Unpin,
    S: Unpin,
    S: Service<Req, Future = SvcFut>,
    S::Response: From<FirewallError>,
    FirewallFut: Future<Output = Result<(), FirewallError>> + Unpin,
    SvcFut: Future<Output = Result<S::Response, S::Error>> + Unpin,
    S::Future: Unpin,
{
    type Output = Result<S::Response, S::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut();

        // If we haven't called the firewall yet, do so
        if !this.called {
            let pinned = Pin::new(&mut this.firewall_fut);
            match ready!(pinned.poll(cx)) {
                Ok(_) => {},
                Err(e) => return Poll::Ready(Ok(e.into())),
            }

            this.called = true;
        }

        loop {
            if let Some(fut) = this.svc_fut.as_mut() {
                match Pin::new(fut).poll(cx) {
                    Poll::Ready(res) => return Poll::Ready(res),
                    Poll::Pending => return Poll::Pending,
                }
            }

            let req = this.req.take().expect("req is none");
            this.svc_fut = Some(this.svc.call(req));
        }
    }
}

/// A helper trait because every server implementation has a different way of getting the IP address
/// :)
pub trait IntoIpAddr {
    fn ip_addr(&self) -> std::net::IpAddr;
}

impl<'a> IntoIpAddr for &'a hyper::server::conn::AddrStream {
    fn ip_addr(&self) -> std::net::IpAddr {
        self.remote_addr().ip()
    }
}
