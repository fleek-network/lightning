use aya_bpf::macros::map;
use aya_bpf::maps::HashMap;
use common::{File, FileMetadata, PacketFilter};

#[map]
pub static PACKET_FILTERS: HashMap<PacketFilter, u32> =
    HashMap::<PacketFilter, u32>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_ALLOW_BINFILE: HashMap<File, u64> =
    HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_ALLOW_PID: HashMap<u64, u64> = HashMap::<u64, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_DENY_BINFILE: HashMap<File, u64> =
    HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_DENY_PID: HashMap<u64, u64> = HashMap::<u64, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_METADATA: HashMap<u64, FileMetadata> =
    HashMap::<u64, FileMetadata>::with_max_entries(1024, 0);
