# SGX Encrypt

Standalone utility to encrypt private data via ECIES (secp256k1 / aes-gcm) for the fleek sgx enclave.

## Usage

```
# Encrypt private data
sgxencrypt --pubkey <network pk> hello.wasm

# Upload encrypted binary to ipfs
fleek storage add hello.cipher

# Request from sgx service and specify to decrypt the wasm itself
```