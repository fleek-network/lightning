use anyhow::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;

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

pub trait Extractor<C, T>
where
    Self: for<'a> Fn(&'a mut C) -> &'a mut T,
    T: ?Sized,
{
    fn get<'b>(&self, context: &'b mut C) -> &'b mut T {
        (self)(context)
    }
}

impl<C, T, F> Extractor<C, T> for F
where
    F: for<'a> Fn(&'a mut C) -> &'a mut T,
    T: ?Sized,
{}

pub trait TryExtractor<C, T>
where
    Self: for<'a> Fn(&'a mut C) -> Result<&'a mut T>,
    T: ?Sized,
{
    fn get<'b>(&self, context: &'b mut C) -> Result<&'b mut T> {
        (self)(context)
    }
}

impl<C, T, F> TryExtractor<C, T> for F
where
    F: for<'a> Fn(&'a mut C) -> Result<&'a mut T>,
    T: ?Sized,
{}

/// A type that can extract a value from a context.
pub type DynExtractor<C, T> = Box<dyn Extractor<C, T>>;
pub type TryDynExtractor<C, T> = Box<dyn TryExtractor<C, T>>;

/// A type which can be drawn in the tui.
/// Think of this type as the base unit of what can be drawn.
///
/// Its very typical for a component to need access to shared state, thats why there is a `Context`
/// type.
///
/// An example of a type that could implement this trait is [test::Paginated].
pub trait Draw {
    type Context;

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
    fn draw(&mut self, context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}

/// `Component` is a trait that represents a visual and interactive element of the user interface.
/// Implementors of this trait can be registered with the main application loop and will be able to
/// receive events, update state, and be rendered on the screen.
///
/// These elements must implement [`Component::register_keybindings`], which takes complete control
/// of the app[ications incoming key events]
pub trait Component: Draw {
    /// The unique identifier of the component.
    /// Registered with the main application loop.
    /// This ID will be displayed in the navigator.
    fn component_name(&self) -> &'static str;

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

    /// Called on each tick of the TUI
    ///
    /// This could be useful for animations or other time-based updates.
    fn tick(&mut self) -> Result<()> {
        Ok(())
    }

    /// Register the keybindings from config
    ///
    /// Best practice is to use [crate::config::parse_actions] and
    /// store the actions as an enum.
    ///
    /// # Note
    /// This should be called in the beginning of the
    /// application lifecycle.
    ///
    /// ### todo
    /// - return result
    fn register_keybindings(&mut self, config: &Config);

    /// The main entry point for updating the components state.
    ///
    /// Before calling this method, the [Component::is_known_event] method should be called
    /// to determine if the compomenet cares about this event.
    ///
    /// Events are lists because there may be multikey combinations.   
    /// Ok(None) should be returned if there was changes made to any state.
    fn handle_event(
        &mut self,
        context: &mut Self::Context,
        event: &[KeyEvent],
    ) -> Result<Option<GlobalAction>>;
}
