pub mod commands;
pub mod policy;
pub mod rate_limiting;
pub mod service;

use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub use commands::{CommandCenter, FireWallRequest, FirewallCommand};
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
    #[error("Rate limit exceeded curr: {}, max: {}", .0, .1)]
    RateLimitExceeded(u64, u64),
    #[error("Connection limit exceeded")]
    ConnectionLimitExceeded,
    #[error("IP is blacklisted")]
    Blacklisted,
    #[error("IP is not whitelisted")]
    NotWhitelisted,
}

/// An indivudal firewall that can be used to protect an endpoint
///
/// This firewall can easily wrap any [`tower::Service`] and will automatically apply the firewall
/// rules see [`Firewall::service`] for more information
pub struct Firewall {
    /// Unfortunatly we need to use a tokio [`Mutex`] here because we need to be able to
    /// lock the firewall in a spawn, and ['std::sync::MutexGuard'] is not [`Send`]
    inner: Arc<Mutex<Inner>>,
    connections: Arc<AtomicUsize>,
}

impl Clone for Firewall {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            connections: self.connections.clone(),
        }
    }
}

impl Firewall {
    pub fn new(
        name: &'static str,
        max_connections: Option<usize>,
        policy: ConnectionPolicy,
        rate_limiting: RateLimiting,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        commands::CommandCenter::global().register(name, command_tx);

        let inner = Inner::new(name, max_connections, policy, rate_limiting);
        let connections = inner.connections.clone();

        let this = Self {
            inner: Arc::new(Mutex::new(inner)),
            connections,
        };

        tokio::spawn(this.clone().update_loop(command_rx));

        this
    }

    pub async fn check(&self, ip: IpAddr) -> Result<(), FirewallError> {
        let mut firewall = self.inner.lock().await;

        firewall.check(ip)
    }

    /// Wrap a service with this firewall
    ///
    /// See [`service::FirewalledServer`] for more information
    pub fn service<S: Clone>(self, inner: S) -> service::Firewalled<S> {
        service::Firewalled::new(self, inner)
    }

    /// Warning! Every check should have a corresponding release_connection call
    /// This function can panic if used incorrectly
    pub fn release_connection(&self) {
        self.connections.fetch_sub(1, Ordering::AcqRel);
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
                commands::FirewallCommand::MaxConnections(max) => {
                    this.set_max_connections(max);

                    Ok(())
                },
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
    }
}

#[derive(Debug)]
pub(crate) struct Inner {
    name: &'static str,
    max_connections: Option<usize>,
    connections: Arc<AtomicUsize>,
    policy: ConnectionPolicy,
    rate_limiting: RateLimiting,
}

/// Admin functionality for the firewall
impl Inner {
    pub fn new(
        name: &'static str,
        max_connections: Option<usize>,
        policy: ConnectionPolicy,
        rate_limiting: RateLimiting,
    ) -> Self {
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
            name,
            max_connections,
            connections: Arc::new(AtomicUsize::new(0)),
            policy,
            rate_limiting,
        }
    }

    pub fn set_max_connections(&mut self, max_connections: usize) {
        if max_connections == 0 {
            self.max_connections = None;
        } else {
            self.max_connections = Some(max_connections);
        }
    }
}

impl Inner {
    /// Warning! Every check should have a corresponding release_connection call
    /// to avoid leaking connections
    #[tracing::instrument(skip(self), fields(name = %self.name))]
    pub fn check(&mut self, ip: IpAddr) -> Result<(), FirewallError> {
        loop {
            let current = self.connections.load(Ordering::Acquire);

            if let Some(max) = self.max_connections {
                if current >= max {
                    return Err(FirewallError::ConnectionLimitExceeded);
                }
            }

            if self
                .connections
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        // Always check the policy first
        // someone shouldnt need ratelimiting if theyre not even
        // allowed to connect
        self.policy.check(ip)?;
        self.rate_limiting.check(ip)?;

        Ok(())
    }
}

impl From<FirewallConfig> for Firewall {
    fn from(config: FirewallConfig) -> Self {
        let inner: Inner = config.into();
        let connections = inner.connections.clone();

        Self {
            inner: Arc::new(Mutex::new(inner)),
            connections,
        }
    }
}

impl From<FirewallConfig> for Inner {
    fn from(config: FirewallConfig) -> Self {
        let FirewallConfig {
            name,
            max_connections,
            connection_policy,
            rate_limiting,
        } = config;

        let policy = match connection_policy {
            ConnectionPolicyConfig::All => ConnectionPolicy::All,
            ConnectionPolicyConfig::Whitelist { members } => ConnectionPolicy::whitelist(members),
            ConnectionPolicyConfig::Blacklist { members } => ConnectionPolicy::blacklist(members),
        };

        let rate = match rate_limiting {
            RateLimitingConfig::None => RateLimiting::none(),
            RateLimitingConfig::Per => RateLimiting::per(),
            RateLimitingConfig::Global { rules } => RateLimiting::global(rules),
        };

        Self::new(name, max_connections, policy, rate)
    }
}
