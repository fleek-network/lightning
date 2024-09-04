use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::map;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};

use crate::action::Action;
use crate::components::Component;
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;
use crate::tui::Frame;
use crate::widgets::utils;
use crate::widgets::utils::InputField;

const PROFILE_NAME: &str = "Profile Name";
const INPUT_FORM_X: u16 = 20;
const INPUT_FORM_Y: u16 = 40;
const PROFILE_NAME_INPUT_FIELD_COUNT: usize = 1;

#[derive(Default)]
pub struct ProfileForm {
    command_tx: Option<UnboundedSender<Action>>,
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<map::Profile>,
    config: Config,
}

impl ProfileForm {
    pub fn new() -> Self {
        let input_fields: Vec<_> = vec![(PROFILE_NAME, TextArea::default())]
            .into_iter()
            .map(|(title, area)| InputField { title, area })
            .collect();

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

    fn update_profile_from_input(&mut self) -> Result<()> {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
        }

        let name = self.input_fields[0].area.yank_text().trim().to_string();
        let profile = map::Profile {
            name: Some(name.as_str().into()),
            ..Default::default()
        };
        self.buf.replace(profile);

        Ok(())
    }

    pub fn yank_input(&mut self) -> Option<map::Profile> {
        self.buf.take()
    }
}

impl Component for ProfileForm {
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
                Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
            },
            Action::Add => {
                if let Err(e) = self.update_profile_from_input() {
                    Ok(Some(Action::Error(e.to_string())))
                } else {
                    let profile = self
                        .yank_input()
                        .expect("We already verified that the input is valid");
                    ctx.add_profile(profile);
                    Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
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
        debug_assert!(self.input_fields.len() == PROFILE_NAME_INPUT_FIELD_COUNT);

        f.render_widget(Clear, area);
        let area = utils::center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(0),
                    Constraint::Length(3),
                    Constraint::Percentage(0),
                ]
                .as_ref(),
            )
            .split(area);

        for (i, (textarea, chunk)) in self
            .input_fields
            .iter_mut()
            // We don't want the first or last because they're for padding.
            .zip(chunks.iter().take(3).skip(1))
            .enumerate()
        {
            if i == self.selected_input_field {
                utils::activate(textarea);
            } else {
                utils::inactivate(textarea)
            }
            f.render_widget(&textarea.area, *chunk);
        }

        Ok(())
    }
}
