use std::array::TryFromSliceError;
use std::borrow::Borrow;
use std::fmt::Display;
use std::str::FromStr;

use arrayref::array_ref;
use fastcrypto::bls12381::min_sig::{
    BLS12381AggregateSignature,
    BLS12381PublicKey,
    BLS12381Signature,
};
use fastcrypto::encoding::{Base58, Encoding};
use fastcrypto::error::FastCryptoError;
use fastcrypto::traits::{AggregateAuthenticator, ToFromBytes};
use serde::{Deserialize, Serialize};

use crate::{base58_array, ConsensusPublicKey, ConsensusSignature, FleekCryptoError};

const CONSENSUS_AGGREGATE_SIGNATURE_SIZE: usize = 48;

/// Aggregate BLS signature for the consensus key.
///
/// This is a wrapper around `[fastcrypto::bls12381::min_sig::BLS12381AggregateSignature]` to
/// provide BLS aggregate signature functionality.
#[derive(Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ConsensusAggregateSignature(
    #[serde(with = "base58_array")] pub [u8; CONSENSUS_AGGREGATE_SIGNATURE_SIZE],
);

impl ConsensusAggregateSignature {
    /// Combine signatures into a single aggregated signature.
    pub fn aggregate<'a, K: Borrow<ConsensusSignature> + 'a, I: IntoIterator<Item = &'a K>>(
        signatures: I,
    ) -> Result<Self, FleekCryptoError> {
        let signatures: Vec<BLS12381Signature> = signatures
            .into_iter()
            .map(|s| s.borrow().try_into())
            .collect::<Result<Vec<_>, _>>()?;
        let agg_sig = BLS12381AggregateSignature::aggregate(&signatures)
            .map_err(|e| FleekCryptoError::AggregateSignaturesFailure(e.to_string()))?;

        agg_sig.try_into()
    }

    /// Verify this aggregate signature where the signatures are over different messages.
    pub fn verify(
        &self,
        pks: &[ConsensusPublicKey],
        messages: &[&[u8]],
    ) -> Result<bool, FleekCryptoError> {
        let pks: Vec<BLS12381PublicKey> = pks
            .iter()
            .map(|s| s.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        let sig: BLS12381AggregateSignature = (*self).try_into()?;
        let result = sig.verify_different_msg(&pks, messages);
        if result.is_err() {
            let err = result.err().unwrap();
            if err == FastCryptoError::InvalidSignature {
                return Ok(false);
            } else {
                return Err(FleekCryptoError::InvalidSignature(err.to_string()));
            }
        }
        Ok(true)
    }
}

/// Format the signature in base58 for display.
impl Display for ConsensusAggregateSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Base58::encode(self.0))
    }
}

/// Format the signature in base58 for debugging.
impl std::fmt::Debug for ConsensusAggregateSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            concat!(stringify!(ConsensusAggregateSignature), r#"("{}")"#),
            Base58::encode(self.0)
        )
    }
}

/// Parse a signature from a base58 string.
impl FromStr for ConsensusAggregateSignature {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = Base58::decode(s).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Signature not in base58 format.",
            )
        })?;

        if bytes.len() != CONSENSUS_AGGREGATE_SIGNATURE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid signature size.",
            ));
        }

        Ok(Self(*array_ref!(
            bytes,
            0,
            CONSENSUS_AGGREGATE_SIGNATURE_SIZE
        )))
    }
}

impl Default for ConsensusAggregateSignature {
    fn default() -> Self {
        Self([0u8; CONSENSUS_AGGREGATE_SIGNATURE_SIZE])
    }
}

impl TryFrom<BLS12381AggregateSignature> for ConsensusAggregateSignature {
    type Error = FleekCryptoError;

    fn try_from(signature: BLS12381AggregateSignature) -> Result<Self, FleekCryptoError> {
        Ok(Self(signature.as_bytes().try_into().map_err(
            |e: TryFromSliceError| FleekCryptoError::InvalidSignature(e.to_string()),
        )?))
    }
}

impl TryFrom<ConsensusAggregateSignature> for BLS12381AggregateSignature {
    type Error = FleekCryptoError;

    fn try_from(signature: ConsensusAggregateSignature) -> Result<Self, Self::Error> {
        BLS12381AggregateSignature::from_bytes(&signature.0)
            .map_err(|e| FleekCryptoError::InvalidAggregateSignature(e.to_string()))
    }
}

impl schemars::JsonSchema for ConsensusAggregateSignature {
    fn schema_name() -> String {
        "ConsensusAggregateSignature".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::ConsensusAggregateSignature"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let sig = ConsensusAggregateSignature::default();

        schemars::schema_for_value!(sig).schema.into()
    }
}
