use std::fmt;
use std::string::ToString;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Help,
    NavLeft,
    NavRight,
    Up,
    Down,
    Add,
    Remove,
}

impl Action {
    pub fn is_navigation_action(&self) -> bool {
        matches!(self, Action::NavLeft | Action::NavRight)
    }
}
