use std::env;
use std::fs::File;
use std::os::unix::net::UnixStream;
use std::str::FromStr;

use chrono::Local;
use resolved_pathbuf::ResolvedPathBuf;
use simplelog::{
    ColorChoice,
    CombinedLogger,
    ConfigBuilder,
    SharedLogger,
    TermLogger,
    TerminalMode,
    ThreadLogMode,
    ThreadPadding,
    WriteLogger,
};
use socket_logger::SocketLogger;
use tracing::log::LevelFilter;

pub fn setup() {
    let log_filter = match env::var("RUST_LOG") {
        Ok(level) => {
            if level == "tui" {
                None
            } else {
                match LevelFilter::from_str(&level.to_lowercase()) {
                    Ok(level) => Some(level),
                    Err(_) => Some(LevelFilter::Off),
                }
            }
        },
        Err(_) => Some(LevelFilter::Off),
    };

    let shared: Vec<Box<dyn SharedLogger>> = match log_filter {
        None => {
            let path = ResolvedPathBuf::try_from("~/.lightning/socket-logger/ctrl").unwrap();
            let socket = UnixStream::connect(path).unwrap();
            let config = socket_logger::Builder::new()
                .ignore("narwhal_consensus::bullshark")
                .ignore("anemo")
                .allow("lightning")
                .build();
            vec![Box::new(SocketLogger::new(
                config,
                LevelFilter::Trace,
                socket,
            ))]
        },
        Some(log_filter) => {
            let config = ConfigBuilder::new()
                .add_filter_ignore_str("narwhal_consensus::bullshark")
                .add_filter_ignore_str("anemo")
                // remove the line below if you want to see narwhal logs
                .add_filter_allow("lightning".to_string())
                .set_location_level(LevelFilter::Error)
                .set_thread_level(LevelFilter::Error)
                .set_thread_mode(ThreadLogMode::Names)
                .set_thread_padding(ThreadPadding::Right(4))
                .build();

            let date = Local::now();
            let log_file = env::temp_dir().join(format!(
                "lightning-test-epoch-change-committee-{}.log",
                date.format("%Y-%m-%d-%H:%M:%S")
            ));
            vec![
                TermLogger::new(
                    log_filter,
                    config.clone(),
                    TerminalMode::Mixed,
                    ColorChoice::Auto,
                ),
                WriteLogger::new(LevelFilter::Trace, config, File::create(log_file).unwrap()),
            ]
        },
    };

    // We swollow the result here because another e2e test might have already initialized the
    // logger.
    let _ = CombinedLogger::init(shared);
}
