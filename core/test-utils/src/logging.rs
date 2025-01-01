use std::env;
use std::fs::File;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::str::FromStr;

use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
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
use socket_logger::SocketLogger;
use tracing::log;
use tracing::log::LevelFilter;

pub fn setup(log_file_path: Option<PathBuf>) {
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

    match log_filter {
        None => {
            let path =
                ResolvedPathBuf::try_from(LIGHTNING_HOME_DIR.join("socket-logger/ctrl")).unwrap();
            let socket = UnixStream::connect(path).unwrap();
            let config = socket_logger::Builder::new()
                .ignore("narwhal_consensus::bullshark")
                .ignore("anemo")
                .allow("lightning")
                .build();
            log::set_max_level(LevelFilter::Trace);
            let _ = log::set_boxed_logger(Box::new(SocketLogger::new(
                config,
                LevelFilter::Trace,
                socket,
            )));
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

            // We swollow the result here because another e2e test might have already initialized
            // the logger.
            if let Some(log_file_path) = log_file_path {
                let _ = CombinedLogger::init(vec![
                    TermLogger::new(
                        log_filter,
                        config.clone(),
                        TerminalMode::Mixed,
                        ColorChoice::Auto,
                    ),
                    WriteLogger::new(
                        LevelFilter::Trace,
                        config,
                        File::create(log_file_path).unwrap(),
                    ),
                ]);
            } else {
                let _ = CombinedLogger::init(vec![TermLogger::new(
                    log_filter,
                    config.clone(),
                    TerminalMode::Mixed,
                    ColorChoice::Auto,
                )]);
            }
        },
    };
}
