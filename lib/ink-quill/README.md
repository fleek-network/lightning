# ink-quill

Inspired by [Random Oracle](https://gist.github.com/qti3e/ed5b1e06514957f7032d3b41ff362c34), The ink-quill library is a Rust package for efficient and reliable transcript construction. The library provides an efficient way to group various pieces of information, making it easier for verification, storage, or exchange of data.

## Usage

```toml
[dependencies]
ink-quill = "0.1.0"
```

### TranscriptBuilder

Here's an example of how to create a transcript and add data to it:

```rust
use ink_quill::{TranscriptBuilder, TranscriptBuilderInput};

async fn main() {
    let mut builder = TranscriptBuilder::empty("example domain");
    builder = builder.with("nonce", &0)
        .with("transaction", &"deposit")
        .with_prefix("amount".to_string())
        .with("value", &1000_u64)
        compile();
}
```

### TranscriptBuilderInput

For your own data types, you can implement the TranscriptBuilderInput trait:

```rust

struct MyData {
    pub value: u64,
}

impl TranscriptBuilderInput for MyData {
    const TYPE: &'static str = "MyData";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.value.to_be_bytes().to_vec()
    }
}
```


## Contributing
Contributions to ink_quill are welcomed. Please make sure to run the test suite before opening a pull request

## License
[MIT](https://github.com/fleek-network/freek/blob/main/lib/ink-quill/LICENSE-MIT)
[APACHE 2.0](https://github.com/fleek-network/freek/blob/main/lib/ink-quill/LICENSE-APACHE)