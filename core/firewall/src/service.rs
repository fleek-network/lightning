use tower::Service;

use crate::{Firewall, FirewallError, FirewallLock};

/// A wrapper around a serivce that implements MakeService
/// for various types that we can extract an ip address from.
/// In this can be used so far with hyper and axum, adding new
/// types is easy just implement [`IntoIpAddr`] for the type
pub struct Firewalled<S> {
    firewall: Firewall,
    inner: S,
}

impl<S> Firewalled<S> {
    pub fn new(firewall: Firewall, inner: S) -> Self {
        Self { firewall, inner }
    }
}

pub struct ConnectedService<S> {
    /// Only needed so we can tell the firewall we dropped a connection
    _lock: FirewallLock,
    /// The actual service
    inner: S,
}

/// Auto implements make service for all types T: IntoIpAddr when S: Service
impl<T, S> Service<T> for Firewalled<S>
where
    S: Clone,
    T: IntoIpAddr,
{
    type Response = ConnectedService<S>;
    type Error = FirewallError;
    type Future = std::future::Ready<Result<ConnectedService<S>, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        std::future::ready({
            // we dont need any protections for the loopback address
            if req.ip_addr().is_loopback() {
                Ok(ConnectedService::from(self))
            } else {
                self.firewall
                    .check(req.ip_addr())
                    .map(|_| ConnectedService::from(self))
            }
        })
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
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        self.inner.call(req)
    }
}

impl<S: Clone> From<&mut Firewalled<S>> for ConnectedService<S> {
    fn from(firewalled: &mut Firewalled<S>) -> Self {
        Self {
            _lock: firewalled.firewall.lock(),
            inner: firewalled.inner.clone(),
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
