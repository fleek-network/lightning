use aya_bpf::macros::map;
use aya_bpf::maps::{HashMap, LpmTrie};
use common::{File, FileMetadata, PacketFilter, PacketFilterParams, SubnetFilterParams};

#[map]
pub static PACKET_FILTERS: HashMap<PacketFilter, PacketFilterParams> =
    HashMap::<PacketFilter, PacketFilterParams>::with_max_entries(1024, 0);
pub static SUBNET_FILTER: LpmTrie<u32, SubnetFilterParams> =
    LpmTrie::<u32, SubnetFilterParams>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_ALLOW: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_OPEN_DENY: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
pub static FILE_METADATA: HashMap<u64, FileMetadata> =
    HashMap::<u64, FileMetadata>::with_max_entries(1024, 0);
