use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ebpf_service::map::{PacketFilterRule, Profile};
use ebpf_service::ConfigSource;
use log::error;
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;

const COLUMN_COUNT: usize = 6;

#[derive(Default)]
pub struct ProfileTable {
    command_tx: Option<UnboundedSender<Action>>,
    profiles: Vec<Profile>,
    src: ConfigSource,
    // Table widget for displaying records.
    longest_item_per_column: [u16; COLUMN_COUNT],
    table_state: ListState,
    config: Config,
}

impl ProfileTable {
    pub fn new(src: ConfigSource) -> Self {
        let profiles = vec![
            Profile {
                name: Some("~/.lightning/keystore/consensus.pem".try_into().unwrap()),
                file_rules: Vec::new(),
                audit: false,
            },
            Profile {
                name: Some("~/.lightning/keystore/node.pem".try_into().unwrap()),
                file_rules: Vec::new(),
                audit: false,
            },
        ];
        Self {
            profiles,
            src,
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table_state: ListState::default().with_selected(Some(0)),
            config: Config::default(),
        }
    }

    pub async fn read_state_from_storage(&mut self) -> Result<()> {
        // If it's an error, there is no file and thus there is nothing to do.
        if let Ok(profiles) = self
            .src
            .read_profiles()
            .await
            .map_err(|e| Report::msg(e.to_string()))
        {
            self.profiles = profiles;
        }

        Ok(())
    }

    fn scroll_up(&mut self) {
        if let Some(cur) = self.table_state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.table_state.select(Some(cur));
            }
        }
    }

    fn scroll_down(&mut self) {
        if let Some(cur) = self.table_state.selected() {
            let len = self.profiles.len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.table_state.select(Some(cur));
            }
        }
    }

    fn remove_profile(&mut self) {
        if let Some(cur) = self.table_state.selected() {
            debug_assert!(cur < self.profiles.len());
            self.profiles.remove(cur);

            if self.profiles.is_empty() {
                self.table_state.select(None);
            } else if cur == self.profiles.len() {
                self.table_state.select(Some(cur - 1));
            } else if cur == 0 {
                self.table_state.select(Some(1));
            }
        }
    }

    fn update_storage(&self) {
        let command_tx = self
            .command_tx
            .clone()
            .expect("Component always has a sender");
        let storage = self.src.clone();
        let new = self.profiles.clone().into_iter().collect::<Vec<_>>();
        tokio::spawn(async move {
            if let Err(e) = storage.write_profiles(new).await {
                let _ = command_tx.send(Action::Error(e.to_string()));
            }
        });
    }
}

impl Component for ProfileTable {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Save => Ok(None),
            Action::Cancel => Ok(None),
            Action::Add => Ok(None),
            Action::Remove => Ok(None),
            Action::Up => {
                self.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.scroll_down();
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Display profiles.
        let chunks = Layout::horizontal([Constraint::Percentage(100)]).split(area);
        let profiles = self
            .profiles
            .iter()
            .map(|p| p.name.as_ref().unwrap().display().to_string())
            .collect::<Vec<_>>();
        let profiles = List::new(profiles)
            .block(Block::default().borders(Borders::ALL).title("Profiles"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        f.render_stateful_widget(profiles, chunks[0], &mut self.table_state);

        Ok(())
    }
}
