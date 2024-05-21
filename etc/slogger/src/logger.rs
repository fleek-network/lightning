use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

use log::{Log, Metadata};
use simplelog::{Config, LevelFilter, SharedLogger};

use crate::schema::Record;

pub struct SocketLogger {
    config: Option<Config>,
    level: LevelFilter,
    socket: Mutex<UnixStream>,
}

impl SocketLogger {
    pub fn new(level: LevelFilter, socket: UnixStream) -> Self {
        Self {
            config: None,
            level,
            socket: Mutex::new(socket),
        }
    }
}

impl Log for SocketLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        match bincode::serialize(&Record::from(record)) {
            Ok(data) => {
                let mut socket = self.socket.lock().unwrap();
                let _ = socket.write_all(data.as_slice());
            },
            Err(e) => {
                // This shouldn't happen and maybe we should use unwrap().
                eprintln!("failed to serialize record: {e}");
            },
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
