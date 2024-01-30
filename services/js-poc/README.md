# POC JS Service

## Example

Store the example js in the blockstore, along with the bbb video, and copy the blake hash of the file

```bash
curl https://ipfs.io/ipfs/bafybeibi5vlbuz3jstustlxbxk7tmxsyjjrxak6us4yqq6z2df3jwidiwi -o bbb.mp4
lightning-node dev store bbb.mp4
lightning-node dev store services/js-poc/examples/example.js
```

Run the node
```
cargo run --bin lightning-node -- run -v
```

Run the client
```
cargo run --example js-poc-client -- <URI> <blake3|ipfs> [json parameter]
```
```
cargo run --example js-poc-client -- $(lightning-node dev store examples/example.js) blake3 '{"some":"thing"}'
```
