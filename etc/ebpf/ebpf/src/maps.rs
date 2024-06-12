use aya_ebpf::macros::map;
use aya_ebpf::maps::{HashMap, LpmTrie};
use lightning_ebpf_common::{File, PacketFilter, PacketFilterParams, Profile, SubnetFilterParams};

#[map]
pub static PACKET_FILTERS: HashMap<PacketFilter, PacketFilterParams> =
    HashMap::<PacketFilter, PacketFilterParams>::with_max_entries(1024, 0);
#[map]
pub static SUBNET_FILTER: LpmTrie<u32, SubnetFilterParams> =
    LpmTrie::<u32, SubnetFilterParams>::with_max_entries(1024, 0);
#[map]
pub static PROFILES: HashMap<File, Profile> = HashMap::<File, Profile>::with_max_entries(1024, 0);
