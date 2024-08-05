# Merklize Tracing Example

An example of using the `merklize` crate with tracing.

## Usage

Start [Jaeger](https://www.jaegertracing.io/) in another terminal:

```sh
docker run -p4317:4317 -p16686:16686 jaegertracing/all-in-one:latest
```

Run this example:

```sh
cargo run --example tracing
```

Open the jaeger UI:

```sh
open http://localhost:16686/
```

![screenshot!](https://github.com/user-attachments/assets/d4cc4643-0f29-4baa-b1f8-34b89d3b53f5)
