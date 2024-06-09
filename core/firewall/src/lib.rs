pub mod commands;
pub mod policy;
pub mod rate_limiting;
pub mod service;

use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub use commands::{CommandCenter, FireWallRequest, FirewallCommand};
use lightning_types::{ConnectionPolicyConfig, FirewallConfig, RateLimitingConfig};
use policy::{ConnectionPolicy, ConnectionPolicyMode};
use rate_limiting::{RateLimiting, RateLimitingMode};
use tokio::sync::mpsc;

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
///
/// This firewall can also be used more generally in shared context
/// see [`Firewall::shared`] for more information
///
/// Note: A firewall is only updated on the *next* call to it, this means there could be some
/// backpressure on updates if the updater doesnt interact with firewall in between
#[derive(Debug)]
pub struct Firewall {
    name: &'static str,
    command_rx: mpsc::Receiver<commands::FireWallRequest>,
    max_connections: Option<usize>,
    connections: Arc<AtomicUsize>,
    policy: ConnectionPolicy,
    rate_limiting: RateLimiting,
}

/// Admin functionality for the firewall
impl Firewall {
    pub fn new(
        name: &'static str,
        max_connections: Option<usize>,
        policy: ConnectionPolicy,
        rate_limiting: RateLimiting,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);

        if rate_limiting.policy_type() == RateLimitingMode::Per
            && policy.mode() != ConnectionPolicyMode::Whitelist
        {
            tracing::warn!(
                %name,
                "Rate limiting is set to per ip but the policy is not whitelist,
                this means that anyone without a policy can connect to the endpoint!"
            );
        }

        commands::CommandCenter::global().register(name, command_tx);

        Self {
            name,
            command_rx,
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

    /// Wrap a service with this firewall
    ///
    /// See [`service::FirewalledServer`] for more information
    pub fn service<S: Clone>(self, inner: S) -> service::Firewalled<S> {
        service::Firewalled::new(self, inner)
    }

    /// Create a shareable instance of a firewall
    ///
    /// This is not really the intended use but its useful for the handshake where we expose TCP and
    /// other transports that arent modeled as services
    pub fn shared(self) -> SharedFirewall {
        SharedFirewall {
            firewall: Arc::new(Mutex::new(self)),
        }
    }
}

impl Firewall {
    /// Gets a lock that automatically releases the connection when dropped
    ///
    /// its up to the user to ensure that the lock is dropped when the connection is actually done
    pub fn lock(&self) -> FirewallLock {
        FirewallLock {
            connections: self.connections.clone(),
        }
    }

    /// Warning! Every check should have a corresponding release_connection call
    /// to avoid leaking connections
    #[tracing::instrument(skip(self), fields(name = %self.name))]
    pub fn check(&mut self, ip: IpAddr) -> Result<(), FirewallError> {
        self.check_for_updates();

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
    fn check_for_updates(&mut self) {
        while let Ok(request) = self.command_rx.try_recv() {
            let (command, sender) = request.parts();

            let res = match command {
                commands::FirewallCommand::MaxConnections(max) => {
                    self.set_max_connections(max);

                    Ok(())
                },
                commands::FirewallCommand::SetGlobalPolicy(rules) => {
                    self.rate_limiting.set_global_policy(rules)
                },
                commands::FirewallCommand::SetPolicyForIp(ip, rules) => {
                    self.rate_limiting.set_policy(ip, rules)
                },
                commands::FirewallCommand::ChangeRateLimitingPolicyType(policy) => {
                    self.rate_limiting.set_policy_type(policy);

                    Ok(())
                },
                commands::FirewallCommand::ToggleBlacklist(ip) => self.policy.toggle_blacklist(ip),
                commands::FirewallCommand::ToggleWhitelist(ip) => self.policy.toggle_whitelist(ip),
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

/// A shared firewall that internally uses a blocking mutex to coordinate access
///
/// This is fine in async conexts because worst case were just doing a few hashmap loopkups under
/// the lock
pub struct SharedFirewall {
    firewall: Arc<Mutex<Firewall>>,
}

impl Clone for SharedFirewall {
    fn clone(&self) -> Self {
        Self {
            firewall: self.firewall.clone(),
        }
    }
}

impl SharedFirewall {
    pub fn check(&self, ip: IpAddr) -> Result<(), FirewallError> {
        self.firewall.lock().unwrap().check(ip)
    }
}

pub struct FirewallLock {
    connections: Arc<AtomicUsize>,
}

impl Drop for FirewallLock {
    fn drop(&mut self) {
        self.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

impl From<FirewallConfig> for Firewall {
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
