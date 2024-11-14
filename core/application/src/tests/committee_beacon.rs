use std::collections::HashSet;

use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::transaction::{TransactionBuilder, TransactionSigner};
use rand::Rng;
use types::{
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconPhase,
    CommitteeSelectionBeaconReveal,
    ExecutionData,
    ExecutionError,
    NodeIndex,
    NodeRegistryChange,
    NodeRegistryChangeSlashReason,
    Staking,
    TransactionResponse,
    UpdateMethod,
};

use crate::tests::utils::TestNetwork;

#[tokio::test]
async fn test_committee_beacon_epoch_change_success() {
    let network = TestNetwork::builder()
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from < 2/3+1 committee nodes (insufficient to start reveal
    // phase).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);
    assert!(!resp.change_epoch);

    // Check that we are still in the same commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transaction from another node (sufficient participation).
    let resp = network
        .execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert!(!resp.change_epoch);

    // Check that we are still in the same commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute a commit phase timeout transaction from any node to trigger transition to reveal
    // phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((5, 7)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from < 2/3+1 committee nodes (insufficient to finalize).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);
    assert!(!resp.change_epoch);

    // Check that we are still in the same reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((5, 7)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from 2/3+1 committee nodes (sufficient to finalize).
    let resp = network
        .execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [3; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);
    assert!(resp.change_epoch);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Success(ExecutionData::EpochChange)
    );

    // Check that we have transitioned out of the reveal phase.
    assert_eq!(query.get_committee_selection_beacon_phase(), None);
    assert_eq!(query.get_committee_selection_beacon_round(), None);

    // Check that the beacons have been cleared.
    assert!(query.get_committee_selection_beacons().is_empty());

    // Check that the epoch has been updated.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the committee members are the same as the genesis committee, since there are no
    // non-committee nodes in then network.
    assert_eq!(query.get_committee_members_by_index(), vec![0, 1, 2, 3]);
}

#[tokio::test]
async fn test_committee_beacon_transition_to_reveal_phase_early_with_full_participation() {
    let network = TestNetwork::builder()
        .with_committee_nodes(2)
        .with_non_committee_nodes(1)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from < 2/3+1 committee nodes (insufficient to start reveal
    // phase).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [11; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [12; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);
    assert!(!resp.change_epoch);

    // Check that we are still in the same commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from remaining nodes for full participation.
    let resp = network
        .execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [13; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((4, 6)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from < 2/3+1 committee nodes (insufficient to finalize).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [11; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [12; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert!(!resp.change_epoch);

    // Check that we are still in the same reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((4, 6)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from 2/3+1 committee nodes (sufficient to finalize).
    let resp = network
        .execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [13; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);
    assert!(resp.change_epoch);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Success(ExecutionData::EpochChange)
    );

    // Check that we have transitioned out of the reveal phase.
    assert_eq!(query.get_committee_selection_beacon_phase(), None);
    assert_eq!(query.get_committee_selection_beacon_round(), None);

    // Check that the beacons have been cleared.
    assert!(query.get_committee_selection_beacons().is_empty());

    // Check that the epoch has been updated.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the committee has changed.
    // We can rely on this being deterministic because we have fixed commits/reveals in this test,
    // which are combined and used as the seed for randomly choosing the new committee.
    assert_eq!(query.get_committee_members_by_index(), vec![2, 1]);
}

#[tokio::test]
async fn test_committee_beacon_committee_selection_is_random() {
    let mut committees = Vec::new();

    for _ in 0..50 {
        let network = TestNetwork::builder()
            .with_committee_nodes(2)
            .with_non_committee_nodes(2)
            .build()
            .await
            .unwrap();
        let query = network.query();

        // Execute epoch change transactions from 2/3+1 committee nodes.
        let epoch = query.get_current_epoch();
        let resp = network.execute_change_epoch(epoch).await.unwrap();
        assert_eq!(resp.block_number, 1);
        assert!(!resp.change_epoch);

        // Check that we have transitioned to the commit phase.
        assert_eq!(
            query.get_committee_selection_beacon_phase(),
            Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
        );
        assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

        // Generate random reveal values.
        let node0_reveal = generate_random_reveal();
        let node1_reveal = generate_random_reveal();
        let node2_reveal = generate_random_reveal();
        let node3_reveal = generate_random_reveal();

        // Execute commit transactions for all nodes.
        let resp = network
            .execute(vec![
                network
                    .node(0)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                        commit: CommitteeSelectionBeaconCommit::build(epoch, 0, node0_reveal),
                    }),
                network
                    .node(1)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                        commit: CommitteeSelectionBeaconCommit::build(epoch, 0, node1_reveal),
                    }),
                network
                    .node(2)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                        commit: CommitteeSelectionBeaconCommit::build(epoch, 0, node2_reveal),
                    }),
                network
                    .node(3)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                        commit: CommitteeSelectionBeaconCommit::build(epoch, 0, node3_reveal),
                    }),
            ])
            .await
            .unwrap();
        assert_eq!(resp.block_number, 2);
        assert!(!resp.change_epoch);

        // Check that we have transitioned to the reveal phase.
        assert_eq!(
            query.get_committee_selection_beacon_phase(),
            Some(CommitteeSelectionBeaconPhase::Reveal((3, 5)))
        );
        assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

        // Execute reveal transactions from all nodes.
        let resp = network
            .execute(vec![
                network
                    .node(0)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                        reveal: node0_reveal,
                    }),
                network
                    .node(1)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                        reveal: node1_reveal,
                    }),
                network
                    .node(2)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                        reveal: node2_reveal,
                    }),
                network
                    .node(3)
                    .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                        reveal: node3_reveal,
                    }),
            ])
            .await
            .unwrap();
        assert_eq!(resp.block_number, 3);
        assert!(resp.change_epoch);
        assert_eq!(
            resp.txn_receipts[3].response,
            TransactionResponse::Success(ExecutionData::EpochChange)
        );

        // Check that the committee length is the same.
        let committee = query.get_committee_members_by_index();
        assert_eq!(committee.len(), 2);

        committees.push(committee);
    }

    // Check that the commitees are not all the same.
    // Some may be the same, because they are randomly selected, but 100 of them should not all be
    // the same. This isn't perfect, but it should be good enough.
    assert!(committees.len() > 1);
    let first_committee = committees[0].clone();
    assert!(committees.iter().any(|x| *x != first_committee));
}

#[tokio::test]
async fn test_committee_beacon_no_participation_in_commit_phase() {
    let network = TestNetwork::builder()
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute commit phase timeout transaction.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);

    // Check that we have transitioned to a new commit phase in a new round.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((5, 7)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(1));

    // Execute empty blocks to exceed the commit phase timeout again.
    for _ in 0..3 {
        network.execute(vec![]).await.unwrap();
    }

    // Execute another commit phase timeout transaction.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 8);

    // Check that we have transitioned to a new commit phase in a new round.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((9, 11)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(2));
}

#[tokio::test]
async fn test_committee_beacon_insufficient_participation_in_commit_phase() {
    let network = TestNetwork::builder()
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from < 2/3+1 committee nodes (insufficient participation).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();

    // Execute commit phase timeout transaction.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);

    // Check that we have transitioned to a new commit phase in a new round.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((5, 7)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(1));

    // Execute commit transactions from 2/3+1 committee nodes to achieve sufficient participation.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Execute empty blocks to exceed the reveal phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute commit phase timeout transaction to transition to a new reveal phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 8);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((9, 11)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(1));

    // Execute reveal transactions from 2/3+1 committee nodes (sufficient to finalize).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [3; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 9);
    assert!(resp.change_epoch);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Success(ExecutionData::None)
    );
    assert_eq!(
        resp.txn_receipts[1].response,
        TransactionResponse::Success(ExecutionData::None)
    );
    assert_eq!(
        resp.txn_receipts[2].response,
        TransactionResponse::Success(ExecutionData::EpochChange)
    );

    // Check that we have transitioned out of the reveal phase.
    assert_eq!(query.get_committee_selection_beacon_phase(), None);
    assert_eq!(query.get_committee_selection_beacon_round(), None);

    // Check that the beacons have been cleared.
    assert!(query.get_committee_selection_beacons().is_empty());

    // Check that the epoch has been updated.
    assert_eq!(query.get_current_epoch(), epoch + 1);
}

#[tokio::test]
async fn test_committee_beacon_node_attempts_reveal_without_committment() {
    let network = TestNetwork::builder()
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from 2/3+1 committee nodes (sufficient to start reveal phase).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);
    assert!(!resp.change_epoch);

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute commit timeout transaction to transition to the reveal phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((6, 8)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transaction from node that did not commit.
    let resp = network
        .maybe_execute(vec![network.node(3).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [4; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconNotCommitted)
    );
}

#[tokio::test]
async fn test_committee_beacon_non_revealing_node_fully_slashed() {
    let network = TestNetwork::builder()
        // The node's initial stake is 1000, and it will be slashed 500, leaving insufficient stake
        // for a node.
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Check the initial stake of all nodes.
    for i in 0..network.nodes.len() {
        let node_index = i as NodeIndex;
        assert_eq!(
            query
                .get_node_info(&node_index, |n| n.stake.staked)
                .unwrap(),
            1000u64.into()
        );
    }

    // Execute epoch change transactions from committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from 2/3+1 committee nodes (sufficient to start reveal phase).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute commit timeout transaction to transition to the reveal phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((6, 8)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from 2 of the 3 nodes that committed, leaving 1
    // non-revealing node, and 1 non-participating node.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Execute empty blocks to exceed the reveal phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute reveal timeout transaction to transition to a new commit phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 9);

    // Check that we have transitioned back to a new commit phase in a new round.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((10, 12)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(1));

    // Check that the non-revealing node has been slashed, while no other nodes have been slashed.
    for i in 0..network.nodes.len() {
        let node_index = i as NodeIndex;
        if node_index == 2 {
            assert_eq!(
                query
                    .get_node_info(&node_index, |n| n.stake.staked)
                    .unwrap(),
                0u64.into()
            );
        } else {
            assert_eq!(
                query
                    .get_node_info(&node_index, |n| n.stake.staked)
                    .unwrap(),
                1000u64.into()
            );
        }
    }

    // Check that the node registry changes have been recorded.
    assert_eq!(query.get_committee_members_by_index(), vec![0, 1, 3]);
    assert_eq!(query.get_active_node_set(), HashSet::from([0, 1, 3]));
    let node_registry_changes = query
        .get_committee_info(&epoch, |c| c.node_registry_changes)
        .unwrap();
    assert_eq!(node_registry_changes.len(), 2);
    assert_eq!(
        node_registry_changes[&9],
        vec![(
            network.node(2).keystore.get_ed25519_pk(),
            NodeRegistryChange::Slashed((
                1000u64.into(),
                Staking {
                    staked: 0u64.into(),
                    stake_locked_until: 0,
                    locked: 0u64.into(),
                    locked_until: 0,
                },
                NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
            )),
        )]
    );

    // Check that the node registry changes are included on the block response.
    assert_eq!(resp.node_registry_changes.len(), 1);
    assert_eq!(
        resp.node_registry_changes,
        vec![(
            network.node(2).keystore.get_ed25519_pk(),
            NodeRegistryChange::Slashed((
                1000u64.into(),
                Staking {
                    staked: 0u64.into(),
                    stake_locked_until: 0,
                    locked: 0u64.into(),
                    locked_until: 0,
                },
                NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
            )),
        )]
    );

    // Execute commit transaction from the previously non-participating node, and check that it's
    // successful.
    let resp = network
        .execute(vec![network.node(3).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [14; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 10);

    // Execute commit transaction from the non-revealing node, and check that it's reverted.
    let resp = network
        .maybe_execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [13; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 11);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::InsufficientStake)
    );
}

#[tokio::test]
async fn test_committee_beacon_non_revealing_node_partially_slashed_insufficient_stake() {
    let network = TestNetwork::builder()
        .with_non_reveal_slash_amount(500)
        .with_min_stake(1000)
        // The node's initial stake is 1000, and it will be slashed 500, leaving insufficient stake
        // for a node.
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Check the initial stake of all nodes.
    for i in 0..network.nodes.len() {
        let node_index = i as NodeIndex;
        assert_eq!(
            query
                .get_node_info(&node_index, |n| n.stake.staked)
                .unwrap(),
            1000u64.into()
        );
    }

    // Execute epoch change transactions from committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from 2/3+1 committee nodes (sufficient to start reveal phase).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute commit timeout transaction to transition to the reveal phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((6, 8)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from 2 of the 3 nodes that committed, leaving 1
    // non-revealing node, and 1 non-participating node.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Execute empty blocks to exceed the reveal phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute reveal timeout transaction to transition to a new commit phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 9);

    // Check that we have transitioned back to a new commit phase in a new round.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((10, 12)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(1));

    // Check that the non-revealing node has been slashed, while no other nodes have been slashed.
    for i in 0..network.nodes.len() {
        let node_index = i as NodeIndex;
        if node_index == 2 {
            assert_eq!(
                query
                    .get_node_info(&node_index, |n| n.stake.staked)
                    .unwrap(),
                500u64.into()
            );
        } else {
            assert_eq!(
                query
                    .get_node_info(&node_index, |n| n.stake.staked)
                    .unwrap(),
                1000u64.into()
            );
        }
    }

    // Check that the node registry changes have been recorded.
    assert_eq!(query.get_committee_members_by_index(), vec![0, 1, 3]);
    assert_eq!(query.get_active_node_set(), HashSet::from([0, 1, 3]));
    let node_registry_changes = query
        .get_committee_info(&epoch, |c| c.node_registry_changes)
        .unwrap();
    assert_eq!(node_registry_changes.len(), 2);
    assert_eq!(
        node_registry_changes[&9],
        vec![(
            network.node(2).keystore.get_ed25519_pk(),
            NodeRegistryChange::Slashed((
                500u64.into(),
                Staking {
                    staked: 500u64.into(),
                    stake_locked_until: 0,
                    locked: 0u64.into(),
                    locked_until: 0,
                },
                NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
            )),
        )]
    );

    // Check that the node registry changes are included on the block response.
    assert_eq!(resp.node_registry_changes.len(), 1);
    assert_eq!(
        resp.node_registry_changes,
        vec![(
            network.node(2).keystore.get_ed25519_pk(),
            NodeRegistryChange::Slashed((
                500u64.into(),
                Staking {
                    staked: 500u64.into(),
                    stake_locked_until: 0,
                    locked: 0u64.into(),
                    locked_until: 0,
                },
                NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
            )),
        )]
    );

    // Execute commit transaction from the previously non-participating node, and check that it's
    // successful.
    let resp = network
        .execute(vec![network.node(3).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [14; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 10);

    // Execute commit transaction from the non-revealing node, and check that it's reverted.
    let resp = network
        .maybe_execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [13; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 11);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::InsufficientStake)
    );
}

#[tokio::test]
async fn test_committee_beacon_non_revealing_node_partially_slashed_sufficient_stake() {
    let network = TestNetwork::builder()
        .with_non_reveal_slash_amount(500)
        .with_min_stake(500)
        // The node's initial stake is 1000, and it will be slashed 500, leaving sufficient stake
        // for a node.
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Check the initial stake of all nodes.
    for i in 0..network.nodes.len() {
        let node_index = i as NodeIndex;
        assert_eq!(
            query
                .get_node_info(&node_index, |n| n.stake.staked)
                .unwrap(),
            1000u64.into()
        );
    }

    // Execute epoch change transactions from committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);
    assert!(!resp.change_epoch);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from 2/3+1 committee nodes (sufficient to start reveal phase).
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute commit timeout transaction to transition to the reveal phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((6, 8)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions from 2 of the 3 nodes that committed, leaving 1
    // non-revealing node, and 1 non-participating node.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Execute empty blocks to exceed the reveal phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute reveal timeout transaction to transition to a new commit phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 9);

    // Check that we have transitioned back to a new commit phase in a new round.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((10, 12)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(1));

    // Check that the non-revealing node has been slashed, while no other nodes have been slashed.
    for i in 0..network.nodes.len() {
        let node_index = i as NodeIndex;
        if node_index == 2 {
            assert_eq!(
                query
                    .get_node_info(&node_index, |n| n.stake.staked)
                    .unwrap(),
                500u64.into()
            );
        } else {
            assert_eq!(
                query
                    .get_node_info(&node_index, |n| n.stake.staked)
                    .unwrap(),
                1000u64.into()
            );
        }
    }

    // Check that the non-revealing node has NOT been removed from the committee or active node set.
    assert_eq!(query.get_committee_members_by_index(), vec![0, 1, 2, 3]);
    assert_eq!(query.get_active_node_set(), HashSet::from([0, 1, 2, 3]));

    // Check that the node registry changes were recorded.
    let node_registry_changes = query
        .get_committee_info(&epoch, |c| c.node_registry_changes)
        .unwrap();
    let expected_changes = vec![(
        network.node(2).keystore.get_ed25519_pk(),
        NodeRegistryChange::Slashed((
            500u64.into(),
            Staking {
                staked: 500u64.into(),
                stake_locked_until: 0,
                locked: 0u64.into(),
                locked_until: 0,
            },
            NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
        )),
    )];
    assert_eq!(node_registry_changes[&9], expected_changes);
    assert_eq!(resp.node_registry_changes, expected_changes);

    // Execute commit transaction from the previously non-participating node, and check that it's
    // successful.
    let resp = network
        .execute(vec![network.node(3).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [14; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 10);

    // Execute commit transaction from the non-revealing node, and check that it's reverted.
    let resp = network
        .maybe_execute(vec![network.node(2).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 1, [13; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 11);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconNonRevealingNode)
    );
}

#[tokio::test]
async fn test_committee_beacon_node_submits_commit_when_commit_phase_not_started() {
    let network = TestNetwork::builder()
        .with_committee_nodes(1)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute commit transaction from a committee node outside of the commit phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(0, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 1);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted)
    );

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 2);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((2, 4)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from 2/3+1 committee nodes (sufficient to start reveal phase).
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((4, 6)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transaction while in the reveal phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(0, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted)
    );
}

#[tokio::test]
async fn test_committee_beacon_node_submits_reveal_outside_of_reveal_phase() {
    let network = TestNetwork::builder()
        .with_committee_nodes(1)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute reveal transaction before the commit and reveal phases start.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 1);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted)
    );

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 2);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((2, 4)))
    );

    // Execute reveal transaction while in the commit phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted)
    );

    // Check that we are still in the same commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((2, 4)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));
}

#[tokio::test]
async fn test_committee_beacon_node_submit_timeouts_at_wrong_times() {
    let network = TestNetwork::builder()
        .with_committee_nodes(2)
        .with_commit_phase_duration(5)
        .with_reveal_phase_duration(5)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute commit timeout transaction before the commit phase starts.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 1);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted)
    );

    // Execute reveal timeout transaction before the commit and reveal phases start.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted)
    );

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 3);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((3, 8)))
    );

    // Execute commit timeout transaction while in the commit phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconCommitPhaseNotTimedOut)
    );

    // Execute reveal timeout transaction while in the commit phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted)
    );

    // Check that we are still in the same commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((3, 8)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transaction while in the commit phase.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Execute reveal timeout transaction while in the commit phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 7);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted)
    );

    // Execute commit transaction from remaining node to transition to the reveal phase.
    let resp = network
        .execute(vec![network.node(1).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 8);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((9, 14)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit timeout transaction while in the reveal phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 9);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted)
    );

    // Execute reveal timeout transaction while in the reveal phase.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 10);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconRevealPhaseNotTimedOut)
    );

    // Check that we are still in the same reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((9, 14)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transactions while in the reveal phase.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 11);
    assert!(resp.change_epoch);
    assert_eq!(
        resp.txn_receipts[1].response,
        TransactionResponse::Success(ExecutionData::EpochChange)
    );

    // Check that we have transitioned out of the reveal phase.
    assert_eq!(query.get_committee_selection_beacon_phase(), None);
    assert_eq!(query.get_committee_selection_beacon_round(), None);

    // Check that the epoch has been updated.
    assert_eq!(query.get_current_epoch(), epoch + 1);
}

#[tokio::test]
async fn test_committee_beacon_invalid_reveal_mismatch_with_commit() {
    let network = TestNetwork::builder()
        .with_committee_nodes(1)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transaction from the committee node.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((3, 5)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transaction from the committee node, with a different reveal than the commit.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [2; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconInvalidReveal)
    );

    // Check that we are still in the same reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((3, 5)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));
}

#[tokio::test]
async fn test_committee_beacon_node_already_committed() {
    let network = TestNetwork::builder()
        .with_committee_nodes(2)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transaction from the committee node.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute another commit transaction from the same node.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconAlreadyCommitted)
    );
}

#[tokio::test]
async fn test_committee_beacon_already_revealed() {
    let network = TestNetwork::builder()
        .with_committee_nodes(2)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from both committee nodes.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((3, 5)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transaction from a committee node.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Execute same reveal transaction again.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconAlreadyRevealed)
    );
}

#[tokio::test]
async fn test_committee_beacon_non_active_node_commit_is_reverted() {
    let network = TestNetwork::builder()
        .with_committee_nodes(2)
        .with_non_committee_nodes(1)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Check that all the nodes are in the active node set.
    assert_eq!(query.get_active_node_set(), HashSet::from([0, 1, 2]));

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Execute opt-out transaction from 1 committee node and 1 non-committee node, which will take
    // affect in the next epoch in terms of committee.active_node_set being updated.
    let resp = network
        .execute(vec![
            network.node(0).build_transaction(UpdateMethod::OptOut {}),
            network.node(2).build_transaction(UpdateMethod::OptOut {}),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute commit transactions from all the nodes to transition to the reveal phase.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Execute reveal transactions from all the nodes.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [3; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);

    // Check that we have changed to the next epoch.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the committee active node set is updated.
    assert_eq!(query.get_active_node_set(), HashSet::from([1]));

    // Emit epoch change transactions from the committee nodes.
    let resp = network.execute_change_epoch(epoch + 1).await.unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((5, 7)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from the 2 non-active nodes, and check that they are reverted.
    let resp = network
        .maybe_execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconNodeNotActive)
    );
    assert_eq!(
        resp.txn_receipts[1].response,
        TransactionResponse::Revert(ExecutionError::CommitteeSelectionBeaconNodeNotActive)
    );
}

#[tokio::test]
async fn test_committee_beacon_node_can_reveal_after_committing_and_becoming_inactive() {
    let network = TestNetwork::builder()
        .with_committee_nodes(4)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from the nodes.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute opt-out transaction from 1 of the nodes.
    let resp = network
        .execute(vec![
            network.node(0).build_transaction(UpdateMethod::OptOut {}),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();

    // Execute commit timeout transaction to transition to the reveal phase.
    // Check that we can execute the commit phase timeout transaction from the non-active node.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((6, 8)))
    );

    // Execute reveal transactions from the committed nodes.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [3; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Check that we have transitioned out of the reveal phase.
    assert_eq!(query.get_committee_selection_beacon_phase(), None);
    assert_eq!(query.get_committee_selection_beacon_round(), None);

    // Check that the beacons have been cleared.
    assert!(query.get_committee_selection_beacons().is_empty());

    // Check that the epoch has been updated.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the new committee only includes the active nodes.
    assert_eq!(query.get_committee_members_by_index(), vec![1, 2, 3]);
}

#[tokio::test]
async fn test_committee_beacon_node_with_insufficient_stake_cannot_commit() {
    let network = TestNetwork::builder()
        .with_stake_lock_time(0)
        .with_committee_nodes(2)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Unstake and withdraw the stake of 1 of the nodes.
    let node_public_key = network.node(0).keystore.get_ed25519_pk();
    let amount = query.get_node_info(&0, |node| node.stake).unwrap().staked;
    let resp = network
        .execute(vec![
            TransactionBuilder::from_update(
                UpdateMethod::Unstake {
                    amount,
                    node: node_public_key,
                },
                network.chain_id,
                1,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
            TransactionBuilder::from_update(
                UpdateMethod::WithdrawUnstaked {
                    node: node_public_key,
                    recipient: Some(network.node(0).owner_secret_key.to_pk().into()),
                },
                network.chain_id,
                2,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute commit transaction from the unstaked node, and check that it is reverted.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::InsufficientStake)
    );
}

#[tokio::test]
async fn test_committee_beacon_node_with_insufficient_stake_cannot_reveal() {
    let network = TestNetwork::builder()
        .with_stake_lock_time(0)
        .with_committee_nodes(2)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from the nodes.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((3, 5)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Unstake and withdraw the stake of 1 of the nodes.
    let node_public_key = network.node(0).keystore.get_ed25519_pk();
    let amount = query.get_node_info(&0, |node| node.stake).unwrap().staked;
    let resp = network
        .execute(vec![
            TransactionBuilder::from_update(
                UpdateMethod::Unstake {
                    amount,
                    node: node_public_key,
                },
                network.chain_id,
                1,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
            TransactionBuilder::from_update(
                UpdateMethod::WithdrawUnstaked {
                    node: node_public_key,
                    recipient: Some(network.node(0).owner_secret_key.to_pk().into()),
                },
                network.chain_id,
                2,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Execute reveal transaction from the unstaked node, and check that it is reverted.
    let resp = network
        .maybe_execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::InsufficientStake)
    );
}

#[tokio::test]
async fn test_committee_beacon_transactions_from_account_are_reverted() {
    let network = TestNetwork::builder()
        .with_committee_nodes(2)
        .with_commit_phase_duration(5)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 6)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transaction from the node.
    let resp = network
        .execute(vec![network.node(0).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute commit transaction from the node owner account and other account, and check that they
    // are reverted.
    let other_account = AccountOwnerSecretKey::generate();
    let resp = network
        .maybe_execute(vec![
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                },
                network.chain_id,
                1,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                },
                network.chain_id,
                1,
                &TransactionSigner::AccountOwner(other_account.clone()),
            )
            .into(),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );
    assert_eq!(
        resp.txn_receipts[1].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );

    // Execute commit phase timeout transaction from the node owner account and other account, and
    // check that they are reverted.
    let resp = network
        .maybe_execute(vec![
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
                network.chain_id,
                2,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
                network.chain_id,
                2,
                &TransactionSigner::AccountOwner(other_account.clone()),
            )
            .into(),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );
    assert_eq!(
        resp.txn_receipts[1].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );

    // Execute commit transaction from the other node, which transitions to the reveal phase.
    let resp = network
        .execute(vec![network.node(1).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
            },
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((6, 8)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute reveal transaction from the node owner account and other account, and check that they
    // are reverted.
    let resp = network
        .maybe_execute(vec![
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
                network.chain_id,
                3,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
                network.chain_id,
                3,
                &TransactionSigner::AccountOwner(other_account.clone()),
            )
            .into(),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );
    assert_eq!(
        resp.txn_receipts[1].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );

    // Execute empty blocks to exceed the reveal phase timeout.
    network.execute(vec![]).await.unwrap();
    network.execute(vec![]).await.unwrap();

    // Execute reveal phase timeout transaction from the node owner account and other account, and
    // check that they are reverted.
    let resp = network
        .maybe_execute(vec![
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
                network.chain_id,
                4,
                &TransactionSigner::AccountOwner(network.node(0).owner_secret_key.clone()),
            )
            .into(),
            TransactionBuilder::from_update(
                UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
                network.chain_id,
                4,
                &TransactionSigner::AccountOwner(other_account.clone()),
            )
            .into(),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 9);
    assert_eq!(
        resp.txn_receipts[0].response,
        TransactionResponse::Revert(ExecutionError::OnlyNode)
    );
}

#[tokio::test]
async fn test_committee_beacon_non_committee_node_participation() {
    let network = TestNetwork::builder()
        .with_committee_nodes(4)
        .with_non_committee_nodes(2)
        .build()
        .await
        .unwrap();
    let query = network.query();

    // Execute epoch change transactions from 2/3+1 committee nodes.
    let epoch = query.get_current_epoch();
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that we have transitioned to the commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute commit transactions from 2/3+1 committee nodes and a non-committee node.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [1; 32]),
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [3; 32]),
                }),
            network
                .node(5)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                    commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [6; 32]),
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Check that we are still in the same commit phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Commit((1, 3)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Execute empty blocks to exceed the commit phase timeout.
    network.execute(vec![]).await.unwrap();

    // Execute commit timeout transaction from non-committee node to transition to the reveal phase.
    let resp = network
        .execute(vec![network.node(5).build_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);

    // Check that we have transitioned to the reveal phase.
    assert_eq!(
        query.get_committee_selection_beacon_phase(),
        Some(CommitteeSelectionBeaconPhase::Reveal((5, 7)))
    );
    assert_eq!(query.get_committee_selection_beacon_round(), Some(0));

    // Remove a committee node from the active set so that we can check that a new, different
    // committee is selected.
    // Note that we remove the non-participating committee node, otherwise the reveal transaction
    // from the removed node would be rejected.
    let resp = network
        .execute(vec![
            network.node(3).build_transaction(UpdateMethod::OptOut {}),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Execute reveal transaction from all nodes that committed.
    let resp = network
        .execute(vec![
            network
                .node(0)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [1; 32],
                }),
            network
                .node(1)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [2; 32],
                }),
            network
                .node(2)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [3; 32],
                }),
            network
                .node(5)
                .build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                    reveal: [6; 32],
                }),
        ])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);
    assert!(resp.change_epoch);
    assert_eq!(
        resp.txn_receipts[3].response,
        TransactionResponse::Success(ExecutionData::EpochChange)
    );

    // Check that we have transitioned out of the reveal phase.
    assert_eq!(query.get_committee_selection_beacon_phase(), None);
    assert_eq!(query.get_committee_selection_beacon_round(), None);

    // Check that the beacons have been cleared.
    assert!(query.get_committee_selection_beacons().is_empty());

    // Check that the epoch has been updated.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the new committee is different from the old committee, but is still the same size.
    assert_ne!(query.get_committee_members_by_index(), vec![0, 1, 2, 3]);
    assert_eq!(query.get_committee_members().len(), 4);
}

fn generate_random_reveal() -> CommitteeSelectionBeaconReveal {
    let mut rng = rand::thread_rng();
    let mut reveal = [0u8; 32];
    rng.fill(&mut reveal);
    reveal
}
