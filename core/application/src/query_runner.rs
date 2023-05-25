use draco_interfaces::{application::SyncQueryRunnerInterface, identity::PeerId, types::NodeInfo};

#[derive(Clone)]
pub struct QueryRunner {}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_balance(&self, _peer: &PeerId) -> u128 {
        todo!()
    }

    fn get_reputation(&self, _peer: &PeerId) -> u128 {
        todo!()
    }

    fn get_relative_score(&self, _n1: &PeerId, _n2: &PeerId) -> u128 {
        todo!()
    }

    fn get_node_info(&self, _id: &PeerId) -> Option<NodeInfo> {
        todo!()
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        todo!()
    }

    fn is_valid_node(&self, _id: &PeerId) -> bool {
        todo!()
    }

    fn get_staking_amount(&self) -> u128 {
        todo!()
    }

    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        todo!()
    }

    fn get_committee_members(&self) -> Vec<draco_interfaces::identity::BlsPublicKey> {
        todo!()
    }
}
