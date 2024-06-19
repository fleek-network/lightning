use once_cell::sync::Lazy;

pub mod args;
pub mod cli;
mod commands;
mod utils;

pub static VERSION: Lazy<String> = Lazy::new(|| {
    format!(
        "{}-{}",
        env!("CARGO_PKG_VERSION"),
        lightning_interfaces::types::REVISION
    )
});
