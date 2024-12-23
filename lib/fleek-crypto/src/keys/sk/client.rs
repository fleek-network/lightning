use arrayref::array_ref;
use fastcrypto::bls12381::min_sig::{BLS12381KeyPair, BLS12381PrivateKey, BLS12381PublicKey};
use fastcrypto::traits::{KeyPair, Signer, ToFromBytes};
use rand::rngs::ThreadRng;
use sec1::{pem, LineEnding};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::super::pk::ClientPublicKey;
use crate::{PublicKey, SecretKey};

const BLS12_381_PEM_LABEL: &str = "LIGHTNING BLS12_381 PRIVATE KEY";

#[derive(Clone, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct ClientSecretKey([u8; 32]);

impl std::fmt::Debug for ClientSecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ClientSecretKeyOf")
            .field(&self.to_pk())
            .finish()
    }
}

impl From<BLS12381PrivateKey> for ClientSecretKey {
    fn from(value: BLS12381PrivateKey) -> Self {
        let bytes = value.as_ref();
        ClientSecretKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&ClientSecretKey> for BLS12381PrivateKey {
    fn from(value: &ClientSecretKey) -> Self {
        BLS12381PrivateKey::from_bytes(&value.0).unwrap()
    }
}

impl From<ClientSecretKey> for BLS12381KeyPair {
    fn from(value: ClientSecretKey) -> Self {
        BLS12381PrivateKey::from(&value).into()
    }
}

impl SecretKey for ClientSecretKey {
    type PublicKey = ClientPublicKey;

    fn generate() -> Self {
        let pair = BLS12381KeyPair::generate(&mut ThreadRng::default());
        pair.private().into()
    }

    fn decode_pem(encoded: &str) -> Option<ClientSecretKey> {
        let (label, bytes) = pem::decode_vec(encoded.as_bytes()).ok()?;
        (label == BLS12_381_PEM_LABEL && bytes.len() == 32)
            .then(|| ClientSecretKey(*array_ref!(bytes, 0, 32)))
    }

    fn encode_pem(&self) -> String {
        pem::encode_string(BLS12_381_PEM_LABEL, LineEnding::LF, &self.0).unwrap()
    }

    /// Sign a raw message.
    fn sign(&self, msg: &[u8]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: BLS12381PrivateKey = self.into();
        secret.sign(msg).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: &BLS12381PrivateKey = &self.into();
        let pubkey: BLS12381PublicKey = secret.into();
        pubkey.into()
    }
}
