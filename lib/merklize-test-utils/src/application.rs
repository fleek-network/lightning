use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use atomo::{
    AtomoBuilder,
    DefaultSerdeBackend,
    StorageBackend,
    StorageBackendConstructor,
    TableSelector,
    UpdatePerm,
};
use atomo_rocks::Options;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ClientPublicKey,
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
use lightning_application::storage::{AtomoStorage, AtomoStorageBuilder};
use lightning_interfaces::{
    IncrementalPutInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use lightning_test_utils::reputation::generate_reputation_measurements;
use lightning_types::{
    AccountInfo,
    Blake3Hash,
    Block,
    Committee,
    CommodityTypes,
    CompressionAlgorithm,
    Epoch,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodePorts,
    NodeServed,
    ProofOfConsensus,
    ProtocolParams,
    ReportedReputationMeasurements,
    ReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    Tokens,
    TotalServed,
    TransactionRequest,
    TxHash,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
    Value,
};
use merklize::hashers::keccak::KeccakHasher;
use merklize::providers::jmt::{self, JmtStateProof};
use merklize::{MerklizeProvider, MerklizedAtomo};
use rand::rngs::StdRng;
use rand::SeedableRng;
use tempfile::TempDir;

/// A merklize provider that can be used to compare against as a baseline, since it does not
/// provide any merklization or state tree functionality.
pub struct BaselineMerklizeProvider {}

impl MerklizeProvider for BaselineMerklizeProvider {
    type Storage = AtomoStorage;
    type Serde = DefaultSerdeBackend;
    type Hasher = KeccakHasher;
    type Proof = jmt::JmtStateProof;

    fn with_tables<C: StorageBackendConstructor>(
        builder: atomo::AtomoBuilder<C, Self::Serde>,
    ) -> atomo::AtomoBuilder<C, Self::Serde> {
        builder
    }

    fn apply_state_tree_changes(_ctx: &TableSelector<Self::Storage, Self::Serde>) -> Result<()> {
        // Do nothing.
        Ok(())
    }

    fn get_state_proof(
        _ctx: &TableSelector<Self::Storage, Self::Serde>,
        _table: &str,
        _serialized_key: Vec<u8>,
    ) -> Result<JmtStateProof> {
        unimplemented!("Baseline context does not implement state proofs.")
    }

    fn get_state_root(
        _ctx: &TableSelector<Self::Storage, Self::Serde>,
    ) -> Result<merklize::StateRootHash> {
        unimplemented!("Baseline context does not implement state roots.")
    }
}

pub fn create_rocksdb_env<M>(temp_dir: &TempDir) -> Env<UpdatePerm, AtomoStorage, M>
where
    M: MerklizeProvider<Storage = AtomoStorage, Serde = DefaultSerdeBackend>,
{
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    let storage = AtomoStorageBuilder::new(Some(temp_dir.path())).with_options(options);

    create_merklize_app_env(storage, PathBuf::from(temp_dir.path()))
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
pub fn query_simple_block<B, M>(
    env: &mut Env<UpdatePerm, B, M>,
    stake_amount: u64,
    eth_address: EthAddress,
) where
    B: StorageBackend,
    M: MerklizeProvider<Storage = B, Serde = DefaultSerdeBackend>,
{
    let account_table = env.inner.resolve::<EthAddress, AccountInfo>("account");

    let balance = env
        .inner
        .run(|ctx| account_table.get(ctx).get(eth_address))
        .map(|info| info.flk_balance)
        .unwrap();
    assert_eq!(balance, stake_amount.into());
}

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

#[allow(dead_code)]
pub fn query_medium_block<B, M>(
    env: &mut Env<UpdatePerm, B, M>,
    stake_amount: u64,
    eth_addresses: Vec<EthAddress>,
) where
    B: StorageBackend,
    M: MerklizeProvider<Storage = B, Serde = DefaultSerdeBackend>,
{
    let account_table = env.inner.resolve::<EthAddress, AccountInfo>("account");

    for eth_address in eth_addresses {
        let balance = env
            .inner
            .run(|ctx| account_table.get(ctx).get(eth_address))
            .map(|info| info.flk_balance)
            .unwrap();
        assert_eq!(balance, stake_amount.into());
    }
}

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

#[allow(dead_code)]
pub fn query_complex_block<B, M>(
    env: &mut Env<UpdatePerm, B, M>,
    stake_amount: u64,
    eth_addresses: Vec<EthAddress>,
    node_public_keys: Vec<NodePublicKey>,
) where
    B: StorageBackend,
    M: MerklizeProvider<Storage = B, Serde = DefaultSerdeBackend>,
{
    let account_table = env.inner.resolve::<EthAddress, AccountInfo>("account");
    let node_table = env.inner.resolve::<NodeIndex, NodeInfo>("node");
    let pubkey_to_index = env
        .inner
        .resolve::<NodePublicKey, NodeIndex>("pub_key_to_index");

    for eth_address in eth_addresses {
        let balance = env
            .inner
            .run(|ctx| account_table.get(ctx).get(eth_address))
            .map(|info| info.flk_balance)
            .unwrap();
        assert_eq!(balance, 0_u64.into());
    }
    for pub_key in node_public_keys {
        let node_index = env
            .inner
            .run(|ctx| pubkey_to_index.get(ctx).get(pub_key))
            .unwrap();

        let node_info = env
            .inner
            .run(|ctx| node_table.get(ctx).get(node_index))
            .unwrap();
        assert_eq!(pub_key, node_info.public_key);
        let target_amount: HpUfixed<18> = stake_amount.into();
        assert_eq!(target_amount, node_info.stake.staked);
    }
}

pub fn create_merklize_app_env<C: StorageBackendConstructor, M>(
    storage: C,
    db_path: PathBuf,
) -> Env<UpdatePerm, <C as StorageBackendConstructor>::Storage, M>
where
    M: MerklizeProvider<Storage = C::Storage, Serde = DefaultSerdeBackend>,
{
    let db = M::with_tables(
        AtomoBuilder::new(storage)
            .with_table::<Metadata, Value>("metadata")
            .with_table::<EthAddress, AccountInfo>("account")
            .with_table::<ClientPublicKey, EthAddress>("client_keys")
            .with_table::<NodeIndex, NodeInfo>("node")
            .with_table::<ConsensusPublicKey, NodeIndex>("consensus_key_to_index")
            .with_table::<NodePublicKey, NodeIndex>("pub_key_to_index")
            .with_table::<(NodeIndex, NodeIndex), Duration>("latencies")
            .with_table::<Epoch, Committee>("committee")
            .with_table::<ServiceId, Service>("service")
            .with_table::<ProtocolParams, u128>("parameter")
            .with_table::<NodeIndex, Vec<ReportedReputationMeasurements>>("rep_measurements")
            .with_table::<NodeIndex, u8>("rep_scores")
            .with_table::<NodeIndex, u8>("submitted_rep_measurements")
            .with_table::<NodeIndex, NodeServed>("current_epoch_served")
            .with_table::<NodeIndex, NodeServed>("last_epoch_served")
            .with_table::<Epoch, TotalServed>("total_served")
            .with_table::<CommodityTypes, HpUfixed<6>>("commodity_prices")
            .with_table::<ServiceId, ServiceRevenue>("service_revenue")
            .with_table::<TxHash, ()>("executed_digests")
            .with_table::<NodeIndex, u8>("uptime")
            .with_table::<Blake3Hash, BTreeSet<NodeIndex>>("uri_to_node")
            .with_table::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_uri")
            .enable_iter("consensus_key_to_index")
            .enable_iter("pub_key_to_index")
            .enable_iter("current_epoch_served")
            .enable_iter("rep_measurements")
            .enable_iter("submitted_rep_measurements")
            .enable_iter("rep_scores")
            .enable_iter("latencies")
            .enable_iter("node")
            .enable_iter("executed_digests")
            .enable_iter("uptime")
            .enable_iter("service_revenue")
            .enable_iter("uri_to_node")
            .enable_iter("node_to_uri"),
    )
    .build()
    .unwrap();

    let db = MerklizedAtomo::new(db);

    let mut env = Env { inner: db };
    let app_config = AppConfig {
        network: Some(Network::TestnetStable),
        genesis_path: None,
        storage: StorageConfig::RocksDb,
        db_path: Some(db_path.try_into().unwrap()),
        db_options: None,
        dev: None,
    };
    let _ = env.apply_genesis_block(&app_config).unwrap();
    env
}

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
