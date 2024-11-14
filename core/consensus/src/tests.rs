use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};
use narwhal_types::{Batch, BatchAPI, Transaction};
use rand::Rng;
use sui_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};

use crate::consensus::PubSubMsg;
use crate::execution::parcel::{AuthenticStampedParcel, Digest};
use crate::execution::transaction_store::TransactionStore;

fn generate_random_tx(length: usize) -> Transaction {
    let mut rng = rand::thread_rng();
    (0..length).map(|_| rng.gen_range(0..255)).collect()
}

fn generate_random_batch(num_txns: usize, tx_length: usize) -> Batch {
    let config =
        ProtocolConfig::get_for_version_if_supported(ProtocolVersion::new(12), Chain::Unknown)
            .unwrap();
    let txns = (0..num_txns)
        .map(|_| generate_random_tx(tx_length))
        .collect();
    Batch::new(txns, &config)
}

fn generate_random_parcel(
    num_batches: usize,
    num_txns: usize,
    tx_length: usize,
    last_executed: Option<Digest>,
) -> AuthenticStampedParcel {
    let transactions = (0..num_batches)
        .flat_map(|_| {
            generate_random_batch(num_txns, tx_length)
                .transactions()
                .clone()
        })
        .collect();
    AuthenticStampedParcel {
        transactions,
        last_executed: last_executed.unwrap_or([0; 32]),
        epoch: 1,
        sub_dag_index: 0,
        sub_dag_round: 0,
    }
}

#[test]
fn test_to_digest_eq() {
    let parcel = generate_random_parcel(5, 4, 10, None);
    assert_eq!(parcel.to_digest(), parcel.to_digest());
}

#[test]
fn test_to_digest_ne() {
    let parcel1 = generate_random_parcel(5, 4, 10, None);
    let parcel2 = generate_random_parcel(5, 6, 10, None);
    let digest1 = parcel1.to_digest();
    let digest2 = parcel2.to_digest();
    assert_ne!(digest1, digest2);
}

#[test]
fn test_to_digest_reorder_batches() {
    let parcel1 = generate_random_parcel(5, 4, 10, None);
    let mut parcel2 = parcel1.clone();
    let temp_batch = parcel2.transactions[1].clone();
    parcel2.transactions[1] = parcel2.transactions[0].clone();
    parcel2.transactions[0] = temp_batch;
    // Payloads with transactions in dif order should not have the same hash
    assert_ne!(parcel1.to_digest(), parcel2.to_digest());
}

#[test]
fn test_ring_buffer_store_get_parcel() {
    let mut ring_buffer = TransactionStore::<Event>::default();
    let parcel = generate_random_parcel(2, 1, 2, None);
    let digest = parcel.to_digest();
    ring_buffer.store_parcel(parcel, 99, None);

    assert_eq!(
        ring_buffer.get_parcel(&digest).unwrap().inner.to_digest(),
        digest
    );
}

#[test]
fn test_ring_buffer_store_get_att() {
    let mut ring_buffer = TransactionStore::<Event>::default();
    let parcel = generate_random_parcel(2, 1, 2, None);
    let digest = parcel.to_digest();
    ring_buffer.store_attestation(digest, 4);
    ring_buffer.store_attestation(digest, 5);

    let attns = ring_buffer.get_attestations(&digest).unwrap();
    assert_eq!(attns.len(), 2);
    assert!(attns.contains(&4));
    assert!(attns.contains(&5));
}

#[test]
fn test_ring_buffer_epoch_change() {
    let mut ring_buffer = TransactionStore::<Event>::default();
    let parcel = generate_random_parcel(2, 1, 2, None);
    let digest = parcel.to_digest();
    let event = Event {
        originator: 1,
        message: None,
        digest,
    };
    ring_buffer.store_pending_parcel(parcel, 2, None, event);

    // Since the added parcel is pending, it should not be returned here.
    assert!(ring_buffer.get_parcel(&digest).is_none());

    // Since the pending parcel was sent from a committee member (2), it be marked as valid after
    // the epoch change.
    let new_committee = vec![0, 1, 2, 3];
    ring_buffer.change_committee(&new_committee);
    assert_eq!(
        ring_buffer.get_parcel(&digest).unwrap().inner.to_digest(),
        digest
    );

    // We keep track of parcels from the previous epoch, so the parcel should still be available.
    let new_committee = vec![0, 5, 4, 3];
    ring_buffer.change_committee(&new_committee);

    assert_eq!(
        ring_buffer.get_parcel(&digest).unwrap().inner.to_digest(),
        digest
    );

    // But now it should be garbage collected
    let new_committee = vec![0, 2, 1, 3];
    ring_buffer.change_committee(&new_committee);

    assert!(ring_buffer.get_parcel(&digest).is_none());
}

#[test]
fn test_ring_buffer_invalid_parcel() {
    let mut ring_buffer = TransactionStore::<Event>::default();
    let parcel = generate_random_parcel(2, 1, 2, None);
    let digest = parcel.to_digest();
    let event = Event {
        originator: 1,
        message: None,
        digest,
    };
    ring_buffer.store_pending_parcel(parcel, 8, None, event);

    // Since the added parcel is pending, it should not be returned here.
    assert!(ring_buffer.get_parcel(&digest).is_none());

    // The parcel originator (8) is not on the new committee, so the parcel should be marked as
    // invalid and removed.
    let new_committee = vec![0, 1, 2, 3];
    ring_buffer.change_committee(&new_committee);
    assert!(ring_buffer.get_parcel(&digest).is_none());
}

struct Event {
    originator: NodeIndex,
    message: Option<PubSubMsg>,
    digest: BroadcastDigest,
}

impl BroadcastEventInterface<PubSubMsg> for Event {
    fn originator(&self) -> NodeIndex {
        self.originator
    }

    fn take(&mut self) -> Option<PubSubMsg> {
        self.message.take()
    }

    fn propagate(self) {}

    fn mark_invalid_sender(self) {}

    fn get_digest(&self) -> Digest {
        self.digest
    }
}
