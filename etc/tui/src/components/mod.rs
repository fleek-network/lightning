use std::str::FromStr;

use anyhow::{anyhow, Result};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::app::GlobalAction;
use crate::config::Config;
use crate::tui::{Event, Frame};

pub mod firewall;
pub mod home;
#[cfg(feature = "logger")]
pub mod logger;
pub mod navigator;
pub mod profile;
pub mod prompt;
pub mod summary;

pub trait Draw {
    /// Render the component on the screen. (REQUIRED)
    ///
    /// # Arguments
    ///
    /// * `f` - A frame used for rendering.
    /// * `area` - The area in which the component should be drawn.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - An Ok result or an error.
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}

/// `Component` is a trait that represents a visual and interactive element of the user interface.
/// Implementors of this trait can be registered with the main application loop and will be able to
/// receive events, update state, and be rendered on the screen.
pub trait Component: Draw {
    /// The unique identifier of the component. 
    /// Registered with the main application loop.
    /// This ID will be displayed in the navigator.
    fn component_name(&self) -> &'static str;

    /// The context type that this component uses
    type Context;

    /// Initialize the component with a specified area if necessary.
    ///
    /// # Arguments
    ///
    /// * `area` - Rectangular area to initialize the component within.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - An Ok result or an error.
    fn init(&mut self, _area: Rect) -> Result<()> {
        Ok(())
    }

    /// Called on each tick of the main application loop.
    ///
    /// This is useful if you have changing visuals
    ///
    /// todo: rename / is it useful? should this be called on the render timer insterad?
    fn tick(&mut self) -> Result<()> {
        Ok(())
    }

    /// An incoming key event, 
    /// 
    /// This is only useful for form components. Your main stateful logic should be handled
    /// in [`Component::handle_known_event`]
    fn incoming_event(&mut self, _event: KeyEvent) {
        Ok(())
    }

    /// Register the keybindings from config
    ///
    /// see [crate::config::parse_actions]
    #[must_use]
    fn register_keybindings(&mut self, config: &Config);

    fn is_known_event(
        &self,
        event: &[KeyEvent],
    ) -> bool;

    /// The main entry point for updating the components state.
    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[KeyEvent],
    ) -> Result<Option<GlobalAction>>;
}
