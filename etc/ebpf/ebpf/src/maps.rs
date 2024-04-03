use aya_bpf::macros::map;
use aya_bpf::maps::HashMap;
use common::{File, FileMetadata, IpPortKey};

#[map]
pub static BLOCK_LIST: HashMap<IpPortKey, u32> =
    HashMap::<IpPortKey, u32>::with_max_entries(1024, 0);
#[map]
pub static BIN_TO_FILE: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static PROCESS_TO_FILE: HashMap<u64, u64> = HashMap::<u64, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_METADATA: HashMap<u64, FileMetadata> =
    HashMap::<u64, FileMetadata>::with_max_entries(1024, 0);
