use serde::{Deserialize, Serialize};
use strum::Display;

use crate::mode::Mode;

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
    UpdateMode(Mode),
    Save,
    Cancel,
    Edit,
    Select,
    Back,
    PageUp,
    PageDown,
    Focus,
    Toggle,
    Hide,
    FilterLeft,
    FilterRight,
    Next,
    LearnModeToggle,
}
