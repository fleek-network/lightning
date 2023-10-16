use fleek_crypto::ClientPublicKey;

use crate::mode::ModeSetting;

pub struct Context {
    mode: ModeSetting,
    // The provider's public key.
    pk: ClientPublicKey,
}

impl Context {
    pub fn new(mode: ModeSetting, pk: ClientPublicKey) -> Self {
        Self { mode, pk }
    }

    /// Returns the mode setting of the connection.
    pub fn mode(&self) -> &ModeSetting {
        &self.mode
    }

    pub fn pk(&self) -> &ClientPublicKey {
        &self.pk
    }
}
