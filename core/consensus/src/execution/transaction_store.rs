use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};

use super::parcel::{AuthenticStampedParcel, Digest};
use crate::consensus::PubSubMsg;

pub struct ParcelWrapper<T: BroadcastEventInterface<PubSubMsg>> {
    pub(crate) parcel: Option<Parcel>,
    pub(crate) attestations: Option<HashSet<NodeIndex>>,
    pub(crate) parcel_event: Option<T>,
    pub(crate) attestation_events: Option<HashMap<NodeIndex, T>>,
}

#[derive(Clone, Debug)]
pub struct Parcel {
    pub inner: AuthenticStampedParcel,
    // The originator of this parcel.
    pub originator: NodeIndex,
    // This is the digest from the broadcast message that contained the parcel.
    // At the moment, both broadcast and consensus use [u8; 32] for the digests, but we should
    // treat them as different types nonetheless.
    pub message_digest: Option<BroadcastDigest>,
}

pub struct TransactionStore<T: BroadcastEventInterface<PubSubMsg>> {
    ring: Vec<HashMap<Digest, ParcelWrapper<T>>>,
    pointer: usize,
}

impl<T: BroadcastEventInterface<PubSubMsg>> TransactionStore<T> {
    // Returns the parcel for the given digest, if it exists.
    // If the parcel does not exist for the current epoch, we will check for parcels from the
    // previous epoch.
    pub fn get_parcel(&self, digest: &Digest) -> Option<&Parcel> {
        self.ring[self.pointer]
            .get(digest)
            .and_then(|wrapper| wrapper.parcel.as_ref())
            .or(self.ring[self.prev_pointer()]
                .get(digest)
                .and_then(|wrapper| wrapper.parcel.as_ref()))
    }

    // Returns the attestations for the given digest, if they exist.
    // If the attestations do not exist for the current epoch, we will check for attestations from
    // the previous epoch.
    pub fn get_attestations(&self, digest: &Digest) -> Option<&HashSet<NodeIndex>> {
        self.ring[self.pointer]
            .get(digest)
            .and_then(|wrapper| wrapper.attestations.as_ref())
            .or(self.ring[self.prev_pointer()]
                .get(digest)
                .and_then(|wrapper| wrapper.attestations.as_ref()))
    }

    // Store a parcel from the current epoch.
    pub fn store_parcel(
        &mut self,
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: Option<BroadcastDigest>,
    ) {
        self.store_parcel_internal(self.pointer, parcel, originator, message_digest, None);
    }

    // Store a parcel from the next epoch. These parcels will be verified once the epoch changes.
    pub fn store_pending_parcel(
        &mut self,
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: Option<BroadcastDigest>,
        event: T,
    ) {
        self.store_parcel_internal(
            self.next_pointer(),
            parcel,
            originator,
            message_digest,
            Some(event),
        );
    }

    // Store an attestation from the current epoch.
    pub fn store_attestation(&mut self, digest: Digest, node_index: NodeIndex) {
        self.store_attestation_internal(self.pointer, digest, node_index, None);
    }

    // Stores an attestation from the next epoch. After the epoch change we have to verify if this
    // attestation originated from a committee member.
    pub fn store_pending_attestation(&mut self, digest: Digest, node_index: NodeIndex, event: T) {
        self.store_attestation_internal(self.next_pointer(), digest, node_index, Some(event));
    }

    /// Change the committee.
    ///
    /// When the committee is changed for across epoch eras and epochs, the parcels from the current
    /// become the parcels from the previous.
    ///
    /// The parcels from the previous are garbage collected, and the parcels from the next becom
    /// ethe parcels from the current after validating them.
    pub fn change_committee(&mut self, committee: &[NodeIndex]) {
        // Verify that the parcels/attestations from the next epoch were send by committee members.
        // Remove invalid parcels/attestations from the hash map.
        let next_pointer = self.next_pointer();
        self.ring[next_pointer].retain(|_, wrapper| {
            let parcel_event = wrapper.parcel_event.take();
            let attn_events = wrapper.attestation_events.take();

            if let Some(parcel) = &mut wrapper.parcel {
                if !committee.contains(&parcel.originator) {
                    wrapper.parcel = None;
                    if let Some(event) = parcel_event {
                        event.mark_invalid_sender();
                    }
                } else if let Some(event) = parcel_event {
                    event.propagate();
                }
            }

            if let Some(attns) = &mut wrapper.attestations {
                let (valid_attn, invalid_attn): (HashSet<_>, HashSet<_>) = attns
                    .iter()
                    .copied()
                    .partition(|node_index| committee.contains(node_index));
                if let Some(mut events) = attn_events {
                    invalid_attn.into_iter().for_each(|node_index| {
                        if let Some(event) = events.remove(&node_index) {
                            event.mark_invalid_sender();
                        }
                    });
                    valid_attn.iter().for_each(|node_index| {
                        if let Some(event) = events.remove(node_index) {
                            event.propagate();
                        }
                    });
                }

                if valid_attn.is_empty() {
                    wrapper.attestations = None;
                } else {
                    wrapper.attestations = Some(valid_attn);
                }
            }

            wrapper.parcel.is_some() || wrapper.attestations.is_some()
        });

        let prev_pointer = self.prev_pointer();
        // Clear previous epoch map, because this will become the next epoch map
        self.ring[prev_pointer].clear();
        self.pointer = self.next_pointer();
    }

    // Store a parcel and optionally provide the digest of the broadcast message that delivered
    // this parcel.
    // If we already store the parcel, we won't overwrite it again. If we already store the parcel,
    // but don't store the broadcast message yet, we will insert the message.
    fn store_parcel_internal(
        &mut self,
        pointer: usize,
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: Option<BroadcastDigest>,
        event: Option<T>,
    ) {
        let digest = parcel.to_digest();
        // We are explicitly matching the entry here instead of using `and_modify` together with
        // `or_insert` in order to avoid cloning the parcel.
        match self.ring[pointer].entry(digest) {
            Entry::Vacant(entry) => {
                entry.insert(ParcelWrapper {
                    parcel: Some(Parcel {
                        inner: parcel,
                        originator,
                        message_digest,
                    }),
                    attestations: None,
                    parcel_event: event,
                    attestation_events: None,
                });
            },
            Entry::Occupied(mut entry) => match &mut entry.get_mut().parcel {
                Some(parcel) => {
                    if parcel.message_digest.is_none() {
                        parcel.message_digest = message_digest;
                    }
                },
                None => {
                    entry.get_mut().parcel = Some(Parcel {
                        inner: parcel,
                        originator,
                        message_digest,
                    });
                },
            },
        }
    }

    fn store_attestation_internal(
        &mut self,
        pointer: usize,
        digest: Digest,
        node_index: NodeIndex,
        event: Option<T>,
    ) {
        self.ring[pointer]
            .entry(digest)
            .and_modify(|wrapper| match &mut wrapper.attestations {
                Some(attestations) => {
                    attestations.insert(node_index);
                },
                None => {
                    wrapper.attestations = Some(HashSet::from([node_index]));
                },
            })
            .or_insert(ParcelWrapper {
                parcel: None,
                attestations: Some(HashSet::from([node_index])),
                parcel_event: None,
                attestation_events: event.map(|t| std::iter::once((node_index, t)).collect()),
            });
    }

    fn next_pointer(&self) -> usize {
        (self.pointer + 1) % self.ring.len()
    }

    fn prev_pointer(&self) -> usize {
        if self.pointer == 0 {
            self.ring.len() - 1
        } else {
            self.pointer - 1
        }
    }
}

impl<T: BroadcastEventInterface<PubSubMsg>> Default for TransactionStore<T> {
    fn default() -> Self {
        Self {
            ring: vec![
                HashMap::with_capacity(100),
                HashMap::with_capacity(100),
                HashMap::with_capacity(100),
            ],
            pointer: 1,
        }
    }
}
