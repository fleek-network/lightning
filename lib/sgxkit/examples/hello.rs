use std::io::Write;

use sgxkit::io::OutputWriter;

fn main() {
    let mut writer = OutputWriter::new();
    writer
        .write_all(b"Hello, world! From, SGX WASM <3\n")
        .unwrap();
}
