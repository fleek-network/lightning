use std::io::Write;

use sgxkit::io::{get_input_data_string, OutputWriter};

fn main() {
    let input = get_input_data_string().unwrap_or("invalid data".into());
    let mut writer = OutputWriter::new();
    writer.write_all(input.as_bytes()).unwrap();
}
