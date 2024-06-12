use std::pin::Pin;
use std::task::ready;

use tower::Service;

use crate::{Firewall, FirewallError};

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

pub struct ConnectedService<S> {
    /// The actual service
    svc: S,
}

/// Auto implements [`tower::MakeService``] for all types T: IntoIpAddr when S: Service
impl<T, S> Service<T> for Firewalled<S>
where
    S: Unpin + Clone,
    T: IntoIpAddr,
{
    type Response = ConnectedService<S>;
    type Error = FirewallError;
    type Future = MakeSvcFuture<
        S,
        // todo can we make this not dynamic?
        Pin<Box<dyn std::future::Future<Output = Result<(), FirewallError>> + Send>>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        let ip = req.ip_addr();

        MakeSvcFuture {
            svc: Some(self.svc.clone()),
            ip,
            fut: Box::pin({
                let firewall = self.firewall.clone();

                async move { firewall.check(ip).await }
            }),
        }
    }
}

pub struct MakeSvcFuture<S, Fut> {
    svc: Option<S>,
    ip: std::net::IpAddr,
    fut: Fut,
}

impl<S, Fut> std::future::Future for MakeSvcFuture<S, Fut>
where
    S: Clone + Unpin,
    Fut: std::future::Future<Output = Result<(), FirewallError>> + Unpin,
{
    type Output = Result<ConnectedService<S>, FirewallError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        if this.ip.is_loopback() {
            return std::task::Poll::Ready(Ok(ConnectedService {
                svc: this.svc.take().expect("firewall svc to be present"),
            }));
        }

        let pinned = std::pin::Pin::new(&mut this.fut);

        match ready!(pinned.poll(_cx)) {
            Ok(()) => std::task::Poll::Ready(Ok(ConnectedService {
                svc: this.svc.take().expect("firewall svc to be present"),
            })),
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

/// A wrapper around the service so we can tell the firewall we dropped a connection
impl<S, Req, Res, E> Service<Req> for ConnectedService<S>
where
    S: Service<Req, Response = Res, Error = E>,
{
    type Response = Res;
    type Error = E;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.svc.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        self.svc.call(req)
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
