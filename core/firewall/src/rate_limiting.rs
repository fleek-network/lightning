use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::{Deref, DerefMut};

use lightning_types::{Period, RateLimitingRule};
use serde::{Deserialize, Serialize};

use crate::{AdminError, FirewallError};

#[derive(Debug)]
pub enum RateLimiting {
    /// There is no rate limiting
    None,
    /// A rate limiting policy per ip
    /// If there is no policy for an ip it will be allowed to make as many requests as it wants
    /// This is really only useful for the whitelisted connection policy setting, otherwise anyone
    /// can connect with no limits
    Per(HashMap<IpAddr, Vec<RateLimitingPolicy>>),
    /// A global rate limiting policy
    /// This is a policy that is applied to all IPs iff they do not have a specific policy set
    /// already If a policy is set for an IP it will override the global policy
    ///
    /// In global mode, the policys in affect are *either global or not*
    /// in other words you cannot have a mix of both
    WithGlobal {
        global: Vec<RateLimitingRule>,
        per: HashMap<IpAddr, Vec<IsGlobal<RateLimitingPolicy>>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RateLimitingMode {
    None,
    Per,
    Global,
}

impl RateLimiting {
    pub fn none() -> Self {
        Self::None
    }

    pub fn global(policy: Vec<RateLimitingRule>) -> Self {
        Self::WithGlobal {
            global: policy,
            per: HashMap::new(),
        }
    }

    pub fn per() -> Self {
        Self::Per(HashMap::new())
    }
}

/// Admin functionality for the firewall
impl RateLimiting {
    /// Sets the rate limiting policy for the given ip
    ///
    /// WARNING! this will override any existing policy for the given ip
    pub fn set_policy(
        &mut self,
        ip: IpAddr,
        policy: Vec<RateLimitingRule>,
    ) -> Result<(), AdminError> {
        tracing::trace!(
            "Setting rate limiting policy for ip: {:?} to: {:?}",
            ip,
            policy
        );

        match self {
            RateLimiting::None => {
                return Err(AdminError::RateLimitingError(RateLimitingMode::None));
            },
            RateLimiting::Per(policy_map) => {
                policy_map.insert(ip, policy.into_iter().map(Into::into).collect());
            },
            RateLimiting::WithGlobal { per, .. } => {
                per.insert(
                    ip,
                    policy.into_iter().map(|p| IsGlobal::no(p.into())).collect(),
                );
            },
        };

        Ok(())
    }

    /// Set the global policy for all ips, doesn't override existing user policies,
    ///
    /// WARNING! this will override the exisiting global policy and reset any global policies
    pub fn set_global_policy(&mut self, rules: Vec<RateLimitingRule>) -> Result<(), AdminError> {
        tracing::trace!("Setting global rate limiting policy to: {:?}", rules);

        match self {
            RateLimiting::None => {
                return Err(AdminError::RateLimitingError(RateLimitingMode::None));
            },
            RateLimiting::Per(_) => {
                return Err(AdminError::RateLimitingError(RateLimitingMode::Per));
            },
            RateLimiting::WithGlobal { global, per } => {
                *global = rules;

                // remove all old global rules
                per.retain(|_, v| v.first().is_some_and(|f| !f.is_global));
            },
        }

        Ok(())
    }

    /// Change the type of rate limiting policy
    ///
    /// Global -> Per: global rules are removed, per rules are kept
    /// Per -> Global: per rules not affected
    pub fn set_policy_type(&mut self, typee: RateLimitingMode) {
        tracing::trace!("Setting rate limiting policy type to: {:?}", typee);

        match typee {
            RateLimitingMode::None => {
                *self = Self::None;
            },
            RateLimitingMode::Per => match self {
                RateLimiting::None => {
                    *self = Self::per();
                },
                // move all the rules to per user rules, filtering any old global ones
                RateLimiting::WithGlobal { per, .. } => {
                    *self = Self::Per(
                        std::mem::take(per)
                            .into_iter()
                            .filter_map(|(k, v)| {
                                // take the user rules if theyre are not global
                                if v.first().is_some_and(|policy| !policy.is_global) {
                                    return Some((k, v.into_iter().map(IsGlobal::inner).collect()));
                                }

                                None
                            })
                            .collect(),
                    );
                },
                // no op
                RateLimiting::Per(_) => {},
            },
            RateLimitingMode::Global => match self {
                RateLimiting::None => {
                    *self = Self::global(vec![]);
                },
                // migrate all per rules
                RateLimiting::Per(policy) => {
                    *self = Self::WithGlobal {
                        global: vec![],
                        per: std::mem::take(policy)
                            .into_iter()
                            .map(|(k, v)| (k, v.into_iter().map(IsGlobal::no).collect()))
                            .collect(),
                    };
                },
                // no op
                RateLimiting::WithGlobal { .. } => {},
            },
        }
    }

    /// Clear the rules for the given ip
    pub fn clear_rules(&mut self, ip: IpAddr) {
        tracing::trace!("Clearing rate limiting rules for ip: {:?}", ip);

        match self {
            RateLimiting::None => {},
            RateLimiting::Per(policy) => {
                policy.remove(&ip);
            },
            RateLimiting::WithGlobal { per, .. } => {
                per.remove(&ip);
            },
        }
    }

    pub fn policy_type(&self) -> RateLimitingMode {
        match self {
            RateLimiting::None => RateLimitingMode::None,
            RateLimiting::Per(_) => RateLimitingMode::Per,
            RateLimiting::WithGlobal { .. } => RateLimitingMode::Global,
        }
    }
}

impl RateLimiting {
    pub fn check(&mut self, ip: IpAddr) -> Result<(), FirewallError> {
        match self {
            RateLimiting::None => (),
            RateLimiting::Per(policy) => {
                if let Some(policy) = policy.get_mut(&ip) {
                    for p in policy.iter_mut() {
                        p.check()?
                    }
                }
            },
            RateLimiting::WithGlobal { global, per } => {
                if let Some(policy) = per.get_mut(&ip) {
                    for p in policy.iter_mut() {
                        p.check()?
                    }

                    // return early since we have per policy setup
                    return Ok(());
                }

                // there is no policy for this ip, use the global policy
                let mut policies: Vec<IsGlobal<RateLimitingPolicy>> = global
                    .iter()
                    .copied()
                    .map(|p| IsGlobal::yes(p.into()))
                    .collect();

                for p in policies.iter_mut() {
                    p.check()?
                }

                per.insert(ip, policies);
            },
        };

        Ok(())
    }
}

/// A rate limiting policy with linear decay
///
/// An implementation of the token bucket
///
/// see [`RateLimitingPolicy::new`] for more information
#[derive(Debug, Clone)]
pub struct RateLimitingPolicy {
    period: Period,
    max_requests: f64,
    last_request: std::time::Instant,

    rate: f64,
    allowed: f64,
}

impl RateLimitingPolicy {
    pub fn new(period: Period, max_requests: u64) -> Self {
        Self {
            period,
            max_requests: max_requests as f64,
            last_request: std::time::Instant::now(),
            allowed: max_requests as f64,
            rate: max_requests as f64 / period.as_millis() as f64,
        }
    }

    /// Check (and increment the counter) if a request is allowed
    pub fn check(&mut self) -> Result<(), FirewallError> {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_request).as_millis();

        if elapsed > self.period.as_millis() as u128 {
            self.allowed = self.max_requests - 1.0;
            self.last_request = now;

            return Ok(());
        }

        // cast is safe since we checked above
        let elapsed = elapsed as f64;

        // top up the bucket
        let new_allowed = self.max_requests.min(self.allowed + elapsed * self.rate);
        if new_allowed >= 1.0 {
            // include the new request
            self.allowed = new_allowed - 1.0;
            self.last_request = now;

            Ok(())
        } else {
            self.allowed = new_allowed;
            self.last_request = now;

            Err(FirewallError::RateLimitExceeded(self.max_requests))
        }
    }
}

impl From<RateLimitingRule> for RateLimitingPolicy {
    fn from(
        RateLimitingRule {
            period,
            max_requests,
        }: RateLimitingRule,
    ) -> Self {
        Self::new(period, max_requests)
    }
}

#[derive(Debug)]
pub struct IsGlobal<T> {
    is_global: bool,
    value: T,
}

impl<T> IsGlobal<T> {
    fn yes(value: T) -> Self {
        Self {
            is_global: true,
            value,
        }
    }

    fn no(value: T) -> Self {
        Self {
            is_global: false,
            value,
        }
    }

    fn inner(self) -> T {
        self.value
    }
}

impl<T> Deref for IsGlobal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for IsGlobal<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[cfg(test)]
mod rate_limiting_test {
    #[test]
    fn test_allows_proper_amount_of_requests() {
        let mut policy = super::RateLimitingPolicy::new(super::Period::Second, 10);

        for _ in 0..10 {
            assert!(policy.check().is_ok());
            println!("{:?}", policy);
        }

        assert!(policy.check().is_err());
    }

    #[test]
    fn test_decays_as_expected() {
        let mut policy = super::RateLimitingPolicy::new(super::Period::Second, 10);

        for _ in 0..10 {
            assert!(policy.check().is_ok());
            println!("{:?}", policy);
        }

        std::thread::sleep(std::time::Duration::from_secs(1));

        for _ in 0..10 {
            assert!(policy.check().is_ok());
            println!("{:?}", policy);
        }

        std::thread::sleep(std::time::Duration::from_millis(500));

        for _ in 0..5 {
            assert!(policy.check().is_ok());
            println!("{:?}", policy);
        }

        assert!(policy.check().is_err());
    }
}
