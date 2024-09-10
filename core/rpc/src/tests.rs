use std::collections::HashSet;

use fleek_crypto::{AccountOwnerSecretKey, EthAddress, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_application::env::ApplicationStateTree;
use lightning_interfaces::prelude::*;
use lightning_test_utils::e2e::TestNetworkBuilder;
use lightning_types::{
    AccountInfo,
    Event,
    GenesisAccount,
    GenesisNodeServed,
    Metadata,
    NodeServed,
    ProtocolParams,
    Staking,
    StateProofKey,
    StateProofValue,
    TotalServed,
    Value,
};
use lightning_utils::application::QueryRunnerExt;
use merklize::StateProof;

use crate::api::{AdminApiClient, FleekApiClient};

#[tokio::test]
async fn test_rpc_get_flk_balance() {
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let eth_address: EthAddress = owner_secret_key.to_pk().into();
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.account.push(GenesisAccount {
                public_key: owner_secret_key.to_pk().into(),
                flk_balance: HpUfixed::<18>::from(1_000_u32),
                stables_balance: 100,
                bandwidth_balance: 100,
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_flk_balance(&node.rpc_client().unwrap(), eth_address, None)
        .await
        .unwrap();
    assert_eq!(HpUfixed::<18>::from(1_000_u32), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_reputation() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].reputation = Some(46);
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response =
        FleekApiClient::get_reputation(&node.rpc_client().unwrap(), node_public_key, None)
            .await
            .unwrap();
    assert_eq!(Some(46), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_staked() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].stake = Staking {
                staked: 1_000_u32.into(),
                stake_locked_until: 365,
                locked: 0_u32.into(),
                locked_until: 0,
            };
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response = FleekApiClient::get_staked(&node.rpc_client().unwrap(), node_public_key, None)
        .await
        .unwrap();
    assert_eq!(HpUfixed::<18>::from(1_000_u32), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_stables_balance() {
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let eth_address: EthAddress = owner_secret_key.to_pk().into();
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.account.push(GenesisAccount {
                public_key: owner_secret_key.to_pk().into(),
                flk_balance: HpUfixed::<18>::from(1_000_u32),
                stables_balance: 100,
                bandwidth_balance: 100,
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response =
        FleekApiClient::get_stables_balance(&node.rpc_client().unwrap(), eth_address, None)
            .await
            .unwrap();
    assert_eq!(HpUfixed::<6>::from(1_00_u32), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_stake_locked_until() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].stake = Staking {
                staked: 1_000_u32.into(),
                stake_locked_until: 365,
                locked: 0_u32.into(),
                locked_until: 0,
            };
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response =
        FleekApiClient::get_stake_locked_until(&node.rpc_client().unwrap(), node_public_key, None)
            .await
            .unwrap();
    assert_eq!(365, response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_locked_time() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].stake = Staking {
                staked: 1_000_u32.into(),
                stake_locked_until: 365,
                locked: 0_u32.into(),
                locked_until: 2,
            };
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response =
        FleekApiClient::get_locked_time(&node.rpc_client().unwrap(), node_public_key, None)
            .await
            .unwrap();
    assert_eq!(2, response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_locked() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].stake = Staking {
                staked: 1_000_u32.into(),
                stake_locked_until: 365,
                locked: 500_u32.into(),
                locked_until: 2,
            };
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response = FleekApiClient::get_locked(&node.rpc_client().unwrap(), node_public_key, None)
        .await
        .unwrap();
    assert_eq!(HpUfixed::<18>::from(500_u32), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_bandwidth_balance() {
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let eth_address: EthAddress = owner_secret_key.to_pk().into();
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.account.push(GenesisAccount {
                public_key: owner_secret_key.to_pk().into(),
                flk_balance: 0u64.into(),
                stables_balance: 0,
                bandwidth_balance: 10_000,
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response =
        FleekApiClient::get_bandwidth_balance(&node.rpc_client().unwrap(), eth_address, None)
            .await
            .unwrap();
    assert_eq!(10_000, response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_node_info() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response =
        FleekApiClient::get_node_info(&node.rpc_client().unwrap(), node_public_key, None)
            .await
            .unwrap();
    let node_info = node
        .app
        .sync_query()
        .get_node_info(&0, |node_info| node_info)
        .unwrap();
    assert_eq!(Some(node_info), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_staking_amount() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_staking_amount(&node.rpc_client().unwrap())
        .await
        .unwrap();
    assert_eq!(node.app.sync_query().get_staking_amount(), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_committee_members() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_committee_members(&node.rpc_client().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(node.app.sync_query().get_committee_members(), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_epoch() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_epoch(&node.rpc_client().unwrap())
        .await
        .unwrap();
    assert_eq!(node.app.sync_query().get_current_epoch(), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_epoch_info() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_epoch_info(&node.rpc_client().unwrap())
        .await
        .unwrap();
    assert_eq!(node.app.sync_query().get_epoch_info(), response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_total_supply() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_total_supply(&node.rpc_client().unwrap(), None)
        .await
        .unwrap();
    let total_supply = match node.app.sync_query().get_metadata(&Metadata::TotalSupply) {
        Some(Value::HpUfixed(s)) => s,
        _ => panic!("TotalSupply is set genesis and should never be empty"),
    };
    assert_eq!(total_supply, response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_year_start_supply() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_year_start_supply(&node.rpc_client().unwrap(), None)
        .await
        .unwrap();
    let supply_year_start = match node
        .app
        .sync_query()
        .get_metadata(&Metadata::SupplyYearStart)
    {
        Some(Value::HpUfixed(s)) => s,
        _ => panic!("SupplyYearStart is set genesis and should never be empty"),
    };
    assert_eq!(supply_year_start, response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_protocol_fund_address() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_protocol_fund_address(&node.rpc_client().unwrap())
        .await
        .unwrap();
    let protocol_account = match node
        .app
        .sync_query()
        .get_metadata(&Metadata::ProtocolFundAddress)
    {
        Some(Value::AccountPublicKey(s)) => s,
        _ => panic!("AccountPublicKey is set genesis and should never be empty"),
    };
    assert_eq!(protocol_account, response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_protocol_param_lock_time() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response =
        FleekApiClient::get_protocol_params(&node.rpc_client().unwrap(), ProtocolParams::LockTime)
            .await
            .unwrap();
    assert_eq!(
        node.app
            .sync_query()
            .get_protocol_param(&ProtocolParams::LockTime)
            .unwrap(),
        response
    );

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_total_served() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.total_served.insert(
                0,
                TotalServed {
                    served: vec![1000],
                    reward_pool: 1_000_u32.into(),
                }
                .into(),
            );
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_total_served(&node.rpc_client().unwrap(), 0)
        .await
        .unwrap();
    assert_eq!(
        TotalServed {
            served: vec![1000],
            reward_pool: 1_000_u32.into(),
        },
        response
    );

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_node_served() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].current_epoch_served = Some(GenesisNodeServed {
                served: vec![1000],
                ..Default::default()
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response =
        FleekApiClient::get_node_served(&node.rpc_client().unwrap(), node_public_key, None)
            .await
            .unwrap();
    assert_eq!(
        NodeServed {
            served: vec![1000],
            ..Default::default()
        },
        response
    );

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_is_valid_node() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].current_epoch_served = Some(GenesisNodeServed {
                served: vec![1000],
                ..Default::default()
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let node_public_key = node.keystore.get_ed25519_pk();

    let response = FleekApiClient::is_valid_node(&node.rpc_client().unwrap(), node_public_key)
        .await
        .unwrap();
    assert!(response);

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_node_registry() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].genesis_committee = false;
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_node_registry(&node.rpc_client().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(response.len(), 3);

    let node_public_keys = network
        .node_by_id
        .values()
        .map(|n| n.keystore.get_ed25519_pk())
        .collect::<HashSet<_>>();

    assert!(node_public_keys.contains(&response[0].public_key));
    assert!(node_public_keys.contains(&response[1].public_key));
    assert!(node_public_keys.contains(&response[2].public_key));

    network.shutdown().await;
}

#[tokio::test]
async fn test_admin_seq() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.node_info[0].genesis_committee = false;
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    // Should fail because we are not authenticated.
    assert!(
        AdminApiClient::ping(&node.rpc_client().unwrap())
            .await
            .is_err()
    );

    // Should work because we are authenticated.
    let client = node.rpc_admin_client().await.unwrap();
    for _ in 0..5 {
        AdminApiClient::ping(&client).await.unwrap();
    }

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_events() {
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.account.push(GenesisAccount {
                public_key: owner_secret_key.to_pk().into(),
                flk_balance: HpUfixed::<18>::from(1_000_u32),
                stables_balance: 100,
                bandwidth_balance: 100,
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let sender = node.rpc.event_tx();

    let client = node.rpc_ws_client().await.unwrap();
    let mut sub = FleekApiClient::handle_subscription(&client, None)
        .await
        .unwrap();
    let event = Event::transfer(
        EthAddress::from([0; 20]),
        EthAddress::from([1; 20]),
        EthAddress::from([2; 20]),
        HpUfixed::<18>::from(10_u16),
    );
    sender.send(vec![event.clone()]);
    assert_eq!(
        sub.next().await.expect("An event from the sub").unwrap(),
        event
    );

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_state_root() {
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.account.push(GenesisAccount {
                public_key: owner_secret_key.to_pk().into(),
                flk_balance: 1000u64.into(),
                stables_balance: 0,
                bandwidth_balance: 0,
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();

    let response = FleekApiClient::get_state_root(&node.rpc_client().unwrap(), None)
        .await
        .unwrap();
    let root_hash = response.to_string();
    assert_eq!(root_hash.len(), 64);
    assert!(root_hash.chars().all(|c| c.is_ascii_hexdigit()));

    network.shutdown().await;
}

#[tokio::test]
async fn test_rpc_get_state_proof() {
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_eth_address: EthAddress = owner_secret_key.to_pk().into();
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .with_genesis_mutator(move |genesis| {
            genesis.account.push(GenesisAccount {
                public_key: owner_secret_key.to_pk().into(),
                flk_balance: 1000u64.into(),
                stables_balance: 0,
                bandwidth_balance: 0,
            });
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0).unwrap();
    let client = node.rpc_client().unwrap();

    let state_key = StateProofKey::Accounts(owner_eth_address);
    let (value, proof) =
        FleekApiClient::get_state_proof(&client, StateProofKey::Accounts(owner_eth_address), None)
            .await
            .unwrap();

    assert!(value.is_some());
    let value = value.unwrap();
    assert_eq!(
        value.clone(),
        StateProofValue::Accounts(AccountInfo {
            flk_balance: 1000u64.into(),
            stables_balance: HpUfixed::zero(),
            bandwidth_balance: 0,
            nonce: 0,
        })
    );

    // Verify proof.
    let root_hash = FleekApiClient::get_state_root(&client, None).await.unwrap();
    proof
        .verify_membership::<_, _, ApplicationStateTree>(
            state_key.table(),
            owner_eth_address,
            AccountInfo {
                flk_balance: 1000u64.into(),
                stables_balance: HpUfixed::zero(),
                bandwidth_balance: 0,
                nonce: 0,
            },
            root_hash,
        )
        .unwrap();

    network.shutdown().await;
}
