# AI service

This service uses [ort](https://docs.rs/ort/latest/ort/). 

Please see the ort [docs ](https://ort.pyke.io/setup/platforms)
to see if there is support for your platform.

# HTTP API

### Inference

```
curl -X POST http://example.com/services/2/infer/<origin>/<cid>
```

Use the query params to specify the type of input
which can be a `safetensors` or a `borsh`-encoded vector. 

Refer to the Rust examples to see an example using 
`safetensors` and the JS example for `borsh`.

### Get model Information

```
curl http://example.com/services/2/model/<origin>/<cid>
```

### Query Params

- `encoding` - encoding of the payload, e.g. `safetensors` or `borsh`. Defaults to `borsh`.
- `format` - whether to use `JSON` or binary for the response. Defaults to `JSON`.

# CDK

You can use our CDK to connect with the service.

You must send an initial JSON message which specifies the parameters for the session.

```
StartSession {
    model: cid
    origin: blake3 | ipfs
    // Only cpu supported at the moment.
    device: cpu | cuda 
    // Format to use for the requests and responses 
    // for the duration of the session.
    contentFormat: json | bin
    // Encoding to use for the duration of the session.
    modelIoEncoding: safetensors | borsh
}
```

Please refer to the chatbot example to see how to use this API.


