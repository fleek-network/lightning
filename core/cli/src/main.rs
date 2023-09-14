mod args;

use clap::Parser;

use crate::args::CliArgs;

#[tokio::main]
async fn main() {
    let _ = CliArgs::parse();
}
