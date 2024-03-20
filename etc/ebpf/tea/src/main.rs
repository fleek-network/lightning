mod cli;
mod commands;

use std::process::exit;

use clap::Parser;
use cli::Cli;

fn main() {
    let opts = Cli::parse();

    let ret = opts.exec();

    if let Err(e) = ret {
        println!("{e:?}");
        exit(1);
    }
}
