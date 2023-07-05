# CDN Service

The CDN service provides a protocol for exchanging blocks of blake3 content for delivery
acknowledgements, ensuring a fair exchange of resources.

The draft protocol specification can be found [here](SPEC.md).

## Standalone Example

An example server is included that provides a mocked version of a draco node running the
cdn service. It can be run with:

```bash
cargo run --example cdn_server --features dummy
```

An example client is also provided, that will request some content from the server. 
It can be run with:

```bash
cargo run --example cdn_client
```
