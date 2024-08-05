use std::borrow::Borrow;

use anyhow::Result;
use atomo::SerdeBackend;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{MerklizeProvider, SimpleHasher, StateKey, StateProof, StateRootHash};

const SPARSE_MERKLE_PLACEHOLDER_HASH: [u8; 32] = *b"SPARSE_MERKLE_PLACEHOLDER_HASH__";
const LEAF_DOMAIN_SEPARATOR: &[u8] = b"JMT::LeafNode";
const INTERNAL_DOMAIN_SEPARATOR: &[u8] = b"JMT::IntrnalNode";

/// Return the ICS23 proof spec for the merklize provider, customized to the specific hasher.
pub fn ics23_proof_spec(hash_op: ics23::HashOp) -> ics23::ProofSpec {
    ics23::ProofSpec {
        leaf_spec: Some(ics23::LeafOp {
            hash: hash_op.into(),
            prehash_key: hash_op.into(),
            prehash_value: hash_op.into(),
            length: ics23::LengthOp::NoPrefix.into(),
            prefix: LEAF_DOMAIN_SEPARATOR.to_vec(),
        }),
        inner_spec: Some(ics23::InnerSpec {
            hash: hash_op.into(),
            child_order: vec![0, 1],
            min_prefix_length: INTERNAL_DOMAIN_SEPARATOR.len() as i32,
            max_prefix_length: INTERNAL_DOMAIN_SEPARATOR.len() as i32,
            child_size: 32,
            empty_child: SPARSE_MERKLE_PLACEHOLDER_HASH.to_vec(),
        }),
        min_depth: 0,
        max_depth: 64,
        prehash_key_before_comparison: true,
    }
}

/// Proof of a state value in the state tree.
/// This is a commitment proof that can be used to verify the existence or non-existence of a value
/// in the state tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmtStateProof(ics23::CommitmentProof);

impl JmtStateProof {
    /// Create a new `StateProof` with the given ics23 commitment proof.
    pub fn new(proof: ics23::CommitmentProof) -> Self {
        Self(proof)
    }
}

impl StateProof for JmtStateProof {
    /// Verify the membership of a key-value pair in the state tree.
    /// This is used to verify that a key exists in the state tree and has the given value. It
    /// encapsulates the serialization of the key and value, and relies on the ics23 crate to
    /// verify the proof from there.
    fn verify_membership<K, V, M: MerklizeProvider>(
        &self,
        table: impl AsRef<str>,
        key: impl Borrow<K>,
        value: impl Borrow<V>,
        root: StateRootHash,
    ) -> Result<()>
    where
        K: Serialize,
        V: Serialize,
    {
        let state_key = StateKey::new(table, M::Serde::serialize(&key.borrow()));
        let serialized_key = M::Serde::serialize(&state_key);
        let serialized_value = M::Serde::serialize(value.borrow());
        let verified = ics23::verify_membership::<ics23::HostFunctionsManager>(
            &self.0,
            &ics23_proof_spec(<M::Hasher as SimpleHasher>::ICS23_HASH_OP),
            &root.as_ref().to_vec(),
            &serialized_key,
            serialized_value.as_slice(),
        );
        if verified {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Membership proof verification failed"))
        }
    }

    /// Verify the non-membership of a key in the state tree.
    /// This is used to verify that a key does not exist in the state tree. It encapsulates the
    /// serialization of the key, and relies on the ics23 crate to verify the proof from there.
    fn verify_non_membership<K, M: MerklizeProvider>(
        self,
        table: impl AsRef<str>,
        key: impl Borrow<K>,
        root: StateRootHash,
    ) -> Result<()>
    where
        K: Serialize,
    {
        let state_key = StateKey::new(table, M::Serde::serialize(&key.borrow()));
        let serialized_key = M::Serde::serialize(&state_key);
        let verified = ics23::verify_non_membership::<ics23::HostFunctionsManager>(
            &self.0,
            &ics23_proof_spec(<M::Hasher as SimpleHasher>::ICS23_HASH_OP),
            &root.as_ref().to_vec(),
            &serialized_key,
        );
        if verified {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Non-membership proof verification failed"))
        }
    }
}

impl From<JmtStateProof> for ics23::CommitmentProof {
    fn from(proof: JmtStateProof) -> Self {
        proof.0
    }
}

impl From<ics23::CommitmentProof> for JmtStateProof {
    fn from(proof: ics23::CommitmentProof) -> Self {
        Self(proof)
    }
}

impl JsonSchema for JmtStateProof {
    fn schema_name() -> String {
        "JmtStateProof".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::JmtStateProof"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let key = Self(ics23::CommitmentProof::default());

        schemars::schema_for_value!(key).schema.into()
    }
}
