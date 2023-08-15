use fleek_crypto::{NodePublicKey, NodeSignature, PublicKey};
use ink_quill::{ToDigest, TranscriptBuilder};
use serde::{Deserialize, Serialize};

const FN_DHT_ENTRY_DOMAIN: &str = "FLEEK_NETWORK_DHT_ENTRY";

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[repr(u8)]
pub enum KeyPrefix {
    /// The content registry keys.
    ContentRegistry,
}

#[derive(Clone, Debug)]
pub enum DhtRequest {
    Put {
        prefix: KeyPrefix,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Get {
        prefix: KeyPrefix,
        key: Vec<u8>,
    },
}

#[derive(Clone, Debug)]
pub enum DhtResponse {
    Put(()),
    Get(Option<TableEntry>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TableEntry {
    /// The name space of this key-value pair.
    pub prefix: KeyPrefix,
    /// The raw key.
    pub key: Vec<u8>,
    /// The raw value.
    pub value: Vec<u8>,
    /// The originator of this key-value relation.
    pub source: NodePublicKey,
    /// The signature from the source committing to this key-value.
    pub signature: Option<NodeSignature>,
}

impl ToDigest for TableEntry {
    fn transcript(&self) -> TranscriptBuilder {
        TranscriptBuilder::empty(FN_DHT_ENTRY_DOMAIN)
            .with("prefix", &(self.prefix as u8))
            .with("key", &self.key)
            .with("value", &self.value)
            .with("source", &self.source.0)
    }
}

impl TableEntry {
    /// Returns true if a signature is present on this entry. This does not mean
    /// that the signature is actually valid, only that it does exists.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Returns true if the signature on this entry is valid. Returns `false` if
    /// either there is no signature present or if the signature is invalid.
    pub fn is_signature_valid(&self) -> bool {
        if let Some(sig) = &self.signature {
            let digest = self.to_digest();
            self.source.verify(sig, &digest)
        } else {
            false
        }
    }
}
