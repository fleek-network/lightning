use std::hash::{BuildHasher, Hasher};
use std::num::{NonZeroU32, Wrapping};

use rand::distributions::Standard;
use rand::rngs::SmallRng;
use rand::{thread_rng, Rng, SeedableRng};
use siphasher::sip128::{Hash128, Hasher128};

use crate::entry::InlineVec;

// Offset of some entry on the file.
pub type Offset = NonZeroU32;

pub struct PhfGenerator {
    items: Vec<(InlineVec, Offset)>,
}

pub struct HasherState {
    pub key: u64,
    pub disps: Vec<(u16, u16)>,
    pub map: Vec<u32>,
}

pub struct Hashes {
    pub g: u32,
    pub f1: u32,
    pub f2: u32,
}

impl PhfGenerator {
    pub fn new(size: usize) -> Self {
        Self {
            items: Vec::with_capacity(size),
        }
    }

    pub fn push(&mut self, entry: &[u8], position: Offset) {
        self.items.push((entry.into(), position));
    }

    pub fn finalize(self) -> HasherState {
        SmallRng::seed_from_u64(1234567890)
            .sample_iter(Standard)
            .find_map(|key| try_generate_hash(&self.items, key))
            .expect("failed to solve PHF")
    }
}

#[inline]
pub fn displace(f1: u32, f2: u32, d1: u32, d2: u32) -> u32 {
    (Wrapping(d2) + Wrapping(f1) * Wrapping(d1) + Wrapping(f2)).0
}

pub fn hash(entry: &[u8], key: u64) -> Hashes {
    let mut hasher = siphasher::sip128::SipHasher13::new_with_keys(0, key);
    let Hash128 {
        h1: lower,
        h2: upper,
    } = hasher.hash(entry);
    Hashes {
        g: (lower >> 32) as u32,
        f1: lower as u32,
        f2: upper as u32,
    }
}

const DEFAULT_LAMBDA: usize = 5;
fn try_generate_hash(entries: &[(InlineVec, Offset)], key: u64) -> Option<HasherState> {
    struct Bucket {
        idx: usize,
        keys: Vec<(Hashes, Offset)>,
    }

    let table_len = entries.len();
    let buckets_len = (entries.len() + DEFAULT_LAMBDA - 1) / DEFAULT_LAMBDA;
    assert!(table_len <= (u16::MAX as usize));

    let mut buckets = (0..buckets_len)
        .map(|idx| Bucket {
            idx,
            keys: Vec::new(),
        })
        .collect::<Vec<_>>();

    for (entry, offset) in entries {
        let hashes = hash(entry.as_slice(), key);
        buckets[(hashes.g as usize) % buckets_len]
            .keys
            .push((hashes, *offset));
    }

    // Sort descending
    buckets.sort_by(|a, b| a.keys.len().cmp(&b.keys.len()).reverse());

    let mut map = vec![0; table_len];
    let mut disps = vec![(0u16, 0u16); buckets_len];

    if entries.len() == 1 {
        return Some(HasherState { key, map, disps });
    }

    // the actual values corresponding to the markers above, as
    // (index, key) pairs, for adding to the main map once we've
    // chosen the right disps.
    let mut values_to_clean =
        Vec::with_capacity(buckets.iter().map(|b| b.keys.len()).max().unwrap());

    'buckets: for bucket in &buckets {
        let bucket_size = bucket.keys.len();

        for d1 in 0..(table_len as u16) {
            'disps: for d2 in 0..(table_len as u16) {
                values_to_clean.clear();

                for (hashes, offset) in &bucket.keys {
                    let idx = (displace(hashes.f1, hashes.f2, d1 as u32, d2 as u32)
                        % (table_len as u32)) as usize;

                    if map[idx] != 0 {
                        for &idx in &values_to_clean {
                            map[idx] = 0;
                        }
                        continue 'disps;
                    }

                    map[idx] = u32::from(*offset);
                    values_to_clean.push(idx);
                }

                // We've picked a good set of disps
                disps[bucket.idx] = (d1, d2);
                continue 'buckets;
            }
        }

        // Unable to find displacements for a bucket
        return None;
    }

    Some(HasherState { key, disps, map })
}
