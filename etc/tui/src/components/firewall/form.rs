use std::net::Ipv4Addr;

use anyhow::{anyhow, bail, Result};
use crossterm::event::KeyEvent;
use ipnet::Ipv4Net;
use lightning_guard::map::PacketFilterRule;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;
use tui_textarea::TextArea;

use super::{Component, FirewallContext, Frame};
use crate::app::GlobalAction;
use crate::components::Draw;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::utils;
use crate::widgets::utils::InputField;

const IP_FIELD_NAME: &str = "IP";
const PORT_FIELD_NAME: &str = "Port";
const PROTO_FIELD_NAME: &str = "Protocol";
const ACTION_FIELD_NAME: &str = "Action";
const INPUT_FORM_X: u16 = 28;
const INPUT_FORM_Y: u16 = 14;
const INPUT_FIELD_COUNT: usize = 4;

pub enum FirewallFormAction {
    Cancel,
    Add,
    Down,
    Quit,
    Suspend
}

impl std::str::FromStr for FirewallFormAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Cancel" => Ok(FirewallFormAction::Cancel),
            "Add" => Ok(FirewallFormAction::Add),
            "Down" => Ok(FirewallFormAction::Down),
            "Quit" => Ok(FirewallFormAction::Quit),
            "Suspend" => Ok(FirewallFormAction::Suspend),
            _ => Err(anyhow!("Invalid FirewallFormAction {s}")),
        }
    }
}


#[derive(Default)]
pub struct FirewallForm {
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<PacketFilterRule>,
    keybindings: ComponentKeyBindings<FirewallFormAction>,
}

impl FirewallForm {
    pub fn new() -> Self {
        let mut input_fields: Vec<_> = vec![
            (IP_FIELD_NAME, TextArea::default()),
            (PORT_FIELD_NAME, TextArea::default()),
            (PROTO_FIELD_NAME, TextArea::default()),
            (ACTION_FIELD_NAME, TextArea::default()),
        ]
        .into_iter()
        .map(|(title, area)| InputField { title, area })
        .collect();

        debug_assert!(input_fields.len() == INPUT_FIELD_COUNT);
        utils::activate(&mut input_fields[0]);
        utils::inactivate(&mut input_fields[1]);
        utils::inactivate(&mut input_fields[2]);
        utils::inactivate(&mut input_fields[3]);

        Self {
            input_fields,
            selected_input_field: 0,
            buf: None,
            keybindings: ComponentKeyBindings::default(),
        }
    }

    fn selected_field(&mut self) -> &mut InputField {
        &mut self.input_fields[self.selected_input_field]
    }

    fn clear_input(&mut self) {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
            field.area.yank_text();
        }
    }

    fn update_filters_from_input(&mut self) -> Result<()> {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
        }

        let (ip, prefix): (Ipv4Addr, Option<u32>) = {
            let input = self.input_fields[0].area.yank_text().trim().to_string();

            match input.parse::<Ipv4Addr>() {
                Ok(ip) => (ip, None),
                Err(_) => {
                    let ip_net = input
                        .parse::<Ipv4Net>()
                        .map_err(|e| anyhow!("Invalid IP: {e:?}"))?;
                    (ip_net.network(), Some(ip_net.prefix_len() as u32))
                },
            }
        };
        let port: u16 = self.input_fields[1]
            .area
            .yank_text()
            .trim()
            .parse()
            .map_err(|e| anyhow!("Invalid port: {e:?}"))?;
        let proto = match self.input_fields[2]
            .area
            .yank_text()
            .trim()
            .to_lowercase()
            .as_str()
        {
            "tcp" => PacketFilterRule::TCP,
            "udp" => PacketFilterRule::UDP,
            "any" => PacketFilterRule::ANY_PROTO,
            _ => bail!("Invalid protocol"),
        };
        let action = match self.input_fields[3]
            .area
            .yank_text()
            .trim()
            .to_lowercase()
            .as_str()
        {
            "pass" => PacketFilterRule::PASS,
            "drop" => PacketFilterRule::DROP,
            _ => bail!("Invalid action"),
        };
        let rule = PacketFilterRule {
            prefix: prefix.unwrap_or(PacketFilterRule::DEFAULT_PREFIX),
            ip,
            port,
            shortlived: false,
            proto,
            audit: true,
            action,
        };

        self.buf.replace(rule);

        Ok(())
    }

    pub fn yank_input(&mut self) -> Option<PacketFilterRule> {
        self.buf.take()
    }
}

impl Component for FirewallForm {
    /// The unique identifier of the component. 
    /// Registered with the main application loop.
    /// This ID will be displayed in the navigator.
    fn component_name(&self) -> &'static str {
        "FirewallForm"
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
    fn register_keybindings(&mut self, config: &Config) {
        self.keybindings = crate::config::parse_actions(&config.keybindings[self.component_name()]);
    }

    /// Check if this event is to be handled by this component
    /// 
    /// # Note
    /// The events are lists because there may be multikey combinations.
    fn is_known_event(
        &self,
        event: &[KeyEvent],
    ) -> bool {
        // this may be incoming text or a key event
        true
    }

    /// The main entry point for updating the components state.
    /// 
    /// Before calling this method, the [Component::is_known_event] method should be called
    /// to determine if the compomenet cares about this event.
    /// 
    /// Events are lists because there may be multikey combinations.    
    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[KeyEvent],
    ) -> Result<Option<GlobalAction>> {
        if event.len() == 1 {
            self.selected_field().area.input(event[0]);
        }

        match self.keybindings.get(event) {
            Some(FirewallFormAction::Cancel) => {
                self.clear_input();
                context.mounted = super::FirewallMounted::Edit;
            },
            Some(FirewallFormAction::Add) => {
               self.update_filters_from_input()?;
               context.mounted = super::FirewallMounted::Edit;
            },
            Some(FirewallFormAction::Down) => {
                if self.selected_input_field < self.input_fields.len() - 1 {
                    self.selected_input_field += 1;
                }
            },
            Some(FirewallFormAction::Quit) => {
                return Ok(Some(GlobalAction::Quit))
            },
            _ => {},
        };

        Ok(Some(GlobalAction::Render))
    }

}

impl Draw for FirewallForm {
    type Context = FirewallContext;

    fn draw(&mut self, context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Display form to enter new rule.
        debug_assert!(self.input_fields.len() == INPUT_FIELD_COUNT);

        f.render_widget(Clear, area);
        let area = center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Fill(1),
                    Constraint::Min(3),
                    Constraint::Min(3),
                    Constraint::Min(3),
                    Constraint::Min(3),
                    Constraint::Fill(1),
                ]
                .as_ref(),
            )
            .split(area);

        for (i, (textarea, chunk)) in self
            .input_fields
            .iter_mut()
            // We don't want the first or last because they're for padding.
            .zip(chunks.iter().take(5).skip(1))
            .enumerate()
        {
            if i == self.selected_input_field {
                utils::activate(textarea);
            } else {
                utils::inactivate(textarea)
            }
            let widget = textarea.area.widget();
            f.render_widget(widget, *chunk);
        }

        Ok(())
    }
}

pub fn center_form(x: u16, y: u16, r: Rect) -> Rect {
    let popup_layout =
        Layout::vertical([Constraint::Fill(1), Constraint::Min(y), Constraint::Fill(1)]).split(r);

    Layout::horizontal([Constraint::Fill(1), Constraint::Min(x), Constraint::Fill(1)])
        .split(popup_layout[1])[1]
}
