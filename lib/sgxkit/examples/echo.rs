use std::io::Write;

use sgxkit::io::{get_input_data, OutputWriter};

#[no_mangle]
fn main() {
    let input = get_input_data().unwrap_or("no data".into());
    let mut writer = OutputWriter::new();
    writer.write_all(input.as_bytes()).unwrap();
}
