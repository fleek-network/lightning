use aya_bpf::macros::map;
use aya_bpf::maps::HashMap;
use common::{File, FileMetadata, PacketFilter, PacketFilterParams};

#[map]
pub static PACKET_FILTERS: HashMap<PacketFilter, PacketFilterParams> =
    HashMap::<PacketFilter, PacketFilterParams>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_ALLOW: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_DENY: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_METADATA: HashMap<u64, FileMetadata> =
    HashMap::<u64, FileMetadata>::with_max_entries(1024, 0);
