use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

use log::{error, Log, Metadata};
use simplelog::{Config, LevelFilter, SharedLogger};

use crate::schema::Record;

pub struct SocketLogger {
    config: Option<Config>,
    level: LevelFilter,
    socket: Mutex<UnixStream>,
}

impl SocketLogger {
    pub fn new(config: Config, level: LevelFilter, socket: UnixStream) -> Self {
        Self {
            config: Some(config),
            level,
            socket: Mutex::new(socket),
        }
    }
}

impl Log for SocketLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match bincode::serialize(&Record::from(record)) {
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

    fn flush(&self) {
        let _ = self.socket.lock().unwrap().flush();
    }
}

impl SharedLogger for SocketLogger {
    fn level(&self) -> LevelFilter {
        self.level
    }

    fn config(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    fn as_log(self: Box<Self>) -> Box<dyn Log> {
        Box::new(self)
    }
}
