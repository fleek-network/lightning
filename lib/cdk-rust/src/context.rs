use fleek_crypto::ClientPublicKey;
use tokio::sync::mpsc::Receiver;

use crate::driver::RequestResponse;
use crate::mode::ModeSetting;

pub struct Context {
    mode: ModeSetting,
    pk: ClientPublicKey,
    shutdown: bool,
}

impl Context {
    pub fn new(mode: ModeSetting, pk: ClientPublicKey) -> Self {
        Self {
            mode,
            pk,
            shutdown: false,
        }
    }

    /// Returns the mode setting of the connection.
    pub fn mode(&self) -> &ModeSetting {
        &self.mode
    }

    pub fn pk(&self) -> &ClientPublicKey {
        &self.pk
    }

    // Shut downs the connection.
    pub fn shutdown(&mut self) {
        self.shutdown = true;
    }
}
