use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

use log::{error, Log, Metadata};
use simplelog::{LevelFilter, SharedLogger};

use crate::config::Config;
use crate::schema::Record;

pub struct SocketLogger {
    config: Config,
    level: LevelFilter,
    socket: Mutex<UnixStream>,
    simple_config: simplelog::Config,
}

impl SocketLogger {
    pub fn new(config: Config, level: LevelFilter, socket: UnixStream) -> Self {
        Self {
            config: config,
            level,
            socket: Mutex::new(socket),
            simple_config: simplelog::Config::default(),
        }
    }

    fn should_skip(&self, record: &log::Record) -> bool {
        if !self.config.filter_allow.is_empty() {
            return !self
                .config
                .filter_allow
                .iter()
                .any(|filter| record.target().starts_with(filter.as_ref()));
        }

        if !self.config.filter_ignore.is_empty() {
            return self
                .config
                .filter_ignore
                .iter()
                .any(|filter| record.target().starts_with(filter.as_ref()));
        }

        false
    }

    fn try_log(&self, record: &log::Record) {
        if self.should_skip(record) {
            return;
        }

        let mut record = Record::from(record);
        let mut id = String::new();
        if let Some(name) = std::thread::current().name() {
            id.push_str(format!("[{name}]").as_str());
        } else {
            let pid = std::process::id();
            id.push_str(format!("[{pid}]").as_str());
        }
        record.metadata.target.push_str(id.as_str());

        match bincode::serialize(&record) {
            Ok(data) => {
                let mut socket = self.socket.lock().unwrap();
                let mut framed = Vec::new();
                framed.extend(data.len().to_le_bytes().as_slice());
                framed.extend(data);
                let _ = socket.write_all(framed.as_slice());
            },
            Err(e) => {
                // This shouldn't happen and maybe we should use unwrap().
                error!("failed to serialize record: {e}");
            },
        }
    }
}

impl Log for SocketLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            self.try_log(record);
        }
    }

    fn flush(&self) {
        let _ = self.socket.lock().unwrap().flush();
    }
}

impl SharedLogger for SocketLogger {
    fn level(&self) -> LevelFilter {
        self.level
    }

    fn config(&self) -> Option<&simplelog::Config> {
        Some(&self.simple_config)
    }

    fn as_log(self: Box<Self>) -> Box<dyn Log> {
        Box::new(self)
    }
}
