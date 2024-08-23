use std::collections::BTreeMap;
use std::net::IpAddr;

use anyhow::Result;
use atomo::{DefaultSerdeBackend, StorageBackendConstructor, TableSelector, UpdatePerm};
use atomo_rocks::Options;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    ConsensusSecretKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
    TransactionSender,
    TransactionSignature,
};
use hp_fixed::unsigned::HpUfixed;
use ink_quill::ToDigest;
use lightning_application::config::{Config as AppConfig, StorageConfig};
use lightning_application::env::Env;
use lightning_application::network::Network;
use lightning_application::storage::AtomoStorage;
use lightning_interfaces::{
    IncrementalPutInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use lightning_types::{
    Blake3Hash,
    Block,
    CompressionAlgorithm,
    NodeIndex,
    NodePorts,
    ProofOfConsensus,
    ReputationMeasurements,
    Tokens,
    TransactionRequest,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use merklize::hashers::keccak::KeccakHasher;
use merklize::providers::jmt::{self, JmtStateProof};
use merklize::MerklizeProvider;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tempfile::TempDir;

use crate::reputation::generate_reputation_measurements;

/// A merklize provider that can be used to compare against as a baseline, since it does not
/// provide any merklization or state tree functionality.
pub struct BaselineMerklizeProvider {}

impl MerklizeProvider for BaselineMerklizeProvider {
    type Storage = AtomoStorage;
    type Serde = DefaultSerdeBackend;
    type Hasher = KeccakHasher;
    type Proof = jmt::JmtStateProof;

    const NAME: &'static str = "baseline";

    fn register_tables<C: StorageBackendConstructor>(
        builder: atomo::AtomoBuilder<C, Self::Serde>,
    ) -> atomo::AtomoBuilder<C, Self::Serde> {
        builder
    }

    fn update_state_tree<I>(
        _ctx: &TableSelector<Self::Storage, Self::Serde>,
        _batch: std::collections::HashMap<String, I>,
    ) -> Result<()>
    where
        I: Iterator<Item = (Box<[u8]>, atomo::batch::Operation)>,
    {
        // Do nothing.
        Ok(())
    }

    fn get_state_proof(
        _ctx: &TableSelector<Self::Storage, Self::Serde>,
        _table: &str,
        _serialized_key: Vec<u8>,
    ) -> Result<JmtStateProof> {
        unimplemented!("Baseline provider does not implement state tree")
    }

    fn get_state_root(
        _ctx: &TableSelector<Self::Storage, Self::Serde>,
    ) -> Result<merklize::StateRootHash> {
        unimplemented!("Baseline provider does not implement state tree")
    }

    fn verify_state_tree(
        _db: &mut atomo::Atomo<UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()> {
        unimplemented!("Baseline provider does not implement state tree")
    }

    fn clear_state_tree(
        _db: &mut atomo::Atomo<UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()> {
        unimplemented!("Baseline provider does not implement state tree")
    }
}

pub fn create_rocksdb_env<StateTree>(temp_dir: &TempDir) -> Env<StateTree>
where
    StateTree: MerklizeProvider<Storage = AtomoStorage, Serde = DefaultSerdeBackend>,
{
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    let db_path = temp_dir.path().join("db");

    let config = &AppConfig {
        network: Some(Network::TestnetStable),
        genesis_path: None,
        storage: StorageConfig::RocksDb,
        db_path: Some(db_path.try_into().unwrap()),
        db_options: None,
        dev: None,
    };
    let mut env = Env::<StateTree>::new(config, None).unwrap();
    env.apply_genesis_block(config).unwrap();

    env
}

pub fn new_simple_block() -> (Block, u64, EthAddress) {
    let stake_amount = 1000_u64;
    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let eth_address: EthAddress = public_key.into();
    let deposit_method = create_deposit_tx(Tokens::FLK, stake_amount);
    let deposit_req = create_update_request(deposit_method, &secret_key, 1);
    let deposit_tx = TransactionRequest::UpdateRequest(deposit_req);

    let block = Block {
        digest: [0; 32],
        transactions: vec![deposit_tx],
        sub_dag_index: 1,
        sub_dag_round: 1,
    };

    (block, stake_amount, eth_address)
}

#[allow(dead_code)]
// TODO(snormore): Re-add this and make use of it in the tests.
// pub fn query_simple_block<StateTree>(
//     env: &mut Env<StateTree>,
//     stake_amount: u64,
//     eth_address: EthAddress,
// ) where
//     StateTree: MerklizeProvider<Storage = AtomoStorage, Serde = DefaultSerdeBackend>,
// {
//     let account_table = env.state.resolve::<EthAddress, AccountInfo>("account");

//     let balance = env
//         .state
//         .run(|ctx| account_table.get(ctx).get(eth_address))
//         .map(|info| info.flk_balance)
//         .unwrap();
//     assert_eq!(balance, stake_amount.into());
// }

pub fn new_medium_block() -> (Block, u64, Vec<EthAddress>) {
    let num_txns = 50;
    let stake_amount = 1000_u64;
    let mut eth_addresses = Vec::new();
    let mut transactions = Vec::new();
    for _ in 0..num_txns {
        let secret_key = AccountOwnerSecretKey::generate();
        let public_key = secret_key.to_pk();
        let eth_address: EthAddress = public_key.into();
        eth_addresses.push(eth_address);
        let deposit_method = create_deposit_tx(Tokens::FLK, stake_amount);
        let deposit_req = create_update_request(deposit_method, &secret_key, 1);
        let deposit_tx = TransactionRequest::UpdateRequest(deposit_req);
        transactions.push(deposit_tx);
    }

    let block = Block {
        digest: [0; 32],
        transactions,
        sub_dag_index: 1,
        sub_dag_round: 1,
    };

    (block, stake_amount, eth_addresses)
}

// TODO(snormore): Re-add this and make use of it in the tests.
// #[allow(dead_code)]
// pub fn query_medium_block<StateTree>(
//     env: &mut Env<StateTree>,
//     stake_amount: u64,
//     eth_addresses: Vec<EthAddress>,
// ) where
//     StateTree: MerklizeProvider<Storage = AtomoStorage, Serde = DefaultSerdeBackend>,
// {
//     let account_table = env.state.resolve::<EthAddress, AccountInfo>("account");

//     for eth_address in eth_addresses {
//         let balance = env
//             .state
//             .run(|ctx| account_table.get(ctx).get(eth_address))
//             .map(|info| info.flk_balance)
//             .unwrap();
//         assert_eq!(balance, stake_amount.into());
//     }
// }

pub fn new_complex_block() -> (Block, u64, Vec<EthAddress>, Vec<NodePublicKey>) {
    let num_txns = 50;
    let stake_amount = 1000_u64;
    let mut rng: StdRng = SeedableRng::from_seed([0u8; 32]);

    let mut eth_addresses = Vec::with_capacity(num_txns);
    let mut node_public_keys = Vec::with_capacity(num_txns);
    let mut transactions = Vec::with_capacity(num_txns);
    for i in 0..num_txns {
        let secret_key = AccountOwnerSecretKey::generate();
        let public_key = secret_key.to_pk();
        let eth_address: EthAddress = public_key.into();
        eth_addresses.push(eth_address);
        let deposit_method = create_deposit_tx(Tokens::FLK, stake_amount);
        let deposit_req = create_update_request(deposit_method, &secret_key, 1);
        let deposit_tx = TransactionRequest::UpdateRequest(deposit_req);
        transactions.push(deposit_tx);

        let node_secret_key = NodeSecretKey::generate();
        let node_public_key = node_secret_key.to_pk();
        node_public_keys.push(node_public_key);
        let consensus_secret_key = ConsensusSecretKey::generate();
        let consensus_public_key = consensus_secret_key.to_pk();
        let domain: IpAddr = "127.0.0.1".parse().unwrap();
        let ports = NodePorts::default();
        let stake_method = create_stake_tx(
            stake_amount as u128,
            node_public_key,
            Some(consensus_public_key),
            Some(domain),
            Some(node_public_key),
            Some(domain),
            Some(ports),
        );
        let stake_req = create_update_request(stake_method, &secret_key, 2);
        let stake_tx = TransactionRequest::UpdateRequest(stake_req);
        transactions.push(stake_tx);

        let mut rep_measurements = BTreeMap::new();
        for j in 0..i {
            let m = generate_reputation_measurements(&mut rng, 1.0);
            rep_measurements.insert(j as NodeIndex, m);
        }
        if !rep_measurements.is_empty() {
            let submit_rep_method = create_submit_rep_measurements_tx(rep_measurements);
            let submit_rep_req = create_update_request_node(submit_rep_method, &node_secret_key, 1);
            let submit_rep_tx = TransactionRequest::UpdateRequest(submit_rep_req);
            transactions.push(submit_rep_tx);
        }
    }

    let block = Block {
        digest: [0; 32],
        transactions,
        sub_dag_index: 1,
        sub_dag_round: 1,
    };

    (block, stake_amount, eth_addresses, node_public_keys)
}

// #[allow(dead_code)]
// TODO(snormore): Re-add this and make use of it in the tests.
// pub fn query_complex_block<StateTree>(
//     env: &mut Env<StateTree>,
//     stake_amount: u64,
//     eth_addresses: Vec<EthAddress>,
//     node_public_keys: Vec<NodePublicKey>,
// ) where
//     StateTree: MerklizeProvider<Storage = AtomoStorage, Serde = DefaultSerdeBackend>,
// {
//     let account_table = env.state.resolve::<EthAddress, AccountInfo>("account");
//     let node_table = env.state.resolve::<NodeIndex, NodeInfo>("node");
//     let pubkey_to_index = env
//         .state
//         .resolve::<NodePublicKey, NodeIndex>("pub_key_to_index");

//     for eth_address in eth_addresses {
//         let balance = env
//             .state
//             .run(|ctx| account_table.get(ctx).get(eth_address))
//             .map(|info| info.flk_balance)
//             .unwrap();
//         assert_eq!(balance, 0_u64.into());
//     }
//     for pub_key in node_public_keys {
//         let node_index = env
//             .state
//             .run(|ctx| pubkey_to_index.get(ctx).get(pub_key))
//             .unwrap();

//         let node_info = env
//             .state
//             .run(|ctx| node_table.get(ctx).get(node_index))
//             .unwrap();
//         assert_eq!(pub_key, node_info.public_key);
//         let target_amount: HpUfixed<18> = stake_amount.into();
//         assert_eq!(target_amount, node_info.stake.staked);
//     }
// }

pub fn create_deposit_tx(token: Tokens, amount: u64) -> UpdateMethod {
    let amount: HpUfixed<18> = amount.into();
    UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token,
        amount,
    }
}

pub fn create_stake_tx(
    amount: u128,
    node_public_key: NodePublicKey,
    consensus_key: Option<ConsensusPublicKey>,
    node_domain: Option<IpAddr>,
    worker_public_key: Option<NodePublicKey>,
    worker_domain: Option<IpAddr>,
    ports: Option<NodePorts>,
) -> UpdateMethod {
    let amount: HpUfixed<18> = amount.into();
    UpdateMethod::Stake {
        amount,
        node_public_key,
        consensus_key,
        node_domain,
        worker_public_key,
        worker_domain,
        ports,
    }
}

pub fn create_submit_rep_measurements_tx(
    measurements: BTreeMap<NodeIndex, ReputationMeasurements>,
) -> UpdateMethod {
    UpdateMethod::SubmitReputationMeasurements { measurements }
}

#[allow(unused)]
pub fn create_unstake_tx(amount: u128, node_public_key: NodePublicKey) -> UpdateMethod {
    let amount: HpUfixed<18> = amount.into();
    UpdateMethod::Unstake {
        amount,
        node: node_public_key,
    }
}

pub fn create_update_request(
    method: UpdateMethod,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let public_key = secret_key.to_pk();
    let payload = UpdatePayload {
        sender: TransactionSender::AccountOwner(public_key.into()),
        chain_id: 59330,
        nonce,
        method,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        payload,
        signature: TransactionSignature::AccountOwner(signature),
    }
}

pub fn create_update_request_node(
    method: UpdateMethod,
    secret_key: &NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let public_key = secret_key.to_pk();
    let payload = UpdatePayload {
        sender: TransactionSender::NodeMain(public_key),
        chain_id: 59330,
        nonce,
        method,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        payload,
        signature: TransactionSignature::NodeMain(signature),
    }
}

pub struct DummyPutter {}

impl IncrementalPutInterface for DummyPutter {
    fn feed_proof(&mut self, _proof: &[u8]) -> Result<(), PutFeedProofError> {
        Ok(())
    }

    fn write(
        &mut self,
        _content: &[u8],
        _compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        Ok(())
    }

    fn is_finished(&self) -> bool {
        true
    }

    async fn finalize(self) -> Result<Blake3Hash, PutFinalizeError> {
        Ok([0; 32])
    }
}
