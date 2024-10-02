use lightning_interfaces::types::{ChainId, UpdateMethod, UpdatePayload, UpdateRequest};
use lightning_interfaces::ToDigest;

use super::TransactionSigner;

/// Build and sign new transactions.
pub struct TransactionBuilder;

impl TransactionBuilder {
    /// Build and sign a new update transaction.
    pub fn from_update(
        method: UpdateMethod,
        chain_id: ChainId,
        nonce: u64,
        signer: &TransactionSigner,
    ) -> UpdateRequest {
        let payload = UpdatePayload {
            sender: signer.to_sender(),
            nonce,
            method,
            chain_id,
        };
        let digest = payload.to_digest();
        let signature = signer.sign(&digest);

        UpdateRequest { payload, signature }
    }
}
