use std::collections::hash_map::Entry;
use std::collections::HashMap;

use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};
use lightning_interfaces::ToDigest;

use super::transaction_store::{Parcel, ParcelWrapper};
use crate::execution::{AuthenticStampedParcel, Digest};

pub struct RingBuffer {
    ring: Vec<HashMap<Digest, ParcelWrapper>>,
    pointer: usize,
}

impl RingBuffer {
    pub fn new() -> Self {
        RingBuffer {
            ring: vec![HashMap::with_capacity(100); 3],
            pointer: 1,
        }
    }

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

    // Returns the attestations for the given digest, if they exists.
    // If the attestations do not exist for the current epoch, we will check for attestations from
    // the previous epoch.
    pub fn get_attestations(&self, digest: &Digest) -> Option<&Vec<NodeIndex>> {
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
        self.store_parcel_internal(self.pointer, parcel, originator, message_digest);
    }

    // Store a parcel from the next epoch. These parcels will be verified once the epoch changes.
    pub fn store_pending_parcel(
        &mut self,
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: Option<BroadcastDigest>,
    ) {
        self.store_parcel_internal(self.next_pointer(), parcel, originator, message_digest);
    }

    // Store an attestation from the current epoch.
    pub fn add_attestation(&mut self, digest: Digest, node_index: NodeIndex) {
        self.add_attestation_internal(self.pointer, digest, node_index);
    }

    // Stores an attestation from the next epoch. After the epoch change we have to verify if this
    // attestation originated from a committee member.
    pub fn add_pending_attestation(&mut self, digest: Digest, node_index: NodeIndex) {
        self.add_attestation_internal(self.next_pointer(), digest, node_index);
    }

    // When the epoch changes, the parcels from the current epochs become
    // the parcels from the previous epoch.
    // The parcels from the previous epoch are garbage collected, and the parcels from
    // the next epoch become the parcels from the current epoch (after validating them).
    pub fn change_epoch(&mut self, committee: &[NodeIndex]) {
        // Verify that the parcels/attestations from the next epoch were send by committee members.
        // Remove invalid parcels/attestations from the hash map.
        let next_pointer = self.next_pointer();
        self.ring[next_pointer].retain(|_, wrapper| {
            if let Some(parcel) = &mut wrapper.parcel {
                if !committee.contains(&parcel.originator) {
                    wrapper.parcel = None;
                }
            }

            if let Some(attns) = &mut wrapper.attestations {
                let valid_attn: Vec<NodeIndex> = attns
                    .iter()
                    .copied()
                    .filter(|node_index| committee.contains(node_index))
                    .collect();
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

    fn add_attestation_internal(&mut self, pointer: usize, digest: Digest, node_index: NodeIndex) {
        self.ring[pointer]
            .entry(digest)
            .and_modify(|wrapper| match &mut wrapper.attestations {
                Some(attestations) => {
                    attestations.push(node_index);
                },
                None => {
                    wrapper.attestations = Some(vec![node_index]);
                },
            })
            .or_insert(ParcelWrapper {
                parcel: None,
                attestations: Some(vec![node_index]),
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
