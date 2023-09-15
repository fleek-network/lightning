use std::collections::HashMap;

use log::{LevelFilter, Record};
use log4rs::filter::{Filter, Response};

#[derive(Debug, Clone)]
pub struct CustomLogFilter {
    loggers: HashMap<String, LevelFilter>,
}

impl CustomLogFilter {
    pub fn new() -> Self {
        Self {
            loggers: HashMap::new(),
        }
    }

    pub fn insert(mut self, name: &str, level: LevelFilter) -> Self {
        self.loggers.insert(name.to_string(), level);
        self
    }
}

impl Filter for CustomLogFilter {
    fn filter(&self, record: &Record) -> Response {
        for (logger, level) in &self.loggers {
            if record.module_path().unwrap().contains(logger) && record.level() > *level {
                return Response::Reject;
            }
        }
        Response::Accept
    }
}
