# POC JS Service

## Example

Store the example js in the blockstore, and copy the blake hash of the file

```bash
cargo run --bin lightning-node -- dev store services/js-poc/examples/example.js 
```

Run the node
```
cargo run --bin lightning-node -- run -v
```

Run the client
```
cargo run --example js-poc-client -- <blake3 hash of js file>
```
