use resolved_pathbuf::ResolvedPathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub packet_filters_path: ResolvedPathBuf,
    pub profiles_path: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            packet_filters_path: ResolvedPathBuf::try_from(
                "~/.lightning/ebpf/state/packet-filter/filters.json",
            )
            .expect("Hardcoded path"),
            profiles_path: ResolvedPathBuf::try_from("~/.lightning/ebpf/state/MAC/profile.json")
                .expect("Hardcoded path"),
        }
    }
}
