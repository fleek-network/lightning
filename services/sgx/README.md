# Fleek Network SGX Service

## Build Requirements

```bash
cargo install fortanix-sgx-tools sgxs-tools
```

## Architecture

The service is divided into 2 parts:

### Runner / Untrusted Userspace

The main service binary acts as a runner which handles:
- starting the enclave
- feeding it requests through a special address `requests`
- exposing blockstore server

### Enclave / Trusted Execution Environment

The enclave is embedded into the service at compile time, which is loaded on startup and run via SGX.
TCP streams are used to connect to the service ipc for accessing node functionality, as well as requesting
file reads from the blockstore. Any information acquired outside the enclave must be regarded as untrusted,
and must have a way to verify the data. For example, client requests will include a signature, and wasm content
will always be read over verified blake3 streams, just like the node to node blockstore server.

#### Keysharing protocol

A keysharing protocol is used for private user keys

> TODO
