# Fleek Network

This repository contains the source code for the implementation of the *Fleek Network*. A
multi-service edge network.

Here is the directory schema:

```txt
draco
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

In a nutshell
