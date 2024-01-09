pub mod args;
pub mod cli;
mod commands;
mod utils;

pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    compile_time_run::run_command_str!("git", "rev-parse", "HEAD")
);
