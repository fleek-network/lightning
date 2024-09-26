use fleek_crypto::SecretKey;
use lightning_interfaces::types::{ContentUpdate, ExecutionData, ExecutionError};
use lightning_interfaces::SyncQueryRunnerInterface;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_submit_content_registry_update() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // When: each node sends an update to register some content.
    let mut expected_records = Vec::new();
    for (list_idx, ck) in keystore.iter().enumerate() {
        let uri = [list_idx as u8; 32];
        expected_records.push((ck.node_secret_key.clone(), uri));

        let updates = vec![ContentUpdate { uri, remove: false }];
        let update = prepare_content_registry_update(updates, &ck.node_secret_key, 1);
        expect_tx_success(update, &update_socket, ExecutionData::None).await;
    }

    // Then: registry indicates that the nodes are providing the correct content.
    for (sk, cid) in expected_records.iter() {
        let index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();
        let cids = content_registry(&query_runner, &index);
        assert_eq!(cids, vec![*cid]);
    }
    // Then: all providers are accounted for.
    for (sk, cid) in expected_records.iter() {
        let index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();
        let providers = uri_to_providers(&query_runner, cid);
        assert_eq!(providers, vec![index]);
    }

    // When: send an update to remove the records.
    for (sk, uri) in expected_records.iter() {
        let updates = vec![ContentUpdate {
            uri: *uri,
            remove: true,
        }];
        let update = prepare_content_registry_update(updates, sk, 2);
        expect_tx_success(update, &update_socket, ExecutionData::None).await;
    }

    // Then: records are removed.
    for (sk, uri) in expected_records.iter() {
        let providers = uri_to_providers(&query_runner, uri);
        assert!(providers.is_empty());
        let index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();
        let cids = content_registry(&query_runner, &index);
        assert!(cids.is_empty());
    }
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_providers_per_cid() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [69u8; 32];
    // Given: providers for that cid.
    let providers = keystore.clone();

    // When: all nodes send an update for that cid.
    for ck in &providers {
        let updates = vec![ContentUpdate { uri, remove: false }];
        let update = prepare_content_registry_update(updates, &ck.node_secret_key, 1);
        expect_tx_success(update, &update_socket, ExecutionData::None).await;
    }

    // Then: state shows that they all provide the cid.
    let expected_providers = providers
        .into_iter()
        .map(|ck| {
            query_runner
                .pubkey_to_index(&ck.node_secret_key.to_pk())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let providers = uri_to_providers(&query_runner, &uri);
    assert_eq!(providers, expected_providers);
    // Then: cid is in every node's registry.
    for provider in providers {
        let uris = content_registry(&query_runner, &provider);
        assert_eq!(vec![uri], uris);
    }

    // When: send an update to remove the records.
    let providers = keystore.clone();
    for ck in &providers {
        let updates = vec![ContentUpdate { uri, remove: true }];
        let update = prepare_content_registry_update(updates, &ck.node_secret_key, 2);
        expect_tx_success(update, &update_socket, ExecutionData::None).await;
    }

    // Then: records are removed.
    let providers = uri_to_providers(&query_runner, &uri);
    assert!(providers.is_empty());
    for node in keystore {
        let index = query_runner
            .pubkey_to_index(&node.node_secret_key.to_pk())
            .unwrap();
        let cids = content_registry(&query_runner, &index);
        assert!(cids.is_empty());
    }
}

#[tokio::test]
async fn test_submit_content_registry_update_mix_of_add_and_remove_updates() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Given: multiple cids.
    let mut uris = Vec::new();
    for i in 0..6 {
        let uri = [i as u8; 32];
        uris.push(uri);
    }

    // Given: a node provides the given cids.
    let mut updates = Vec::new();
    for uri in &uris {
        updates.push(ContentUpdate {
            uri: *uri,
            remove: false,
        });
    }

    let update = prepare_content_registry_update(updates.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // When: we remove some records and add some new ones.
    let mut removed = Vec::new();
    let mut updates = updates
        .into_iter()
        .step_by(2)
        .map(|mut update| {
            update.remove = true;
            removed.push(update.uri);
            update
        })
        .collect::<Vec<_>>();

    for i in 6..9 {
        let uri = [i as u8; 32];
        uris.push(uri);
        updates.push(ContentUpdate { uri, remove: false });
    }

    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Then: the state is updated appropriately.
    // Check that removed is a subset so that both branches below can be asserted.
    assert!(!removed.is_empty());
    assert!(removed.len() < uris.len());
    let node = query_runner
        .pubkey_to_index(&keystore[0].node_secret_key.to_pk())
        .unwrap();
    for uri in &uris {
        if removed.contains(uri) {
            let providers = uri_to_providers(&query_runner, uri);
            assert!(providers.is_empty());
        } else {
            let providers = uri_to_providers(&query_runner, uri);
            assert_eq!(providers, vec![node]);
        }
    }
    let expected_uris = uris
        .iter()
        .copied()
        .filter(|uri| !removed.contains(uri))
        .collect::<Vec<_>>();
    let uris_for_node = content_registry(&query_runner, &node);
    assert_eq!(expected_uris, uris_for_node);
}

#[tokio::test]
async fn test_submit_content_registry_update_too_many_updates_in_transaction() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 2;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a big list of updates.
    let uri = [69u8; 32];
    let mut updates = Vec::new();
    for _ in 0..101u32 {
        updates.push(ContentUpdate { uri, remove: false });
    }

    // When: we submit the update transaction.
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);

    // Then: the transaction reverts because the list went past the limit.
    expect_tx_revert(update, &update_socket, ExecutionError::TooManyUpdates).await;
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_cids_per_provider() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Given: multiple cids that some nodes will provide.
    let mut uris = Vec::new();
    let mut updates = Vec::new();
    for i in 0..6 {
        let uri = [i as u8; 32];
        uris.push(uri);
        updates.push(ContentUpdate { uri, remove: false });
    }

    // When: each node sends an update to register the cids.
    let update = prepare_content_registry_update(updates.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    let update = prepare_content_registry_update(updates, &keystore[1].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Then: nodes show up as providers for the cids.
    let node1 = query_runner
        .pubkey_to_index(&keystore[0].node_secret_key.to_pk())
        .unwrap();
    let node2 = query_runner
        .pubkey_to_index(&keystore[1].node_secret_key.to_pk())
        .unwrap();

    for uri in &uris {
        let providers = uri_to_providers(&query_runner, uri);
        assert_eq!(providers, vec![node1, node2]);
    }
    // Then: each node is providing the correct set of cids.
    let uris1 = content_registry(&query_runner, &node1);
    assert_eq!(uris1, uris);
    let uris2 = content_registry(&query_runner, &node2);
    assert_eq!(uris2, uris);

    // When: one of the nodes submits an update to remove a record for one cid.
    let updates = vec![ContentUpdate {
        uri: uris[0],
        remove: true,
    }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Then: the record is removed for that one provider.
    let providers = uri_to_providers(&query_runner, &uris[0]);
    assert_eq!(providers, vec![node2]);
    // Then: the rest of the records are not affected.
    for uri in uris.iter().skip(1) {
        let providers = uri_to_providers(&query_runner, uri);
        assert_eq!(providers, vec![node1, node2]);
    }
    let expected_cids = uris.clone()[1..].to_vec();
    let cids1 = content_registry(&query_runner, &node1);
    assert_eq!(cids1, expected_cids);

    // When: we remove the rest for that same node.
    let mut updates = Vec::new();
    for uri in uris.iter().skip(1) {
        updates.push(ContentUpdate {
            uri: *uri,
            remove: true,
        })
    }
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 3);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Then: all records for that provider are gone.
    for uri in uris {
        let providers = uri_to_providers(&query_runner, &uri);
        assert_eq!(providers, vec![node2]);
    }
    let cids = content_registry(&query_runner, &node1);
    assert!(cids.is_empty());
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_updates_for_cid_in_same_transaction() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [0u8; 32];

    // When: Add and remove the same given cid in the same transaction for some node.
    let updates = vec![
        ContentUpdate { uri, remove: false },
        ContentUpdate { uri, remove: true },
    ];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);

    // Then: the transaction is reverted.
    expect_tx_revert(
        update,
        &update_socket,
        ExecutionError::TooManyUpdatesForContent,
    )
    .await;

    // When: we provide same updates for cid in the same transaction.
    let uri = [88u8; 32];
    let updates = vec![
        ContentUpdate { uri, remove: false },
        ContentUpdate { uri, remove: false },
    ];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);

    // Then: the transaction is reverted.
    expect_tx_revert(
        update,
        &update_socket,
        ExecutionError::TooManyUpdatesForContent,
    )
    .await;
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_updates_for_cid_in_diff_transactions() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [0u8; 32];
    // Given: register given cid in the node's register.
    let updates = vec![ContentUpdate { uri, remove: false }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // When: we submit same update for same cid in different transaction.
    let updates = vec![ContentUpdate { uri, remove: false }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);

    // Then: the transaction is successful.
    expect_tx_success(update, &update_socket, ExecutionData::None).await;
}

#[tokio::test]
async fn test_submit_content_registry_update_remove_unknown_cid() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [0u8; 32];
    let updates = vec![ContentUpdate { uri, remove: false }];

    // Given: one node provides the cid.
    let update = prepare_content_registry_update(updates.clone(), &keystore[1].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Given: another node provides other cids except the given cid.
    let mut updates = Vec::new();
    for idx in 1..4u8 {
        updates.push(ContentUpdate {
            uri: [idx; 32],
            remove: false,
        });
    }
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // When: that node tries to remove registry for the given cid which it doesn't have.
    let updates = vec![ContentUpdate { uri, remove: true }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);

    // Then: the transaction is reverted.
    expect_tx_revert(
        update,
        &update_socket,
        ExecutionError::InvalidContentRemoval,
    )
    .await;
}

#[tokio::test]
async fn test_submit_content_registry_update_remove_unknown_cid_empty_registry() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: some cid that is not in the registry and the node is not providing any content.
    let updates = vec![ContentUpdate {
        uri: [68u8; 32],
        remove: true,
    }];

    // When: we try to remove the cid from the registry.
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);

    // Then: the transaction is reverted.
    expect_tx_revert(
        update,
        &update_socket,
        ExecutionError::InvalidStateForContentRemoval,
    )
    .await;
}
