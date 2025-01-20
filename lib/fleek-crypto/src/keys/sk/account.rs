use arrayref::array_ref;
use fastcrypto::hash::Keccak256;
use fastcrypto::secp256k1::{Secp256k1KeyPair, Secp256k1PrivateKey, Secp256k1PublicKey};
use fastcrypto::traits::{KeyPair, RecoverableSigner, ToFromBytes};
use rand::rngs::ThreadRng;
use sec1::der::EncodePem;
use sec1::pkcs8::ObjectIdentifier;
use sec1::{pem, EcParameters, EcPrivateKey, LineEnding};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::keys::AccountOwnerPublicKey;
use crate::{PublicKey, SecretKey};

#[derive(Clone, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct AccountOwnerSecretKey([u8; 32]);

impl std::fmt::Debug for AccountOwnerSecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AccountOwnerSecretKeyOf")
            .field(&self.to_pk())
            .finish()
    }
}

impl From<Secp256k1PrivateKey> for AccountOwnerSecretKey {
    fn from(value: Secp256k1PrivateKey) -> Self {
        let bytes = value.as_ref();
        AccountOwnerSecretKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&AccountOwnerSecretKey> for Secp256k1PrivateKey {
    fn from(value: &AccountOwnerSecretKey) -> Self {
        Secp256k1PrivateKey::from_bytes(&value.0).unwrap()
    }
}

impl AccountOwnerSecretKey {
    pub fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl SecretKey for AccountOwnerSecretKey {
    type PublicKey = AccountOwnerPublicKey;

    fn generate() -> Self {
        let pair = Secp256k1KeyPair::generate(&mut ThreadRng::default());
        pair.private().into()
    }

    fn decode_pem(encoded: &str) -> Option<Self> {
        let (label, der_bytes) = pem::decode_vec(encoded.as_bytes()).ok()?;
        if label != "EC PRIVATE KEY" {
            return None;
        }
        let info = EcPrivateKey::try_from(der_bytes.as_ref()).ok()?;
        Some(Self(*array_ref!(info.private_key, 0, 32)))
    }

    fn encode_pem(&self) -> String {
        let pubkey = self.to_pk();
        EcPrivateKey {
            private_key: &self.0,
            parameters: Some(EcParameters::NamedCurve(
                // http://oid-info.com/get/1.3.132.0.10
                ObjectIdentifier::new("1.3.132.0.10").unwrap(),
            )),
            public_key: Some(&pubkey.0),
        }
        .to_pem(LineEnding::LF)
        .unwrap()
    }

    /// Sign a raw message, hashed with Keccak256.
    fn sign(&self, msg: &[u8]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: Secp256k1PrivateKey = self.into();
        let pair: Secp256k1KeyPair = secret.into();
        pair.sign_recoverable_with_hash::<Keccak256>(msg).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: &Secp256k1PrivateKey = &self.into();
        let pubkey: Secp256k1PublicKey = secret.into();
        pubkey.into()
    }
}
