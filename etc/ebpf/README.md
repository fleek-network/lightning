# Lightning eBPF

## Packages

- `service` - Contains the main control application binary and a client/server library. 
- `ebpf` - Contains the kernel-space eBPF programs.

## Running

You can run the eBPF control application process via the Lightning Node CLI.
This feature is behind the `ebpf` feature flag.

## Dependencies

> You need a Linux platform to run our eBPF solution.

1. Install Rust toolchains.

```
rustup install stable
rustup toolchain install nightly --component rust-src
```

2. Install `bpf-linker` via the following command if running on a Linux x86_64 system.
If you're on any other architecture, please see the `aya` [docs](https://aya-rs.dev/book/start/development/) on how to install it.

```
cargo install bpf-linker
```

3. Install the `bindgen` executable.

```
cargo install bindgen-cli
```
