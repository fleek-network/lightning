# Fleek eBPF

## Packages

- `cli` - cli for building and running 
eBPF kernel-space and user-space programs. 
Its main purpose is to inform us of user-space application requirements for 
the lightning integration.
- `ebpf` - eBPF programs.

## Linux

1. Install Rust toolchains.

```
rustup install stable
rustup toolchain install nightly --component rust-src
```

2. Install `bpf-linker` if running on a linux x86_64 system.

```
cargo install bpf-linker
```

## MacOS

Note: since eBPF is a Linux feature, you can only build.

1. Install `nightly-2024-02-12 `.
2. Install the `bpf-linker`. 

```
cargo install --git https://github.com/aya-rs/bpf-linker --rev 2ae2397701272bface78abc36b0b32d99a2a6998 --no-default-features
```

Note: Recently, nightly started using LLVM 18 which forced `bpf-linker` to upgrade to LLVM 18.
macOS's homebrew doesn't ship it so we have to use an older version.
For more info, see https://github.com/aya-rs/aya/pull/883.

3. Install target `ARCH=x86_64
   rustup target add ${ARCH}-unknown-linux-musl`.

If you don't have the musl cross compiler, you can install it with brew.

```bash
brew install FiloSottile/musl-cross/musl-cross --with-aarch64
```
Alternatively, if you run into an invalid option [error](https://github.com/FiloSottile/homebrew-musl-cross/issues/17#issuecomment-1817163468), you can do
```
brew install FiloSottile/musl-cross/musl-cross
brew reinstall musl-cross --with-aarch64
```

First compile the ebpf program into bytecode. For example,

```rust
cd xdp
cargo build
```

You can now compile the user-space component, `tea`, with 

```bash
RUSTFLAGS="-Clinker=${ARCH}-linux-musl-ld" cargo +nightly-2024-02-12 build --target=${ARCH}-unknown-linux-musl
```

## Links

* Development setup  - https://aya-rs.dev/book/start/development/
* Cross-compiling - https://aya-rs.dev/book/aya/crosscompile/