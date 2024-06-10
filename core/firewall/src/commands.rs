use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

use lightning_types::RateLimitingRule;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, OnceCell};

use crate::rate_limiting::RateLimitingMode;
use crate::AdminError;

/// Global shared command center for the firewalls.
///
/// This is used to send commands to the firewalls from the admin server.
static COMMAND_CENTER: OnceCell<CommandCenter> = OnceCell::const_new();

/// The command center is used to send commands to the firewalls.
///
/// Every firewall registers itself with the [`COMMAND_CENTER`] and can be accessed by name.
pub struct CommandCenter {
    senders: Mutex<HashMap<&'static str, mpsc::Sender<FireWallRequest>>>,
}

impl CommandCenter {
    /// Get a refrence to the global command center
    pub fn global() -> &'static CommandCenter {
        match COMMAND_CENTER.get() {
            Some(center) => center,
            None => {
                let center = CommandCenter {
                    senders: Mutex::new(HashMap::new()),
                };

                // we dont care if this fails as long as theres one in there
                let _ = COMMAND_CENTER.set(center);
                COMMAND_CENTER.get().unwrap()
            },
        }
    }

    pub fn register(&self, name: &'static str, sender: mpsc::Sender<FireWallRequest>) {
        let mut lock = self.senders.lock().unwrap();

        // overwrite if it exists in case of restarts
        lock.insert(name, sender);
    }

    pub fn sender(&self, name: &str) -> Option<mpsc::Sender<FireWallRequest>> {
        self.senders.lock().unwrap().get(name).cloned()
    }
}

pub struct FireWallRequest {
    res: oneshot::Sender<Result<(), AdminError>>,
    command: FirewallCommand,
}

impl FireWallRequest {
    pub fn new(res: oneshot::Sender<Result<(), AdminError>>, command: FirewallCommand) -> Self {
        Self { res, command }
    }

    pub fn parts(self) -> (FirewallCommand, oneshot::Sender<Result<(), AdminError>>) {
        (self.command, self.res)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FirewallCommand {
    SetGlobalPolicy(Vec<RateLimitingRule>),
    SetPolicyForIp(IpAddr, Vec<RateLimitingRule>),
    ChangeRateLimitingPolicyType(RateLimitingMode),
    ToggleBlacklist(IpAddr),
    ToggleWhitelist(IpAddr),
}

impl FirewallCommand {
    /// Send the command to the firewall.
    pub async fn send(self, sender: &mpsc::Sender<FireWallRequest>) -> anyhow::Result<()> {
        let (tx, _) = oneshot::channel();

        let request = FireWallRequest::new(tx, self);

        sender.send(request).await?;

        Ok(())
    }

    /// Send the command to the firewall and wait for the response.
    ///
    /// Warning! this could take a while, firewall updtaes are lazily evaluated.
    pub async fn send_and_wait(self, sender: &mpsc::Sender<FireWallRequest>) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();

        let request = FireWallRequest::new(tx, self);

        sender.send(request).await?;

        Ok(rx.await??)
    }
}
