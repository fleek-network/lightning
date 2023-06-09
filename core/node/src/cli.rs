use std::path::PathBuf;

use clap::{arg, Parser};

#[derive(Parser)]
#[command(about, version)]
pub struct CliArgs {
    /// Path to the toml configuration file
    #[arg(short, long, default_value = "draco.toml")]
    pub config: PathBuf,
}
