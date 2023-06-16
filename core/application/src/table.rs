use std::{any::Any, cell::RefCell, hash::Hash};

use atomo::{KeyIterator, SerdeBackend, TableRef as AtomoTableRef, TableSelector};
use draco_interfaces::types::{
    ExecutionError, ProofOfConsensus, ProofOfMisbehavior, UpdateRequest,
};
use fleek_crypto::{AccountOwnerPublicKey, NodePublicKey};
use serde::{de::DeserializeOwned, Serialize};

pub trait Backend {
    type Ref<K: Eq + Hash + Send + Serialize + DeserializeOwned
     + 'static, V: Clone + Send + Serialize + DeserializeOwned + 'static>: TableRef<K, V>;

    fn get_table_reference<
        K: Eq + Hash + Send + Serialize + DeserializeOwned,
        V: Clone + Send + Serialize + DeserializeOwned,
    >(
        &self,
        id: &str,
    ) -> Self::Ref<K, V>;
    /// This function takes in the Transaction and verifies the Signature matches the Sender. It
    /// also checks the nonce of the sender and makes sure it is equal to the account nonce + 1,
    /// to prevent replay attacks and enforce ordering
    fn verify_transaction(&self, txn: &UpdateRequest) -> Result<(), ExecutionError>;
    /// Takes in a zk Proof Of Delivery and returns true if valid
    fn verify_proof_of_delivery(
        &self,
        client: &AccountOwnerPublicKey,
        provider: &NodePublicKey,
        commodity: &u128,
        service_id: &u32,
        proof: (),
    ) -> bool;
    /// Takes in a zk Proof Of Consensus and returns true if valid
    fn verify_proof_of_consensus(&self, proof: ProofOfConsensus) -> bool;
    /// Takes in a zk Proof Of Misbehavior and returns true if valid
    fn verify_proof_of_misbehavior(&self, proof: ProofOfMisbehavior) -> bool;
}

pub trait TableRef<K, V> {
    fn set(&self, key: K, value: V);
    fn get(&self, key: &K) -> Option<V>;
    fn keys(&self) -> KeyIterator<'_, K>;
    fn remove(&self, key: &K);
}

pub struct StateTables<'selector, S: SerdeBackend> {
    pub table_selector: &'selector TableSelector<S>,
}

impl<'selector, S: SerdeBackend> Backend for StateTables<'selector, S> {
    type Ref<
        K: Eq + Hash + Send + Serialize + DeserializeOwned + 'static,
        V: Clone + Send + Serialize + DeserializeOwned + 'static,
    > = AtomoTable<'selector, K, V, S>;

    fn get_table_reference<
        K: Eq + Hash + Send + Serialize + DeserializeOwned,
        V: Clone + Send + Serialize + DeserializeOwned,
    >(
        &self,
        id: &str,
    ) -> Self::Ref<K, V> {
        AtomoTable(RefCell::new(self.table_selector.get_table(id)))
    }

    fn verify_transaction(&self, _txn: &UpdateRequest) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn verify_proof_of_delivery(
        &self,
        _client: &AccountOwnerPublicKey,
        _provider: &NodePublicKey,
        _commodity: &u128,
        _service_id: &u32,
        _proof: (),
    ) -> bool {
        true
    }

    fn verify_proof_of_consensus(&self, _proof: ProofOfConsensus) -> bool {
        true
    }

    fn verify_proof_of_misbehavior(&self, _proof: ProofOfMisbehavior) -> bool {
        true
    }
}

pub struct AtomoTable<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + 'static,
    V: Serialize + DeserializeOwned + 'static,
    S: SerdeBackend,
>(RefCell<AtomoTableRef<'selector, K, V, S>>);

impl<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any + Clone,
    S: SerdeBackend,
> TableRef<K, V> for AtomoTable<'selector, K, V, S>
{
    fn set(&self, key: K, value: V) {
        self.0.borrow_mut().insert(key, value);
    }

    fn get(&self, key: &K) -> Option<V> {
        self.0.borrow_mut().get(key)
    }

    fn keys(&self) -> KeyIterator<'_, K> {
        todo!()
    }

    fn remove(&self, key: &K) {
        self.0.borrow_mut().remove(key)
    }
}
