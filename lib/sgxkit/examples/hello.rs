use std::io::Write;

use sgxkit::io::output_writer;

#[sgxkit::main]
fn main() {
    let mut writer = output_writer();
    writer
        .write_all(b"Hello, world! From, SGX WASM <3\n")
        .unwrap();
}
