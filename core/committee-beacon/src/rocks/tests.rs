use fxhash::FxHashMap;
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
    let epoch = 1;

    // Set a beacon and check that it's retrievable.
    let (commit, reveal) = generate_random_beacon();
    db.set_beacon(epoch, commit, reveal);
    assert_eq!(query.get_beacon(epoch, commit), Some(reveal));

    // Set the same beacon again and check that it remains the same.
    db.set_beacon(epoch, commit, reveal);
    assert_eq!(query.get_beacon(epoch, commit), Some(reveal));

    // Set the same beacon commit with a different reveal and check that it overwrites.
    let (_, new_reveal) = generate_random_beacon();
    db.set_beacon(epoch, commit, new_reveal);
    assert_eq!(query.get_beacon(epoch, commit), Some(new_reveal));
    assert_ne!(new_reveal, reveal);
}

#[test]
fn test_get_beacons() {
    let tempdir = tempdir().unwrap();
    let db = RocksCommitteeBeaconDatabase::build(CommitteeBeaconDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();
    let epoch = 1;

    // Set some beacons.
    let (commit1, reveal1) = generate_random_beacon();
    let (commit2, reveal2) = generate_random_beacon();
    db.set_beacon(epoch, commit1, reveal1);
    db.set_beacon(epoch, commit2, reveal2);

    // Check that we get all the beacons.
    assert_eq!(
        query.get_beacons(),
        FxHashMap::from_iter([((epoch, commit1), reveal1), ((epoch, commit2), reveal2)])
    );
}

#[test]
fn test_clear_beacons() {
    let tempdir = tempdir().unwrap();
    let db = RocksCommitteeBeaconDatabase::build(CommitteeBeaconDatabaseConfig {
        path: tempdir.path().to_path_buf().try_into().unwrap(),
    });
    let query = db.query();
    let epoch = 1;

    // Set some beacons.
    let (commit1, reveal1) = generate_random_beacon();
    let (commit2, reveal2) = generate_random_beacon();
    db.set_beacon(epoch, commit1, reveal1);
    db.set_beacon(epoch, commit2, reveal2);

    let (commit1, reveal1) = generate_random_beacon();
    let (commit2, reveal2) = generate_random_beacon();
    db.set_beacon(epoch + 1, commit1, reveal1);
    db.set_beacon(epoch + 1, commit2, reveal2);

    // Clear some beacons and check that they're gone.
    db.clear_beacons_before_epoch(epoch);
    assert_eq!(query.get_beacon(epoch, commit1), None);
    assert_eq!(query.get_beacon(epoch, commit2), None);
    assert_eq!(query.get_beacon(epoch + 1, commit1), Some(reveal1));
    assert_eq!(query.get_beacon(epoch + 1, commit2), Some(reveal2));
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
