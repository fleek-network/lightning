pub mod commands;
pub mod policy;
pub mod rate_limiting;
pub mod service;

use std::net::IpAddr;
use std::sync::Arc;

pub use commands::{CommandCenter, FireWallRequest, FirewallCommand};
use lightning_interfaces::ShutdownWaiter;
use lightning_types::{ConnectionPolicyConfig, FirewallConfig, RateLimitingConfig};
use policy::{ConnectionPolicy, ConnectionPolicyMode};
use rate_limiting::{RateLimiting, RateLimitingMode};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("Bad connection policy type for the given request {:?}", .0)]
    WrongPolicyType(ConnectionPolicyMode),
    #[error("Bad connection policy type for the given request {:?}", .0)]
    RateLimitingError(RateLimitingMode),
}

#[derive(Debug, thiserror::Error)]
pub enum FirewallError {
    #[error("Rate limit exceeded, current count: {}", .0)]
    RateLimitExceeded(f64),
    #[error("IP is blacklisted")]
    Blacklisted,
    #[error("IP is not whitelisted")]
    NotWhitelisted,
}

impl From<FirewallError> for hyper::Response<hyper::Body> {
    fn from(e: FirewallError) -> Self {
        let status = match e {
            FirewallError::RateLimitExceeded(_) => hyper::StatusCode::TOO_MANY_REQUESTS,
            FirewallError::Blacklisted => hyper::StatusCode::FORBIDDEN,
            FirewallError::NotWhitelisted => hyper::StatusCode::FORBIDDEN,
        };

        hyper::Response::builder()
            .status(status)
            .body(hyper::Body::empty())
            .unwrap()
    }
}

/// An indivudal firewall that can be used to protect an endpoint
///
/// This firewall can easily wrap any [`tower::Service`] and will automatically apply the firewall
/// rules see [`Firewall::service`] for more information
pub struct Firewall {
    /// Unfortunatly we need to use a tokio [`Mutex`] here because we need to be able to
    /// lock the firewall in a spawn, and ['std::sync::MutexGuard'] is not [`Send`]
    inner: Arc<Mutex<Inner>>,
}

impl Clone for Firewall {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Firewall {
    pub fn new(
        name: String,
        policy: ConnectionPolicy,
        rate_limiting: RateLimiting,
        shutdown: ShutdownWaiter,
    ) -> Self {
        let inner = Inner::new(&name, policy, rate_limiting);

        let (command_tx, command_rx) = mpsc::channel(100);
        commands::CommandCenter::global().register(name, command_tx);

        let this = Self {
            inner: Arc::new(Mutex::new(inner)),
        };

        let clone = this.clone();
        let waiter = shutdown.clone();
        lightning_interfaces::spawn!(
            async move {
                waiter
                    .run_until_shutdown(clone.update_loop(command_rx))
                    .await;
            },
            "FIREWALL: update loop",
            crucial(shutdown)
        );

        this
    }

    pub fn from_config(config: FirewallConfig, shutdown: ShutdownWaiter) -> Self {
        let FirewallConfig {
            name,
            connection_policy,
            rate_limiting,
        } = config;

        let policy = match connection_policy {
            ConnectionPolicyConfig::All => ConnectionPolicy::All,
            ConnectionPolicyConfig::Whitelist { members } => ConnectionPolicy::whitelist(members),
            ConnectionPolicyConfig::Blacklist { members } => ConnectionPolicy::blacklist(members),
        };

        let rate = match rate_limiting {
            RateLimitingConfig::None => RateLimiting::None,
            RateLimitingConfig::Per => RateLimiting::per(),
            RateLimitingConfig::Global { rules } => RateLimiting::global(rules),
        };

        Self::new(name, policy, rate, shutdown)
    }

    pub async fn check(&self, ip: IpAddr) -> Result<(), FirewallError> {
        // todo: saftey
        if ip.is_loopback() {
            return Ok(());
        }

        let mut firewall = self.inner.lock().await;

        firewall.check(ip)
    }

    /// Wrap a service with this firewall
    ///
    /// See [`service::FirewalledServer`] for more information
    pub fn service<S: Clone>(self, inner: S) -> service::Firewalled<S> {
        service::Firewalled::new(self, inner)
    }
}

impl Firewall {
    /// Updates come from the admin RPC server and are sent to individual firewalls
    /// to update their settings.
    ///
    /// This function will never block
    async fn update_loop(self, mut rx: mpsc::Receiver<commands::FireWallRequest>) {
        while let Some(request) = rx.recv().await {
            let (command, sender) = request.parts();
            let mut this = self.inner.lock().await;

            let res = match command {
                commands::FirewallCommand::SetGlobalPolicy(rules) => {
                    this.rate_limiting.set_global_policy(rules)
                },
                commands::FirewallCommand::SetPolicyForIp(ip, rules) => {
                    this.rate_limiting.set_policy(ip, rules)
                },
                commands::FirewallCommand::ChangeRateLimitingPolicyType(policy) => {
                    this.rate_limiting.set_policy_type(policy);

                    Ok(())
                },
                commands::FirewallCommand::ToggleBlacklist(ip) => this.policy.toggle_blacklist(ip),
                commands::FirewallCommand::ToggleWhitelist(ip) => this.policy.toggle_whitelist(ip),
            };

            match res {
                Ok(_) => {
                    let _ = sender.send(Ok(()));
                },
                Err(e) => {
                    tracing::error!("Error updating firewall: {:?}", e);
                    let _ = sender.send(Err(e));
                },
            };
        }

        tracing::warn!("Firewall update loop ended");
    }
}

#[derive(Debug)]
pub(crate) struct Inner {
    policy: ConnectionPolicy,
    rate_limiting: RateLimiting,
}

/// Admin functionality for the firewall
impl Inner {
    pub fn new(name: &str, policy: ConnectionPolicy, rate_limiting: RateLimiting) -> Self {
        if rate_limiting.policy_type() == RateLimitingMode::Per
            && policy.mode() != ConnectionPolicyMode::Whitelist
        {
            tracing::warn!(
                %name,
                "Rate limiting is set to per ip but the policy is not whitelist,
                this means that anyone without a policy can connect to the endpoint!"
            );
        }

        Self {
            policy,
            rate_limiting,
        }
    }
}

impl Inner {
    pub fn check(&mut self, ip: IpAddr) -> Result<(), FirewallError> {
        // Always check the policy first
        // someone shouldnt need ratelimiting if theyre not even
        // allowed to connect
        self.policy.check(ip)?;
        self.rate_limiting.check(ip)?;

        Ok(())
    }
}
