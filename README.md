![Fleek-Network-Lightning-GitHub-Banner](https://github.com/fleek-network/lightning/assets/55561695/9d58244f-5148-414b-923a-9c452e4d5088)

Lightning is the open-source Rust implementation of Fleek Network.

# Lightning - Fleek Network Node

![Build & Tests](https://img.shields.io/endpoint.svg?url=https%3A%2F%2Fgarnix.io%2Fapi%2Fbadges%2Ffleek-network%2Flightning%3Fbranch%3Dmain)

This repository contains the source code for the implementation of _Fleek Network_.

Here is the directory schema:

```txt
lightning
├── lib
│   ├── affair
│   ├── atomo
│   ├── blake3-stream
│   ├── blake3-tree
│   ├── fleek-crypto
│   ├── hp-fixed
│   ├── ink-quill
│   ├── sdk
│   ├── sdk-macros
│   └── simulon
├── core
│   ├── application
│   ├── blockstore
│   ├── consensus
│   ├── handshake
│   ├── interfaces
│   ├── mock
│   ├── node
│   ├── notifier
│   ├── origin-arweave
│   ├── origin-filecoin
│   ├── origin-ipfs
│   ├── pod
│   ├── rep-collector
│   ├── reputation
│   ├── rpc
│   ├── signer
│   ├── table
│   ├── test-utils
│   └── topology
└── services
    └── cdn
```

There are 3 top level directories `lib` & `core` and `services`:

1. `lib`: Any open source libraries we create to solve our own problems,
   these libraries are released with the friendly licenses with the Rust
   ecosystem (`MIT` | `Apache`).

2. `core`: This is all of the implementation of the core protocol, the main crate
   is `node` which contains our most important and released `main.rs`. Another important
   crate that is advised to everyone to get familiar with is `interfaces` which contains
   the top-down specification of all of the project.

3. `services`: Our services which we build using the `SDK`.

# Interfaces

The design pattern adopted for this software is highly inspired by the Object-Oriented model
described by Alan Kay, which may be a bit different from OOP that grew to fame due to Java.

In a nutshell this is similar to the same idea, and we represent different units of computation
and process as objects that communicate with each other by message passing.

# Rust Version

To get the best of the ecosystem by default the nightly version of Rust is set in the `rust-toolchain`
file, the version in use is `nightly-2023-07-01`, which is the version used by the `rustfmt` project
itself which is a good sign of reliability.

This is set so that the default that the IDEs will pick and default invocation of `cargo fmt` is consistent
between everyone and also we get to use some of the nightly options when it comes to clippy and fmt.

But this does not mean we're going to use this for building and releasing the binary, building the binary
for any actual use cases should use the `cargo +stable build` command and the coming scripts and CI config
will ease this process. Our MSRV (Minimum Supported Rust Version) is 1.75.

So in summary: We use the default that is set in the `rust-toolchain` to have consistent formatting of
the code. And does not necessarily indicate that it is the version of the compiler we are going to use
for builds, which should be rust stable and at a minimum version 1.75.

# Development

To run a local devnet node and perform a simple healthcheck, do the following:

```bash
# Initialize local devnet configuration
$ cargo run -- init --dev

# Start the node up
$ cargo run --all-features -- run

# In another terminal, send a request to the ping rpc endpoint
$ curl -X POST -H "Content-Type: application/json" -d '{
      "jsonrpc": "2.0",
      "method": "flk_ping",
      "params": [],
      "id": 1
    }' http://127.0.0.1:4230/rpc/v0
# Response: {"jsonrpc":"2.0","result":"pong","id":1}
```

To run standard development checks such as testing, linting, and building locally, you can execute [`dev/checks`](./dev/checks), or any of `dev/test`, `dev/clippy`, `dev/fmt`, `dev/build` individually.

# Nix

The project provides a nix flake for a determanistic development and build environment.

Nix (with flakes enabled) can be easily installed with https://github.com/DeterminateSystems/nix-installer

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

## Development shell

A development environment can be used with:

```bash
nix develop github:fleek-network/lightning

# For a local repo
nix develop .
```

## Build

The project can be built in a pure offline environment with:

```bash
nix build github:fleek-network/lightning

# For a local repo
nix build .
```

## Shell environment

A shell can also be spawned with the `lightning-node` binary provided in the path:

> Note: until we provide a cachix cache from ci, the node will be built locally before entering the shell

```bash
nix shell github:fleek-network/lightning

# For a local repo
nix shell .
```

The binary can also be ran directly:

```bash
nix run github:fleek-network/lightning -- run -v

# For a local repo
nix run . -- run -v
```
