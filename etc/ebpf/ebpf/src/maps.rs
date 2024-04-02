use aya_bpf::macros::map;
use aya_bpf::maps::HashMap;
use common::{File, IpPortKey};

// Todo: replace with HashMapOfMaps or HashMapOfArrays when aya adds support.
#[map]
pub static BINARY_FILES: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static PROCESSES: HashMap<u64, u64> = HashMap::<u64, u64>::with_max_entries(1024, 0);
#[map]
pub static BLOCK_LIST: HashMap<IpPortKey, u32> =
    HashMap::<IpPortKey, u32>::with_max_entries(1024, 0);
