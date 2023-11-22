use std::collections::BTreeMap;
use std::time::SystemTime;
use std::vec;

use affair::Socket;
use anyhow::{anyhow, Result};
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::application::ExecutionEngineSocket;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    Block,
    BlockExecutionResponse,
    DeliveryAcknowledgment,
    Epoch,
    HandshakePorts,
    NodePorts,
    ProofOfConsensus,
    ReputationMeasurements,
    Tokens,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::{partial, ApplicationInterface, SyncQueryRunnerInterface, ToDigest};
use tokio::test;

use crate::app::Application;
use crate::config::{Config, Mode, StorageConfig};
use crate::genesis::{Genesis, GenesisNode};
use crate::query_runner::QueryRunner;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
});

pub struct Params {
    epoch_time: Option<u64>,
    max_inflation: Option<u16>,
    protocol_share: Option<u16>,
    node_share: Option<u16>,
    service_builder_share: Option<u16>,
    max_boost: Option<u16>,
    supply_at_genesis: Option<u64>,
}

#[derive(Clone)]
struct GenesisCommitteeKeystore {
    _owner_secret_key: AccountOwnerSecretKey,
    node_secret_key: NodeSecretKey,
    _consensus_secret_key: ConsensusSecretKey,
    _worker_secret_key: NodeSecretKey,
}

fn get_genesis_committee(num_members: usize) -> (Vec<GenesisNode>, Vec<GenesisCommitteeKeystore>) {
    let mut keystore = Vec::new();
    let mut committee = Vec::new();
    (0..num_members as u16).for_each(|i| {
        let node_secret_key = NodeSecretKey::generate();
        let consensus_secret_key = ConsensusSecretKey::generate();
        let owner_secret_key = AccountOwnerSecretKey::generate();
        add_to_committee(
            &mut committee,
            &mut keystore,
            node_secret_key,
            consensus_secret_key,
            owner_secret_key,
            i,
        )
    });
    (committee, keystore)
}

fn add_to_committee(
    committee: &mut Vec<GenesisNode>,
    keystore: &mut Vec<GenesisCommitteeKeystore>,
    node_secret_key: NodeSecretKey,
    consensus_secret_key: ConsensusSecretKey,
    owner_secret_key: AccountOwnerSecretKey,
    index: u16,
) {
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_public_key = owner_secret_key.to_pk();
    committee.push(GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 8000 + index,
            worker: 9000 + index,
            mempool: 7000 + index,
            rpc: 6000 + index,
            pool: 5000 + index,
            dht: 4000 + index,
            pinger: 2000 + index,
            handshake: HandshakePorts {
                http: 5000 + index,
                webrtc: 6000 + index,
                webtransport: 7000 + index,
            },
        },
        None,
        true,
    ));
    keystore.push(GenesisCommitteeKeystore {
        _owner_secret_key: owner_secret_key,
        _worker_secret_key: node_secret_key.clone(),
        node_secret_key,
        _consensus_secret_key: consensus_secret_key,
    });
}

fn get_new_committee(
    query_runner: &QueryRunner,
    committee: &[GenesisNode],
    keystore: &[GenesisCommitteeKeystore],
) -> (Vec<GenesisNode>, Vec<GenesisCommitteeKeystore>) {
    let mut new_committee = Vec::new();
    let mut new_keystore = Vec::new();
    let committee_members = query_runner.get_committee_members();
    for node in committee_members {
        let index = committee
            .iter()
            .enumerate()
            .find_map(|(index, c)| {
                if c.primary_public_key == node {
                    Some(index)
                } else {
                    None
                }
            })
            .expect("Committe member was not found in genesis Committee");
        new_committee.push(committee[index].clone());
        new_keystore.push(keystore[index].clone());
    }
    (new_committee, new_keystore)
}

// Init the app and return the execution engine socket that would go to narwhal and the query socket
// that could go to anyone
fn init_app(config: Option<Config>) -> (ExecutionEngineSocket, QueryRunner) {
    let config = config.or(Some(Config {
        genesis: None,
        mode: Mode::Dev,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));
    let app = Application::<TestBinding>::init(config.unwrap(), Default::default()).unwrap();

    (app.transaction_executor(), app.sync_query())
}

fn init_app_with_params(
    params: Params,
    committee: Option<Vec<GenesisNode>>,
) -> (ExecutionEngineSocket, QueryRunner) {
    let mut genesis = Genesis::load().expect("Failed to load genesis from file.");

    if let Some(committee) = committee {
        genesis.node_info = committee;
    }

    genesis.epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    if let Some(epoch_time) = params.epoch_time {
        genesis.epoch_time = epoch_time;
    }

    if let Some(max_inflation) = params.max_inflation {
        genesis.max_inflation = max_inflation;
    }

    if let Some(protocol_share) = params.protocol_share {
        genesis.protocol_share = protocol_share;
    }

    if let Some(node_share) = params.node_share {
        genesis.node_share = node_share;
    }

    if let Some(service_builder_share) = params.service_builder_share {
        genesis.service_builder_share = service_builder_share;
    }

    if let Some(max_boost) = params.max_boost {
        genesis.max_boost = max_boost;
    }

    if let Some(supply_at_genesis) = params.supply_at_genesis {
        genesis.supply_at_genesis = supply_at_genesis;
    }
    let config = Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    };

    init_app(Some(config))
}

// Helper function that performs an epoch change.
// In order to submit the `ChangeEpoch` transactions, this function needs access to the committee's
// private keys. These are supplied in the `committee_keystore`.
async fn simple_epoch_change(
    epoch: Epoch,
    committee_keystore: &Vec<GenesisCommitteeKeystore>,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    query_runner: &QueryRunner,
) -> Result<()> {
    let required_signals = 2 * committee_keystore.len() / 3 + 1;
    // make call epoch change for 2/3rd committe members
    for (index, node) in committee_keystore.iter().enumerate().take(required_signals) {
        let nonce = query_runner
            .get_node_info(&node.node_secret_key.to_pk())
            .unwrap()
            .nonce
            + 1;
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch },
            &node.node_secret_key,
            nonce,
        );
        let res = run_transaction(vec![req.into()], update_socket).await?;
        // check epoch change
        if index == required_signals - 1 {
            assert!(res.change_epoch);
        }
    }
    Ok(())
}

// Helper method to get a transaction update request from a node.
// Passing the private key around like this should only be done for
// testing.
fn get_update_request_node(
    method: UpdateMethod,
    secret_key: &NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let payload = UpdatePayload {
        sender: secret_key.to_pk().into(),
        nonce,
        method,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        signature: signature.into(),
        payload,
    }
}
// Passing the private key around like this should only be done for
// testing.
fn get_update_request_account(
    method: UpdateMethod,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let payload = UpdatePayload {
        sender: secret_key.to_pk().into(),
        nonce,
        method,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        signature: signature.into(),
        payload,
    }
}

fn get_genesis() -> (Genesis, Vec<GenesisNode>) {
    let genesis = Genesis::load().unwrap();

    (genesis.clone(), genesis.node_info)
}
// Helper methods for tests
// Passing the private key around like this should only be done for
// testing.
fn pod_request(
    secret_key: &NodeSecretKey,
    commodity: u128,
    service_id: u32,
    nonce: u64,
) -> UpdateRequest {
    get_update_request_node(
        UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
            commodity,  // units of data served
            service_id, // service 0 serving bandwidth
            proofs: vec![DeliveryAcknowledgment],
            metadata: None,
        },
        secret_key,
        nonce,
    )
}

async fn run_transaction(
    requests: Vec<TransactionRequest>,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) -> Result<BlockExecutionResponse> {
    let res = update_socket
        .run(Block {
            transactions: requests,
            digest: [0; 32],
        })
        .await
        .map_err(|r| anyhow!(format!("{r:?}")))?;
    Ok(res)
}

async fn deposit(
    amount: HpUfixed<18>,
    token: Tokens,
    secret_key: &AccountOwnerSecretKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    nonce: u64,
) {
    // Deposit some FLK into account 1
    let req = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token,
            amount,
        },
        secret_key,
        nonce,
    );
    run_transaction(vec![req.into()], update_socket)
        .await
        .unwrap();
}

async fn stake(
    amount: HpUfixed<18>,
    node_public_key: NodePublicKey,
    consensus_key: ConsensusPublicKey,
    secret_key: &AccountOwnerSecretKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    nonce: u64,
) {
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount,
            node_public_key,
            consensus_key: Some(consensus_key),
            node_domain: Some("127.0.0.1".parse().unwrap()),
            worker_public_key: Some([0; 32].into()),
            worker_domain: Some("127.0.0.1".parse().unwrap()),
            ports: Some(NodePorts::default()),
        },
        secret_key,
        nonce,
    );
    if let TransactionResponse::Revert(error) = run_transaction(vec![update.into()], update_socket)
        .await
        .unwrap()
        .txn_receipts[0]
        .response
        .clone()
    {
        panic!("Stake reverted: {error:?}");
    }
}

#[test]
async fn test_genesis() {
    // Init application + get the query and update socket
    let (_, query_runner) = init_app(None);
    // Get the genesis paramaters plus the initial committee
    let (genesis, genesis_committee) = get_genesis();
    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let balance = query_runner.get_staked(&node.primary_public_key);
        assert_eq!(HpUfixed::<18>::from(genesis.min_stake), balance);
    }
}

#[test]
async fn test_supply_across_epoch() {
    let (mut committee, mut keystore) = get_genesis_committee(4);

    let epoch_time = 100;
    let max_inflation = 10;
    let protocol_part = 10;
    let node_part = 80;
    let service_part = 10;
    let boost = 4;
    let supply_at_genesis = 1000000;
    let (update_socket, query_runner) = init_app_with_params(
        Params {
            epoch_time: Some(epoch_time),
            max_inflation: Some(max_inflation),
            protocol_share: Some(protocol_part),
            node_share: Some(node_part),
            service_builder_share: Some(service_part),
            max_boost: Some(boost),
            supply_at_genesis: Some(supply_at_genesis),
        },
        Some(committee.clone()),
    );

    // get params for emission calculations
    let percentage_divisor: HpUfixed<18> = 100_u16.into();
    let supply_at_year_start: HpUfixed<18> = supply_at_genesis.into();
    let inflation: HpUfixed<18> = HpUfixed::from(max_inflation) / &percentage_divisor;
    let node_share = HpUfixed::from(node_part) / &percentage_divisor;
    let protocol_share = HpUfixed::from(protocol_part) / &percentage_divisor;
    let service_share = HpUfixed::from(service_part) / &percentage_divisor;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();
    let consensus_secret_key = ConsensusSecretKey::generate();

    // deposit FLK tokens and stake it
    deposit(
        10_000_u64.into(),
        Tokens::FLK,
        &owner_secret_key,
        &update_socket,
        1,
    )
    .await;
    stake(
        10_000_u64.into(),
        node_secret_key.to_pk(),
        consensus_secret_key.to_pk(),
        &owner_secret_key,
        &update_socket,
        2,
    )
    .await;
    // the index should be increment of whatever the size of genesis committee is, 5 in this case
    add_to_committee(
        &mut committee,
        &mut keystore,
        node_secret_key.clone(),
        consensus_secret_key.clone(),
        owner_secret_key.clone(),
        5,
    );
    // every epoch supply increase similar for simplicity of the test
    let _node_1_usd = 0.1 * 10000_f64;

    // calculate emissions per unit
    let emissions_per_epoch: HpUfixed<18> = (&inflation * &supply_at_year_start) / &365.0.into();

    let mut supply = supply_at_year_start;

    // 365 epoch changes to see if the current supply and year start suppply are ok
    for i in 0..365 {
        // add at least one transaction per epoch, so reward pool is not zero
        let nonce = query_runner
            .get_node_info(&node_secret_key.to_pk())
            .unwrap()
            .nonce;
        let pod_10 = pod_request(&node_secret_key, 10000, 0, nonce + 1);
        // run the delivery ack transaction
        if let TransactionResponse::Revert(error) =
            run_transaction(vec![pod_10.into()], &update_socket)
                .await
                .unwrap()
                .txn_receipts[0]
                .response
                .clone()
        {
            panic!("{error:?}");
        }

        // We have to submit uptime measurements to make sure nodes aren't set to
        // participating=false in the next epoch.
        // This is obviously tedious. The alternative is to deactivate the removal of offline nodes
        // for testing.
        for node in &keystore {
            let mut map = BTreeMap::new();
            let measurements = ReputationMeasurements {
                latency: None,
                interactions: None,
                inbound_bandwidth: None,
                outbound_bandwidth: None,
                bytes_received: None,
                bytes_sent: None,
                uptime: Some(100),
                hops: None,
            };
            for peer in &keystore {
                if node.node_secret_key == peer.node_secret_key {
                    continue;
                }
                let public_key = peer.node_secret_key.to_pk();
                let peer_index = query_runner.pubkey_to_index(public_key).unwrap();
                map.insert(peer_index, measurements.clone());
            }
            let nonce = query_runner
                .get_node_info(&node.node_secret_key.to_pk())
                .unwrap()
                .nonce
                + 1;
            let req = get_update_request_node(
                UpdateMethod::SubmitReputationMeasurements { measurements: map },
                &node.node_secret_key,
                nonce,
            );
            if let TransactionResponse::Revert(error) =
                run_transaction(vec![req.into()], &update_socket)
                    .await
                    .unwrap()
                    .txn_receipts[0]
                    .response
                    .clone()
            {
                panic!("{error:?}");
            }
        }

        let (_, new_keystore) = get_new_committee(&query_runner, &committee, &keystore);
        if let Err(err) = simple_epoch_change(i, &new_keystore, &update_socket, &query_runner).await
        {
            panic!("error while changing epoch, {err}");
        }

        let supply_increase = &emissions_per_epoch * &node_share
            + &emissions_per_epoch * &protocol_share
            + &emissions_per_epoch * &service_share;
        let total_supply = query_runner.get_total_supply();
        supply += supply_increase;
        assert_eq!(total_supply, supply);
        if i == 364 {
            // the supply_year_start should update
            let supply_year_start = query_runner.get_year_start_supply();
            assert_eq!(total_supply, supply_year_start);
        }
    }
}
