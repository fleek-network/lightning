use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{query::NodeInfo, table::TableKey};

pub type Distance = TableKey;

pub const MAX_DISTANCE: [u8; 32] = [255u8; 32];

pub fn leading_zero_bits(key_a: &TableKey, key_b: &TableKey) -> usize {
    let distance = distance(key_a, key_b);
    let mut index = 0;
    for byte in distance {
        let leading_zeros = byte.leading_zeros();
        index += leading_zeros;
        if byte > 0 {
            break;
        }
    }
    index as usize
}

pub fn distance(key_a: &TableKey, key_b: &TableKey) -> Distance {
    key_a
        .iter()
        .zip(key_b.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<_>>()
        .try_into()
        .expect("Converting to array to succeed")
}

pub struct DistanceMap<V> {
    target: Arc<TableKey>,
    min_distance_seen: Distance,
    closest: BTreeMap<Distance, V>,
}

impl<V> DistanceMap<V> {
    pub fn new(target: TableKey) -> Self {
        Self {
            closest: BTreeMap::new(),
            min_distance_seen: MAX_DISTANCE,
            target: Arc::new(target),
        }
    }
    /// Returns true if at least one of the nodes added is closer to target,
    /// otherwise, it returns false.
    pub fn add_nodes(&mut self, nodes: Vec<(TableKey, V)>) -> bool {
        for (key, value) in nodes {
            let distance = distance(&self.target, &key);
            self.closest.insert(distance, value);
        }
        let min_distance = match self.closest.first_key_value() {
            Some((distance, _)) => distance,
            None => {
                panic!("empty query list");
            },
        };
        if min_distance < &self.min_distance_seen {
            self.min_distance_seen = *min_distance;
            true
        } else {
            false
        }
    }

    pub fn get(&self, key: &TableKey) -> Option<&V> {
        let distance = distance(&self.target, &key);
        self.closest.get(&distance)
    }

    pub fn get_mut(&mut self, key: &TableKey) -> Option<&mut V> {
        let distance = distance(&self.target, &key);
        self.closest.get_mut(&distance)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &V> {
        self.closest.values()
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.closest.values_mut()
    }
}
