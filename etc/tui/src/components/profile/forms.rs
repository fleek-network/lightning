use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::map;
use lightning_guard::map::FileRule;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};

use super::{Component, Frame, ProfileContext};
use crate::components::Draw;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::utils;
use crate::widgets::utils::InputField;

const PROFILE_NAME: &str = "Profile Name";
const TARGET_NAME: &str = "Target";
const PERMISSIONS: &str = "Permissions";
const INPUT_FORM_X: u16 = 20;
const INPUT_FORM_Y: u16 = 40;
const PROFILE_NAME_INPUT_FIELD_COUNT: usize = 1;
const RULE_INPUT_FIELD_COUNT: usize = 2;

pub enum ProfileFormActions {
    Cancel,
    Add,
    Up,
    Down,
}

#[derive(Default)]
pub struct ProfileForm {
    rule_form: RuleForm,
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<map::Profile>,
    config: Config,
    keybindings: ComponentKeyBindings<ProfileFormActions>,
}

impl ProfileForm {
    pub fn new() -> Self {
        let input_fields: Vec<_> = vec![(PROFILE_NAME, TextArea::default())]
            .into_iter()
            .map(|(title, area)| InputField { title, area })
            .collect();

        Self {
            rule_form: RuleForm::default(),
            input_fields,
            selected_input_field: 0,
            buf: None,
            config: Config::default(),
            keybindings: Default::default(),
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
    type Context = ProfileContext;

    fn component_name(&self) -> &'static str {
        "ProfileForm"
    }

    fn incoming_event(&mut self, event: KeyEvent) {
        self.selected_field().area.input(Input::Event(event));
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.keybindings = crate::config::parse_actions(&config.keybindings[self.component_name()]);
    }

    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if let Some(action) = self.keybindings.get(event) {
            match action {
                ProfileFormActions::Add => {
                    if let Err(e) = self.update_profile_from_input() {
                        return Err(anyhow::anyhow!("creating profile failed {e}"));
                    } else {
                        Ok(Some(crate::app::GlobalAction::Render))
                    }
                },
                ProfileFormActions::Cancel => {
                    self.clear_input();
                    context.mounted = super::ProfileSubComponent::ProfileViewEdit.as_str();

                    Ok(Some(crate::app::GlobalAction::Render))
                },
                ProfileFormActions::Up => {
                    if self.selected_input_field > 0 {
                        self.selected_input_field -= 1;
                    }

                    Ok(Some(crate::app::GlobalAction::Render))
                },
                ProfileFormActions::Down => {
                    if self.selected_input_field < self.input_fields.len() - 1 {
                        self.selected_input_field += 1;
                    }

                    Ok(Some(crate::app::GlobalAction::Render))
                },
            }
        }
    }

    fn is_known_event(&self, event: &[KeyEvent]) -> bool {
        self.keybindings.contains_key(event)
    }

    // fn update(&mut self, action: Action) -> Result<Option<Action>> {
    //     match action {
    //         Action::Cancel => {
    //             self.clear_input();
    //             Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
    //         },
    //         Action::Add => {
    //             if let Err(e) = self.update_profile_from_input() {
    //                 Ok(Some(Action::Error(e.to_string())))
    //             } else {
    //                 // Todo: Here send new entry.
    //                 Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
    //             }
    //         },
    //         Action::Up => {
    //             if self.selected_input_field > 0 {
    //                 self.selected_input_field -= 1;
    //             }
    //             Ok(Some(Action::Render))
    //         },
    //         Action::Down => {
    //             if self.selected_input_field < self.input_fields.len() - 1 {
    //                 self.selected_input_field += 1;
    //             }
    //             Ok(Some(Action::Render))
    //         },
    //         _ => Ok(None),
    //     }
    // }
}

impl Draw for ProfileForm {
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
            let widget = textarea.area.widget();
            f.render_widget(widget, *chunk);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct RuleForm {
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<FileRule>,
    config: Config,
}

// impl RuleForm {
//     pub fn new() -> Self {
//         let mut input_fields: Vec<_> = vec![
//             (TARGET_NAME, TextArea::default()),
//             (PERMISSIONS, TextArea::default()),
//         ]
//         .into_iter()
//         .map(|(title, area)| InputField { title, area })
//         .collect();

//         debug_assert!(input_fields.len() == RULE_INPUT_FIELD_COUNT);
//         utils::activate(&mut input_fields[0]);
//         utils::inactivate(&mut input_fields[1]);

//         Self {
//             command_tx: None,
//             input_fields,
//             selected_input_field: 0,
//             buf: None,
//             config: Config::default(),
//         }
//     }

//     fn selected_field(&mut self) -> &mut InputField {
//         &mut self.input_fields[self.selected_input_field]
//     }

//     fn clear_input(&mut self) {
//         for field in self.input_fields.iter_mut() {
//             field.area.select_all();
//             field.area.cut();
//             field.area.yank_text();
//         }
//     }

//     fn update_filters_from_input(&mut self) -> Result<()> {
//         for field in self.input_fields.iter_mut() {
//             field.area.select_all();
//             field.area.cut();
//         }

//         let perm_input = self.input_fields[1].area.yank_text().trim().to_lowercase();
//         let mut permissions = FileRule::NO_OPERATION;
//         for op in perm_input.chars() {
//             if op == FileRule::OPEN_SYMBOL {
//                 permissions |= FileRule::OPEN_MASK;
//             }
//             if op == FileRule::READ_SYMBOL {
//                 permissions |= FileRule::READ_MASK;
//             }
//             if op == FileRule::WRITE_SYMBOL {
//                 permissions |= FileRule::WRITE_MASK;
//             }
//         }

//         self.buf.replace(FileRule {
//             file: self.input_fields[0].area.yank_text().trim().try_into()?,
//             permissions,
//         });

//         Ok(())
//     }

//     pub fn yank_input(&mut self) -> Option<FileRule> {
//         self.buf.take()
//     }
// }

// impl Component for RuleForm {
//     fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
//         self.command_tx = Some(tx);
//         Ok(())
//     }

//     fn register_config_handler(&mut self, config: Config) -> Result<()> {
//         self.config = config;
//         Ok(())
//     }

//     fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
//         self.selected_field().area.input(Input::from(key));
//         Ok(None)
//     }

//     fn update(&mut self, action: Action) -> Result<Option<Action>> {
//         match action {
//             Action::Cancel => {
//                 self.clear_input();
//                 Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit)))
//             },
//             Action::Add => {
//                 if let Err(e) = self.update_filters_from_input() {
//                     Ok(Some(Action::Error(e.to_string())))
//                 } else {
//                     Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit)))
//                 }
//             },
//             Action::Up => {
//                 if self.selected_input_field > 0 {
//                     self.selected_input_field -= 1;
//                 }
//                 Ok(Some(Action::Render))
//             },
//             Action::Down => {
//                 if self.selected_input_field < self.input_fields.len() - 1 {
//                     self.selected_input_field += 1;
//                 }
//                 Ok(Some(Action::Render))
//             },
//             _ => Ok(None),
//         }
//     }

//     fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
//         // Display form to enter new rule.
//         debug_assert!(self.input_fields.len() == RULE_INPUT_FIELD_COUNT);

//         f.render_widget(Clear, area);
//         let area = utils::center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

//         let chunks = Layout::default()
//             .direction(Direction::Vertical)
//             .constraints(
//                 [
//                     Constraint::Percentage(0),
//                     Constraint::Length(3),
//                     Constraint::Length(3),
//                     Constraint::Percentage(0),
//                 ]
//                 .as_ref(),
//             )
//             .split(area);

//         for (i, (textarea, chunk)) in self
//             .input_fields
//             .iter_mut()
//             // We don't want the first or last because they're for padding.
//             .zip(chunks.iter().take(3).skip(1))
//             .enumerate()
//         {
//             if i == self.selected_input_field {
//                 utils::activate(textarea);
//             } else {
//                 utils::inactivate(textarea)
//             }
//             let widget = textarea.area.widget();
//             f.render_widget(widget, *chunk);
//         }

//         Ok(())
//     }
// }
