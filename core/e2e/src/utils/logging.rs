use std::env;
use std::fs::File;
use std::str::FromStr;

use chrono::Local;
use log::LevelFilter;
use simplelog::{
    ColorChoice,
    CombinedLogger,
    ConfigBuilder,
    TermLogger,
    TerminalMode,
    ThreadLogMode,
    ThreadPadding,
    WriteLogger,
};

pub fn setup() {
    let log_filter = match env::var("RUST_LOG") {
        Ok(level) => match LevelFilter::from_str(&level.to_lowercase()) {
            Ok(level) => level,
            Err(_) => LevelFilter::Off,
        },
        Err(_) => LevelFilter::Off,
    };

    let date = Local::now();
    let log_file = std::env::temp_dir().join(format!(
        "lightning-test-epoch-change-committee-{}.log",
        date.format("%Y-%m-%d-%H:%M:%S")
    ));

    CombinedLogger::init(vec![
        TermLogger::new(
            log_filter,
            ConfigBuilder::new()
                .add_filter_ignore_str("narwhal_consensus::bullshark")
                .add_filter_ignore_str("anemo")
                .set_location_level(LevelFilter::Error)
                .set_thread_level(log_filter)
                .set_thread_mode(ThreadLogMode::Names)
                .set_thread_padding(ThreadPadding::Right(4))
                .build(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            ConfigBuilder::new()
                .add_filter_ignore_str("narwhal_consensus::bullshark")
                .add_filter_ignore_str("anemo")
                .set_location_level(LevelFilter::Error)
                .set_thread_level(log_filter)
                .set_thread_mode(ThreadLogMode::Names)
                .set_thread_padding(ThreadPadding::Right(4))
                .build(),
            File::create(log_file).unwrap(),
        ),
    ])
    .unwrap();
}
