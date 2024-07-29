use fastcrypto::hash::HashFunction;
use fleek_blake3 as blake3;
use lightning_interfaces::types::{Epoch, NodeIndex};
use lightning_interfaces::{ToDigest, TranscriptBuilder};
use narwhal_crypto::DefaultHashFunction;
use narwhal_types::{BatchDigest, Transaction};
use serde::{Deserialize, Serialize};

pub type Digest = [u8; 32];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticStampedParcel {
    pub transactions: Vec<Transaction>,
    pub last_executed: Digest,
    pub epoch: Epoch,
    pub sub_dag_index: u64,
    pub sub_dag_round: u64,
}

impl ToDigest for AuthenticStampedParcel {
    fn transcript(&self) -> TranscriptBuilder {
        panic!("We don't need this here");
    }

    fn to_digest(&self) -> Digest {
        let batch_digest =
            BatchDigest::new(DefaultHashFunction::digest_iterator(self.transactions.iter()).into());

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(self.transactions.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&batch_digest.0);
        bytes.extend_from_slice(&self.last_executed);
        bytes.extend_from_slice(&self.sub_dag_index.to_le_bytes());
        bytes.extend_from_slice(&self.sub_dag_round.to_le_bytes());

        blake3::hash(&bytes).into()
    }
}

/// A message an authority sends out attest that an Authentic stamp parcel is accurate. When an edge
/// node gets 2f+1 of these it commits the transactions in the parcel
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitteeAttestation {
    /// The digest we are attesting is correct
    pub digest: Digest,
    /// We send random bytes with this message so it gives it a unique hash and differentiates it
    /// from the other committee members attestation broadcasts
    pub node_index: NodeIndex,
    pub epoch: Epoch,
}
