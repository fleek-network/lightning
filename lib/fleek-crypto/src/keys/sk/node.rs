use std::str::FromStr;

use arrayref::array_ref;
use fastcrypto::ed25519::{Ed25519KeyPair, Ed25519PrivateKey, Ed25519PublicKey};
use fastcrypto::traits::{KeyPair, Signer, ToFromBytes};
use rand::rngs::ThreadRng;
use sec1::der::EncodePem;
use sec1::pkcs8::{AlgorithmIdentifierRef, ObjectIdentifier, PrivateKeyInfo};
use sec1::{pem, LineEnding};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::keys::pk::NodePublicKey;
use crate::{PublicKey, SecretKey};

/// A node's ed25519 main secret key
#[derive(Clone, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct NodeSecretKey([u8; 32]);

impl std::fmt::Debug for NodeSecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("NodeSecretKeyOf")
            .field(&self.to_pk())
            .finish()
    }
}

impl From<Ed25519PrivateKey> for NodeSecretKey {
    fn from(value: Ed25519PrivateKey) -> Self {
        let bytes = value.as_ref();
        NodeSecretKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&NodeSecretKey> for Ed25519PrivateKey {
    fn from(value: &NodeSecretKey) -> Self {
        Ed25519PrivateKey::from_bytes(&value.0).unwrap()
    }
}

impl From<NodeSecretKey> for Ed25519KeyPair {
    fn from(value: NodeSecretKey) -> Self {
        let secret: Ed25519PrivateKey = (&value).into();
        secret.into()
    }
}

impl SecretKey for NodeSecretKey {
    type PublicKey = NodePublicKey;

    fn generate() -> Self {
        let pair = Ed25519KeyPair::generate(&mut ThreadRng::default());
        pair.private().into()
    }

    fn decode_pem(encoded: &str) -> Option<Self> {
        let (label, der_bytes) = pem::decode_vec(encoded.as_bytes()).ok()?;
        if label != "PRIVATE KEY" {
            return None;
        }
        let info = PrivateKeyInfo::try_from(der_bytes.as_ref()).ok()?;
        Some(Self(*array_ref!(info.private_key, 2, 32)))
    }

    fn encode_pem(&self) -> String {
        let algorithm = AlgorithmIdentifierRef {
            oid: ObjectIdentifier::from_str("1.3.101.112").unwrap(),
            parameters: None,
        };
        PrivateKeyInfo::new(algorithm, &{
            let mut key = vec![0x04, 0x20];
            key.append(&mut self.0.into());
            key
        })
        .to_pem(LineEnding::LF)
        .unwrap()
    }

    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: Ed25519PrivateKey = self.into();
        let pair: Ed25519KeyPair = secret.into();
        pair.sign(digest).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: &Ed25519PrivateKey = &self.into();
        let pubkey: Ed25519PublicKey = secret.into();
        pubkey.into()
    }
}
