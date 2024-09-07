use std::collections::HashMap;

use bit_set::BitSet;
use fleek_crypto::{ConsensusAggregateSignature, ConsensusSignature};
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch};
use rand::Rng;
use tempfile::tempdir;

use crate::database::{CheckpointerDatabase, CheckpointerDatabaseQuery};
use crate::rocks::RocksCheckpointerDatabase;
use crate::CheckpointerDatabaseConfig;

#[test]
fn test_add_and_get_checkpoint_headers() {
    let tempdir = tempdir().unwrap();
    let db = RocksCheckpointerDatabase::build(CheckpointerDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();

    // Check that the database is empty.
    let headers = query.get_checkpoint_headers(0);
    assert_eq!(headers, HashMap::new());

    // Add some headers and check that they're retrievable.
    let epoch0_headers = (0..10)
        .map(|_| {
            let header = random_checkpoint_header(0);
            (header.node_id, header)
        })
        .collect::<HashMap<_, _>>();
    for header in epoch0_headers.values() {
        db.set_node_checkpoint_header(0, header.clone());
    }
    assert_eq!(query.get_checkpoint_headers(0), epoch0_headers);

    // Add the same headers and check that it doesn't duplicate.
    for header in epoch0_headers.values() {
        db.set_node_checkpoint_header(0, header.clone());
    }
    assert_eq!(query.get_checkpoint_headers(0), epoch0_headers);

    // Add headers for a different epoch and check that it doesn't affect the previous epoch.
    assert_eq!(query.get_checkpoint_headers(1), HashMap::new());
    let epoch1_headers = (0..10)
        .map(|_| {
            let header = random_checkpoint_header(1);
            (header.node_id, header)
        })
        .collect::<HashMap<_, _>>();
    for header in epoch1_headers.values() {
        db.set_node_checkpoint_header(1, header.clone());
    }
    assert_eq!(query.get_checkpoint_headers(0), epoch0_headers);
    assert_eq!(query.get_checkpoint_headers(1), epoch1_headers);
}

#[test]
fn test_set_and_get_aggregate_checkpoint_header() {
    let tempdir = tempdir().unwrap();
    let db = RocksCheckpointerDatabase::build(CheckpointerDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();

    // Check that the database is empty.
    assert_eq!(query.get_aggregate_checkpoint_header(0), None);

    // Set an aggregate checkpoint header and check that it's retrievable.
    let header = random_aggregate_checkpoint_header(0);
    db.set_aggregate_checkpoint_header(0, header.clone());
    assert_eq!(
        query.get_aggregate_checkpoint_header(0),
        Some(header.clone())
    );

    // Set the same header again and check that it remains the same.
    db.set_aggregate_checkpoint_header(0, header.clone());
    assert_eq!(
        query.get_aggregate_checkpoint_header(0),
        Some(header.clone())
    );

    // Set the same epoch with a different header and check that it overwrites.
    let new_header = random_aggregate_checkpoint_header(0);
    db.set_aggregate_checkpoint_header(0, new_header.clone());
    assert_eq!(
        query.get_aggregate_checkpoint_header(0),
        Some(new_header.clone())
    );
    assert_ne!(new_header, header);

    // Set the header for a different epoch and check that it doesn't affect the previous
    // epoch.
    assert_eq!(query.get_aggregate_checkpoint_header(1), None);
    let header = random_aggregate_checkpoint_header(1);
    db.set_aggregate_checkpoint_header(1, header.clone());
    assert_eq!(
        query.get_aggregate_checkpoint_header(0),
        Some(new_header.clone())
    );
    assert_eq!(query.get_aggregate_checkpoint_header(1), Some(header));
}

fn random_checkpoint_header(epoch: Epoch) -> CheckpointHeader {
    let mut rng = rand::thread_rng();

    CheckpointHeader {
        epoch,
        node_id: rng.gen(),
        previous_state_root: rng.gen::<[u8; 32]>().into(),
        next_state_root: rng.gen::<[u8; 32]>().into(),
        serialized_state_digest: rng.gen::<[u8; 32]>(),
        signature: ConsensusSignature({
            let mut sig = [0u8; 48];
            for item in &mut sig {
                *item = rng.gen();
            }
            sig
        }),
    }
}

fn random_aggregate_checkpoint_header(epoch: Epoch) -> AggregateCheckpointHeader {
    let mut rng = rand::thread_rng();

    AggregateCheckpointHeader {
        epoch,
        state_root: rng.gen::<[u8; 32]>().into(),
        signature: ConsensusAggregateSignature({
            let mut sig = [0u8; 48];
            for item in &mut sig {
                *item = rng.gen();
            }
            sig
        }),
        nodes: (0..32).fold(BitSet::with_capacity(32), |mut bs, i| {
            if rng.gen_bool(0.5) {
                bs.insert(i);
            }
            bs
        }),
    }
}
