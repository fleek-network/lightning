# Lightning - Fleek Network Node

[![Commits](https://github.com/fleek-network/lightning/actions/workflows/commits.yml/badge.svg)](https://github.com/fleek-network/lightning/actions/workflows/commits.yml)
[![Build & Tests](https://github.com/fleek-network/lightning/actions/workflows/cron.yml/badge.svg)](https://github.com/fleek-network/lightning/actions/workflows/cron.yml)
[![Code Coverage](https://codecov.io/github/fleek-network/lightning/branch/main/graph/badge.svg?token=7SN9432OHC)](https://codecov.io/github/fleek-network/lightning)

This repository contains the source code for the implementation of the *Fleek Network*. A
multi-service edge network.

Here is the directory schema:

```txt
lightning
├── lib
│   ├── affair
│   ├── atomo
│   └── blake3-tree
├── core
│   ├── node
│   ├── interfaces
│   ├── application
│   ├── blockstore
│   ├── consensus
│   ├── handshake
│   ├── identity
│   ├── origin-arweave
│   ├── origin-filecoin
│   ├── origin-ipfs
│   ├── pod
│   ├── reputation
│   ├── rpc
│   └── sdk
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
file, the version in use is `nightly-2023-01-24`, which is the version used by the `rustfmt` project
itself which is a good sign of reliability.

This is set so that the default that the IDEs will pick and default invocation of `cargo fmt` is consistent
between everyone and also we get to use some of the nightly options when it comes to clippy and fmt.

But this does not mean we're going to use this for building and releasing the binary, building the binary
for any actual use cases should use the `cargo +stable build` command and the coming scripts and CI config
will ease this process.

So in summary: We use the default that is set in the `rust-toolchain` to have consistent formatting of
the code. And does not necessarily indicate that it is the version of the compiler we are going to use
for builds.
