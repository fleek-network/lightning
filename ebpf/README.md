# Fleek eBPF

Install Rust toolchains.

``
rustup install stable
rustup toolchain install nightly --component rust-src
``

Install `bpf-linker` if running on a linux x86_64 system.

``
cargo install bpf-linker
``

Otherwise, on macos or linux on any other architecture, 
you need to install the newest stable version of LLVM first 
then install the linker. (This hasn't been tested).

``
cargo install --no-default-features bpf-linker
``

Install `bpftool`.