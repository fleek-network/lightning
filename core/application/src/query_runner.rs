use draco_interfaces::application::SyncQueryRunnerInterface;
use draco_interfaces::identity::PeerId;
use draco_interfaces::types::NodeInfo;

#[derive(Clone)]
pub struct QueryRunner {}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_balance(&self, peer: &PeerId) -> u128 {
        todo!()
    }

    fn get_reputation(&self, peer: &PeerId) -> u128 {
        todo!()
    }

    fn get_relative_score(&self, n1: &PeerId, n2: &PeerId) -> u128 {
        todo!()
    }

    fn get_node_info(&self, id: &PeerId) -> Option<NodeInfo> {
        todo!()
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        todo!()
    }

    fn is_valid_node(&self, id: &PeerId) -> bool {
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
