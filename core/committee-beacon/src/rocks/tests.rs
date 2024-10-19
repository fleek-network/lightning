use std::collections::HashMap;

use lightning_interfaces::types::{CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal};
use rand::Rng;
use sha3::{Digest, Sha3_256};
use tempfile::tempdir;

use crate::database::{CommitteeBeaconDatabase, CommitteeBeaconDatabaseQuery};
use crate::rocks::RocksCommitteeBeaconDatabase;
use crate::CommitteeBeaconDatabaseConfig;

#[test]
fn test_set_and_get_beacon() {
    let tempdir = tempdir().unwrap();
    let db = RocksCommitteeBeaconDatabase::build(CommitteeBeaconDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();

    // Set a beacon and check that it's retrievable.
    let (commit, reveal) = generate_random_beacon();
    db.set_beacon(commit, reveal);
    assert_eq!(query.get_beacon(commit), Some(reveal));

    // Set the same beacon again and check that it remains the same.
    db.set_beacon(commit, reveal);
    assert_eq!(query.get_beacon(commit), Some(reveal));

    // Set the same beacon commit with a different reveal and check that it overwrites.
    let (_, new_reveal) = generate_random_beacon();
    db.set_beacon(commit, new_reveal);
    assert_eq!(query.get_beacon(commit), Some(new_reveal));
    assert_ne!(new_reveal, reveal);
}

#[test]
fn test_get_beacons() {
    let tempdir = tempdir().unwrap();
    let db = RocksCommitteeBeaconDatabase::build(CommitteeBeaconDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();

    // Set some beacons.
    let (commit1, reveal1) = generate_random_beacon();
    let (commit2, reveal2) = generate_random_beacon();
    db.set_beacon(commit1, reveal1);
    db.set_beacon(commit2, reveal2);

    // Check that we get all the beacons.
    assert_eq!(
        query.get_beacons(),
        HashMap::from([(commit1, reveal1), (commit2, reveal2)])
    );
}

#[test]
fn test_clear_beacons() {
    let tempdir = tempdir().unwrap();
    let db = RocksCommitteeBeaconDatabase::build(CommitteeBeaconDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();

    // Set some beacons.
    let (commit1, reveal1) = generate_random_beacon();
    let (commit2, reveal2) = generate_random_beacon();
    db.set_beacon(commit1, reveal1);
    db.set_beacon(commit2, reveal2);

    // Clear the beacons and check that they're gone.
    db.clear_beacons();
    assert_eq!(query.get_beacon(commit1), None);
    assert_eq!(query.get_beacon(commit2), None);
}

fn generate_random_beacon() -> (
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
) {
    let mut rng = rand::thread_rng();
    let reveal: [u8; 32] = rng.gen();

    let mut hasher = Sha3_256::new();
    hasher.update(reveal);
    let commit: [u8; 32] = hasher.finalize().into();

    (commit.into(), reveal)
}
