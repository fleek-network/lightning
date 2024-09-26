# SGX Encrypt

Standalone utility to encrypt private data via ECIES (secp256k1 / aes-gcm) for the fleek sgx enclave.

## Usage

### Wasm modules

```
# Encrypt wasm module
sgxencrypt module hello.wasm

# Upload encrypted binary to ipfs
fleek storage add hello.cipher

# Request from sgx service and specify to decrypt the wasm itself
```

### Shared data (for multiple wasm modules)

```
# Encrypt data
sgxencrypt shared foo.json <wasm hash 1> <wasm hash 2> ...

# Upload data to ipfs
fleek storage add foo.cipher
```

### Wasm module specific data

```
# Encrypt data
sgxencrypt wasm <wasm hash> foo.json
```
