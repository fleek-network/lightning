use std::net::Ipv4Addr;

use anyhow::{anyhow, bail, Result};
use crossterm::event::KeyEvent;
use ipnet::Ipv4Net;
use lightning_guard::map::PacketFilterRule;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};

use super::{Component, Frame};
use crate::action::Action;
use crate::components::utils::InputField;
use crate::components::{self};
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;

const IP_FIELD_NAME: &str = "IP";
const PORT_FIELD_NAME: &str = "Port";
const PROTO_FIELD_NAME: &str = "Protocol";
const ACTION_FIELD_NAME: &str = "Action";
const INPUT_FORM_X: u16 = 28;
const INPUT_FORM_Y: u16 = 14;
const INPUT_FIELD_COUNT: usize = 4;

#[derive(Default)]
pub struct FirewallForm {
    command_tx: Option<UnboundedSender<Action>>,
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<PacketFilterRule>,
    config: Config,
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
        components::utils::activate(&mut input_fields[0]);
        components::utils::inactivate(&mut input_fields[1]);
        components::utils::inactivate(&mut input_fields[2]);
        components::utils::inactivate(&mut input_fields[3]);

        Self {
            command_tx: None,
            input_fields,
            selected_input_field: 0,
            buf: None,
            config: Config::default(),
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

    fn yank_input(&mut self) -> Option<PacketFilterRule> {
        self.buf.take()
    }
}

impl Component for FirewallForm {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        self.selected_field().area.input(Input::from(key));
        Ok(None)
    }

    fn update(&mut self, action: Action, ctx: &mut State) -> Result<Option<Action>> {
        match action {
            Action::Cancel => {
                self.clear_input();
                Ok(Some(Action::UpdateMode(Mode::FirewallEdit)))
            },
            Action::Add => {
                if let Err(e) = self.update_filters_from_input() {
                    Ok(Some(Action::Error(e.to_string())))
                } else {
                    let filter = self
                        .yank_input()
                        .expect("We already verified that the input is valid");
                    ctx.add_filter(filter);
                    Ok(Some(Action::UpdateMode(Mode::FirewallEdit)))
                }
            },
            Action::Up => {
                if self.selected_input_field > 0 {
                    self.selected_input_field -= 1;
                }
                Ok(Some(Action::Render))
            },
            Action::Down => {
                if self.selected_input_field < self.input_fields.len() - 1 {
                    self.selected_input_field += 1;
                }
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
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
                components::utils::activate(textarea);
            } else {
                components::utils::inactivate(textarea)
            }
            f.render_widget(&textarea.area, *chunk);
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
