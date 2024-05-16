use std::collections::BTreeSet;

use fleek_crypto::NodePublicKey;
use lightning_types::Epoch;

pub struct StateChain {
    since: Epoch,
    epoch_change: Vec<EpochChange>,
}

struct EpochChange {
    checkpoint: Checkpoint,
    node_added: Vec<NodePublicKey>, // 30
    node_removed: Vec<usize>,       // 2
    node_merkle_proof: Vec<u8>,     // 64
}

struct Checkpoint {
    state_root_hash: [u8; 32],
    signer_nodes: BigFlagSet,    //
    signature: BlsAggrSignature, // 96byte
}

struct CheckpointAttestation {
    signature: BlsSignature,
}

impl Checkpoint {
    fn verify(&self, active_nodes: &[NodePublicKey]) -> bool {
        let mut public_keys = Vec::new();
        for index in self.signer_nodes {
            public_keys.push(&active_nodes[index as usize]);
        }
        if public_keys.len() < 2 * active_nodes.len() / 3 {
            return false;
        }
        let mut payload = Vec::<u8>::new();
        payload.extend(&self.epoch_number.to_be_bytes());
        payload.extend(&self.state_root_hash);
        verify_bls_aggr_signatures(&public_keys, &self.signature, &payload)
    }
}

// 1000 epoch
// 20 + 1000 * 28 = 28020 nodes -> 3502 byte
// SUM { ceil((20 + i * 28) / 8) }

impl StateChain {
    fn verify(&self, current_epoch: Epoch, active_nodes: &[NodePublicKey]) -> bool {
        let mut current_epoch = current_epoch;
        let mut active_nodes: BTreeSet<NodePublicKey> = active_nodes.iter().copied().collect();
        for change in self.epoch_change {
            if change.checkpoint.epoch_number != current_epoch {
                current_epoch += 1;
                return false;
            }

            // verify the checkpoint against current nodes we know.
            if !change.checkpoint.verify(&active_nodes) {
                return false;
            }
            // update the active nodes.
            for node in &change.node_added {
                active_nodes.insert(*node);
            }
            for node in &change.node_removed {
                active_nodes.remove(node);
            }
            // verify the current set of nodes is included in the state root hash.
            if !verify_merkle_inclusion(
                &change.checkpoint.state_root_hash,
                &change.node_merkle_proof,
                ser(&active_nodes),
            ) {
                return false;
            }
        }
        return true;
    }
}

pub trait CheckpointInterface {
    fn get_checpoint_at(&self, epoch: Option<Epoch>);
    fn get_state_chain_since(&self, epoch: Option<Epoch>);
}
