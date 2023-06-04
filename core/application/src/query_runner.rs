use draco_interfaces::{application::SyncQueryRunnerInterface, types::NodeInfo};

#[derive(Clone)]
pub struct QueryRunner {}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_balance(&self, _client: &fleek_crypto::ClientPublicKey) -> u128 {
        todo!()
    }

    fn get_reputation(&self, _node: &fleek_crypto::NodePublicKey) -> u128 {
        todo!()
    }

    fn get_relative_score(
        &self,
        _n1: &fleek_crypto::NodePublicKey,
        _n2: &fleek_crypto::NodePublicKey,
    ) -> u128 {
        todo!()
    }

    fn get_node_info(&self, _id: &fleek_crypto::NodePublicKey) -> Option<NodeInfo> {
        todo!()
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        todo!()
    }

    fn is_valid_node(&self, _id: &fleek_crypto::NodePublicKey) -> bool {
        todo!()
    }

    fn get_staking_amount(&self) -> u128 {
        todo!()
    }

    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        todo!()
    }

    fn get_committee_members(&self) -> Vec<fleek_crypto::NodePublicKey> {
        todo!()
    }
}
