# SGX Kit

Development toolkit for building wasm modules on the Fleek Network SGX Service.

## Examples

Example wasm binaries can be compiled like so:

```bash
# Build hello wasm
cargo build -r --example sgx-wasm-hello --target wasm32-unknown-unknown

# Or do the same with the echo example
cargo build -r --example sgx-wasm-echo --target wasm32-unknown-unknown
```

### Loading data onto the node

The resulting binaries can then be uploaded to ipfs and cached on a node:

```bash
fleek storage add target/wasm32-unknown-unknown/release/examples/sgx-wasm-hello.wasm
curl fleek-test.network/services/0/ipfs/<CID>
```

The sgx service only supports raw blake3 hashes at this time.
To get the hash of the cached wasm file, you can use the `b3sum` utility:

```bash
b3sum target/wasm32-unknown-unknown/release/examples/sgx-wasm-hello.wasm
```

### Calling the sgx service directly

The wasm can then be called on the service like so:

```bash
curl fleek-test.network/services/3 --data '{"hash": "<b3sum hash>", "input": "foo"}'
```

### Private wasm modules

The wasm can also be encrypted/sealed for the enclave before uploading.
Using [sgxencrypt](../../lib/sgxencrypt):

```bash
sgxencrypt --pubkey <network seal pk> target/wasm32-unknown-unknown/release/examples/sgx-wasm-hello.wasm -o hello.wasm.cipher
fleek storage add hello.wasm.cipher
```

The service will decrypt with a request like so:

```bash
curl fleek-test.network/services/3 --data '{"hash": "<b3sum hash>", "decrypt": true, "input": "foo"}'
```

