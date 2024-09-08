use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use fleek_blake3 as blake3;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};

type Entry = (Box<[u8]>, Box<[u8]>);

pub fn build_db_from_checkpoint(
    path: &Path,
    hash: [u8; 32],
    checkpoint: &[u8],
    extra_tables: &[String],
    options: Options,
) -> Result<(DB, Vec<String>)> {
    let mut table_map = deserialize_db(checkpoint)?;
    let mut extra_tables: HashSet<String> = HashSet::from_iter(extra_tables.iter().cloned());

    // If any of the extra tables are already in the checkpoint, remove them from the extra tables.
    for table in table_map.keys() {
        if extra_tables.contains(table) {
            extra_tables.remove(table);
        }
    }

    // Add the remaining extra tables to the table map.
    for table in &extra_tables {
        table_map.insert(table.clone(), vec![]);
    }

    // Build the tables and insert the data.
    let columns: Vec<String> = table_map.keys().cloned().collect();
    let cf_iter: Vec<_> = columns
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(name, options.clone()))
        .collect();
    let db = DB::open_cf_descriptors(&options, path, cf_iter)?;
    let mut table_names = Vec::new();
    for (table_name, entries) in table_map {
        let cf = db.cf_handle(&table_name).context("Unknown table name")?;
        table_names.push(table_name);
        for (key, value) in entries {
            db.put_cf(&cf, key, value)?;
        }
    }

    // Check that the serialized db matches the checkpoint.
    // We only serialize the tables that are not in the extra tables set.
    let tables_to_serialize = table_names
        .iter()
        .filter(|table| !extra_tables.contains(&table.to_string()))
        .cloned()
        .collect::<Vec<_>>();
    let bytes = serialize_db(&db, &tables_to_serialize)?;
    if checkpoint != bytes {
        return Err(anyhow!("Serialized db does not match checkpoint"));
    }

    // Verify that the calculated hash matches the hash of the checkpoint.
    let calc_hash = blake3::hash(&bytes);
    if &hash != calc_hash.as_bytes() {
        return Err(anyhow!("Failed to verify hash"));
    }

    // The returned table set needs to include all tables, including the extra ones that were not
    // serialized and present in the checkpoint.
    Ok((db, table_names))
}

/// Serializes a RocksDb database into a stream of bytes.
/// The serialization format is:
/// [num_tables][table1 name length][table1 name bytes][table1 bytes][table2 name length][table2
/// name bytes][table2 bytes]...
pub fn serialize_db(db: &DB, table_names: &[String]) -> Result<Vec<u8>> {
    let mut table_names_sort = table_names.to_vec();
    table_names_sort.sort();

    let snapshot = db.snapshot();

    let mut bytes = Vec::new();
    let num_tables = (table_names_sort.len() as u64).to_le_bytes();
    bytes.extend(&num_tables);

    for table_name in table_names_sort {
        let table_name_len = (table_name.len() as u64).to_le_bytes();
        bytes.extend(&table_name_len);
        bytes.extend(table_name.as_bytes());

        let cf = db
            .cf_handle(&table_name)
            .ok_or(anyhow!("Unknown table name"))?;
        let table_iter = snapshot.iterator_cf(&cf, IteratorMode::Start);
        let table_bytes = serialize_table(table_iter.flatten());
        bytes.extend(&table_bytes);
    }
    Ok(bytes)
}

/// Deserializes a RocksDb database from a stream of bytes.
pub fn deserialize_db(bytes: &[u8]) -> Result<BTreeMap<String, Vec<Entry>>> {
    let num_tables = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let mut pointer = 8;
    let mut tables = BTreeMap::new();
    for _ in 0..num_tables {
        let table_name_len =
            u64::from_le_bytes(bytes[pointer..pointer + 8].try_into().unwrap()) as usize;
        pointer += 8;
        let table_name = String::from_utf8(bytes[pointer..pointer + table_name_len].to_owned())?;
        pointer += table_name_len;
        let (table, pointer_offset) = deserialize_table(&bytes[pointer..bytes.len()]);
        tables.insert(table_name, table);
        pointer += pointer_offset;
    }
    Ok(tables)
}

/// Serializes a database table into a stream of bytes.
/// The serialization format is:
/// [num key value pairs][key1 length][key1 bytes][value1 length][value1 bytes][key2 length][key2
/// bytes][value2 length][value2 bytes]...
fn serialize_table<T: Iterator<Item = Entry>>(table_iter: T) -> Vec<u8> {
    let mut entries_count: u64 = 0;
    let mut bytes = vec![0; 8];
    for (key, val) in table_iter {
        bytes.extend((key.len() as u64).to_le_bytes());
        bytes.extend(key.as_ref());
        bytes.extend((val.len() as u64).to_le_bytes());
        bytes.extend(val.as_ref());
        entries_count += 1;
    }
    bytes[..8].copy_from_slice(&entries_count.to_le_bytes()[..8]);
    bytes
}

/// Deserializes a database table from a stream of bytes.
fn deserialize_table(bytes: &[u8]) -> (Vec<Entry>, usize) {
    let mut entries = Vec::new();
    let entries_count = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let mut pointer = 8;
    for _ in 0..entries_count {
        let key_length =
            u64::from_le_bytes(bytes[pointer..pointer + 8].try_into().unwrap()) as usize;
        pointer += 8;
        let key = &bytes[pointer..pointer + key_length];
        pointer += key_length;
        let value_length =
            u64::from_le_bytes(bytes[pointer..pointer + 8].try_into().unwrap()) as usize;
        pointer += 8;
        let value = &bytes[pointer..pointer + value_length];
        entries.push((key.into(), value.into()));
        pointer += value_length;
    }
    (entries, pointer)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ops::Range;

    use fleek_blake3 as blake3;
    use rand::Rng;
    use rocksdb::{ColumnFamilyDescriptor, Options, DB};

    use super::{
        build_db_from_checkpoint,
        deserialize_db,
        deserialize_table,
        serialize_db,
        serialize_table,
        Entry,
    };

    fn generate_random_bytes(length: usize) -> Box<[u8]> {
        let mut rng = rand::thread_rng();
        (0..length).map(|_| rng.gen_range(0..255)).collect()
    }

    fn build_random_table(
        num_entries: usize,
        key_length_range: Range<usize>,
        value_length_range: Range<usize>,
    ) -> Vec<Entry> {
        let mut entries = Vec::new();
        for _ in 0..num_entries {
            let key_length = rand::thread_rng().gen_range(key_length_range.clone());
            let key = generate_random_bytes(key_length);
            let value_length = rand::thread_rng().gen_range(value_length_range.clone());
            let value = generate_random_bytes(value_length);
            entries.push((key, value));
        }
        entries
    }

    #[test]
    fn test_serialize_deserialize_table() {
        let table_target = build_random_table(1000, 4..32, 4..32);

        let bytes = serialize_table(table_target.clone().into_iter());
        let (table, _) = deserialize_table(&bytes);
        assert_eq!(table_target, table);
    }

    #[test]
    fn test_serialize_deserialize_db() {
        let db_path = std::env::temp_dir().join("rocksdb_serialization_test");
        if db_path.exists() {
            std::fs::remove_dir_all(&db_path).unwrap();
        }
        let columns = vec![
            "table2".to_owned(),
            "table1".to_owned(),
            "table3".to_owned(),
            "table4".to_owned(),
        ];
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let cf_iter: Vec<_> = columns
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(name.to_owned(), options.clone()))
            .collect();
        let db = DB::open_cf_descriptors(&options, &db_path, cf_iter).unwrap();
        let mut target_tables: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for col in &columns {
            let cf = db.cf_handle(col).expect("Unknown table name");
            let num_entries = rand::thread_rng().gen_range(100..1000);
            for _ in 0..num_entries {
                let key_length = rand::thread_rng().gen_range(4..16);
                let key = generate_random_bytes(key_length);
                let value_length = rand::thread_rng().gen_range(4..32);
                let value = generate_random_bytes(value_length);

                target_tables
                    .entry(col.clone())
                    .or_default()
                    .push((key.clone(), value.clone()));
                db.put_cf(&cf, key, value).unwrap();
            }
            // sort table to be in the same order as the rocksdb table
            target_tables.get_mut(col).unwrap().sort();
        }
        let db_bytes = serialize_db(&db, &columns).expect("Failed to serialize db");
        let db_tables = deserialize_db(&db_bytes).expect("Failed to deserialize db");

        assert_eq!(target_tables, db_tables);
        if db_path.exists() {
            std::fs::remove_dir_all(&db_path).unwrap();
        }
    }

    #[test]
    fn test_build_db_from_checkpoint() {
        let path = std::env::temp_dir().join("lightning_test_rocksdb_1");
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("failed to remove old rocksdb for test");
        }
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let columns = vec![
            "table2".to_owned(),
            "table1".to_owned(),
            "table3".to_owned(),
            "table4".to_owned(),
        ];
        let cf_iter: Vec<_> = columns
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(name.to_owned(), options.clone()))
            .collect();
        let mut table_names = Vec::new();

        // Build a database and fill it with some data
        let db = DB::open_cf_descriptors(&options, &path, cf_iter).unwrap();
        for col in &columns {
            let cf = db.cf_handle(col).expect("Unknown table name");
            let num_entries = rand::thread_rng().gen_range(100..1000);
            table_names.push(col.clone());
            for _ in 0..num_entries {
                let key_length = rand::thread_rng().gen_range(4..16);
                let key = generate_random_bytes(key_length);
                let value_length = rand::thread_rng().gen_range(4..32);
                let value = generate_random_bytes(value_length);

                db.put_cf(&cf, key, value).unwrap();
            }
        }

        // Compute checkpoint for the database
        let checkpoint = serialize_db(&db, &table_names).expect("Failed to serialize db");
        let hash = blake3::hash(&checkpoint);
        let new_path = std::env::temp_dir().join("lightning_test_rocksdb_2");
        if new_path.exists() {
            std::fs::remove_dir_all(&new_path).expect("failed to remove old rocksdb for test");
        }

        // Build a new database from the checkpoint
        let (new_db, _) = build_db_from_checkpoint(
            &new_path,
            *hash.as_bytes(),
            &checkpoint,
            &["tree".to_string()],
            options,
        )
        .expect("Failed to build db from checkpoint");

        // Make sure that the checkpoints match
        let new_checkpoint = serialize_db(&new_db, &table_names).expect("Failed to serialize db");
        assert_eq!(checkpoint, new_checkpoint);

        std::fs::remove_dir_all(path).expect("failed to remove old rocksdb");
        std::fs::remove_dir_all(new_path).expect("failed to remove old rocksdb");
    }
}
