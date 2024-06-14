use aya_ebpf::macros::map;
use aya_ebpf::maps::{HashMap, LpmTrie, LruPerCpuHashMap, PerCpuHashMap, RingBuf};
use lightning_ebpf_common::{
    Buffer,
    File,
    FileCacheKey,
    FileRule,
    PacketFilter,
    PacketFilterParams,
    Profile,
    SubnetFilterParams,
};

#[map]
pub static PACKET_FILTERS: HashMap<PacketFilter, PacketFilterParams> =
    HashMap::<PacketFilter, PacketFilterParams>::with_max_entries(1024, 0);
#[map]
pub static SUBNET_FILTER: LpmTrie<u32, SubnetFilterParams> =
    LpmTrie::<u32, SubnetFilterParams>::with_max_entries(1024, 0);
#[map]
pub static PROFILES: HashMap<File, Profile> = HashMap::<File, Profile>::with_max_entries(1024, 0);
#[map]
pub static BUFFERS: PerCpuHashMap<u32, Buffer> =
    PerCpuHashMap::<u32, Buffer>::with_max_entries(8, 0);
#[map]
pub static FILE_CACHE: LruPerCpuHashMap<FileCacheKey, FileRule> =
    LruPerCpuHashMap::<FileCacheKey, FileRule>::with_max_entries(128, 0);
#[map]
pub static EVENTS: RingBuf = RingBuf::with_byte_size(1024 * 4096, 0);
