use aya_ebpf::macros::map;
use aya_ebpf::maps::{HashMap, LpmTrie};
use common::{File, FileRuleList, PacketFilter, PacketFilterParams, SubnetFilterParams};

#[map]
pub static PACKET_FILTERS: HashMap<PacketFilter, PacketFilterParams> =
    HashMap::<PacketFilter, PacketFilterParams>::with_max_entries(1024, 0);
#[map]
pub static SUBNET_FILTER: LpmTrie<u32, SubnetFilterParams> =
    LpmTrie::<u32, SubnetFilterParams>::with_max_entries(1024, 0);
#[map]
pub static FILE_RULES: HashMap<File, FileRuleList> =
    HashMap::<File, FileRuleList>::with_max_entries(1024, 0);
