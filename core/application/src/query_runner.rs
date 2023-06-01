use draco_interfaces::{application::SyncQueryRunnerInterface, types::NodeInfo};

#[derive(Clone)]
pub struct QueryRunner {}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_balance(&self, client: &fleek_crypto::ClientPublicKey) -> u128 {
        todo!()
    }

    fn get_reputation(&self, node: &fleek_crypto::NodePublicKey) -> u128 {
        todo!()
    }

    fn get_relative_score(
        &self,
        n1: &fleek_crypto::NodePublicKey,
        n2: &fleek_crypto::NodePublicKey,
    ) -> u128 {
        todo!()
    }

    fn get_node_info(&self, id: &fleek_crypto::NodePublicKey) -> Option<NodeInfo> {
        todo!()
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        todo!()
    }

    fn is_valid_node(&self, id: &fleek_crypto::NodePublicKey) -> bool {
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
