pub mod form;

use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::map::PacketFilterRule;
use lightning_guard::ConfigSource;
use ratatui::prelude::{Color, Constraint, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Row};
use unicode_width::UnicodeWidthStr;

use super::{Component, Draw, Frame};
use crate::app::{ApplicationContext, GlobalAction};
use crate::components::firewall::form::FirewallForm;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::table::Table;

const COLUMN_COUNT: usize = 6;

enum FirewallMounted {
    Main,
    Edit,
    Form,
}

pub struct FirewallContext {
    mounted: FirewallMounted,
}

enum FirewallAction {
    Edit,
    Up,
    Down,
    NavLeft,
    NavRight,
    Quit,
}

impl std::str::FromStr for FirewallAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Edit" => Ok(FirewallAction::Edit),
            "Up" => Ok(FirewallAction::Up),
            "Down" => Ok(FirewallAction::Down),
            "NavLeft" => Ok(FirewallAction::NavLeft),
            "NavRight" => Ok(FirewallAction::NavRight),
            "Quit" => Ok(FirewallAction::Quit),
            _ => Err(anyhow::anyhow!("Invalid FirewallAction {s}")),
        }
    }
}

enum FirewallEditAction {
    Cancel,
    Save,
    Add,
    Remove,
    Up,
    Down,
}

impl std::str::FromStr for FirewallEditAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Cancel" => Ok(FirewallEditAction::Cancel),
            "Save" => Ok(FirewallEditAction::Save),
            "Add" => Ok(FirewallEditAction::Add),
            "Remove" => Ok(FirewallEditAction::Remove),
            "Up" => Ok(FirewallEditAction::Up),
            "Down" => Ok(FirewallEditAction::Down),
            _ => Err(anyhow::anyhow!("Invalid FirewallEditAction {s}")),
        }
    }
}

/// Component that displaying and managing packet-filter rules.
pub struct FireWall {
    table: Table<PacketFilterRule>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    form: FirewallForm,
    src: ConfigSource,
    config: Config,
    context: FirewallContext,
    main_keybindings: ComponentKeyBindings<FirewallAction>,
    edit_keybindings: ComponentKeyBindings<FirewallEditAction>,
}

impl FireWall {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            src,
            table: Table::new(),
            longest_item_per_column: [0; COLUMN_COUNT],
            form: FirewallForm::new(),
            config: Config::default(),
            context: FirewallContext {
                mounted: FirewallMounted::Main,
            },
            main_keybindings: ComponentKeyBindings::default(),
            edit_keybindings: ComponentKeyBindings::default(),
        }
    }

    pub async fn read_state_from_storage(&mut self) -> Result<()> {
        // If it's an error, there is no file and thus there is nothing to do.
        if let Ok(filters) = self.src.read_packet_filters().await {
            self.table.load_records(filters);
        }

        Ok(())
    }

    pub fn form(&mut self) -> &mut FirewallForm {
        &mut self.form
    }

    fn save(&mut self) {
        self.table.commit_changes();
        let config_src = self.src.clone();
        let new = self.table.records().cloned().collect::<Vec<_>>();
        tokio::spawn(async move {
            if let Err(e) = config_src.write_packet_filters(new).await {
                // todo where to write errors
                // let _ = command_tx.send(Action::Error(e.to_string()));
            }
        });
    }

    fn space_between_columns(&self) -> [u16; COLUMN_COUNT] {
        let prefix = self
            .table
            .records()
            .map(|r| r.prefix.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let ip_len = self
            .table
            .records()
            .map(|r| r.ip.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let port_len = self
            .table
            .records()
            .map(|r| r.port.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let proto_len = self
            .table
            .records()
            .map(|r| r.proto_str().as_str().width())
            .max()
            .unwrap_or(0);
        let trigger_event_len = self
            .table
            .records()
            .map(|r| r.audit.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let action_len = self
            .table
            .records()
            .map(|r| r.action_str().as_str().width())
            .max()
            .unwrap_or(0);

        [
            ip_len as u16,
            prefix as u16,
            port_len as u16,
            proto_len as u16,
            trigger_event_len as u16,
            action_len as u16,
        ]
    }
}

impl Component for FireWall {
    /// The unique identifier of the component.
    /// Registered with the main application loop.
    /// This ID will be displayed in the navigator.
    fn component_name(&self) -> &'static str {
        "Firewall"
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
        self.main_keybindings =
            crate::config::parse_actions(&config.keybindings[self.component_name()]);
        self.edit_keybindings = crate::config::parse_actions(&config.keybindings["FirewallEdit"]);

        // todo form
    }

    /// Check if this event is to be handled by this component
    ///
    /// # Note
    /// The events are lists because there may be multikey combinations.
    fn is_known_event(&self, event: &[KeyEvent]) -> bool {
        match self.context.mounted {
            FirewallMounted::Main => self.main_keybindings.get(event).is_some(),
            FirewallMounted::Edit => self.edit_keybindings.get(event).is_some(),
            _ => false,
            // FirewallMounted::Form => self.form.is_known_event(event),
        }
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
        match self.context.mounted {
            FirewallMounted::Main => {
                if let Some(action) = self.main_keybindings.get(event) {
                    match action {
                        FirewallAction::Edit => {
                            self.context.mounted = FirewallMounted::Edit;
                        }
                        FirewallAction::Up => {
                            self.table.scroll_up();
                        }
                        FirewallAction::Down => {
                            self.table.scroll_down();
                        }
                        FirewallAction::Quit => {
                            return Ok(Some(GlobalAction::Quit));
                        },
                        FirewallAction::NavLeft => {
                            context.nav_left();
                        },
                        FirewallAction::NavRight => {
                            context.nav_right();
                        },
                        _ => {}
                    }
                }
            }
            FirewallMounted::Edit => {
                if let Some(action) = self.edit_keybindings.get(event) {
                    match action {
                        FirewallEditAction::Cancel => {
                            self.table.restore_state();
                            self.context.mounted = FirewallMounted::Main;
                        }
                        FirewallEditAction::Save => {
                            self.save();
                            self.context.mounted = FirewallMounted::Main;
                        }
                        FirewallEditAction::Add => {
                            self.context.mounted = FirewallMounted::Form;
                        }
                        FirewallEditAction::Remove => {
                            self.table.remove_selected_record();
                        }
                        FirewallEditAction::Up => {
                            self.table.scroll_up();
                        }
                        FirewallEditAction::Down => {
                            self.table.scroll_down();
                        }
                    }
                }
            }
            FirewallMounted::Form => {
                return self.form.handle_known_event(&mut self.context, event);
            }
        }

        Ok(Some(GlobalAction::Render))
    }
}

impl Draw for FireWall {
    type Context = ApplicationContext;

    fn draw(&mut self, _context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.longest_item_per_column = self.space_between_columns();
        debug_assert!(self.longest_item_per_column.len() == COLUMN_COUNT);

        let column_names = ["IP", "Subnet", "Port", "Protocol", "Audit", "Action"];
        debug_assert!(column_names.len() == COLUMN_COUNT);

        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::DarkGray);
        let header = column_names
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style);

        let rows = self.table.records().map(|data| {
            let item = flatten_filter(data);
            item.into_iter()
                .map(|content| {
                    let text = Text::from(content);
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(Color::White).bg(Color::Black))
        });

        let contraints = [
            Constraint::Min(self.longest_item_per_column[0] + 1),
            Constraint::Min(self.longest_item_per_column[1] + 1),
            Constraint::Min(self.longest_item_per_column[2] + 1),
            Constraint::Min(self.longest_item_per_column[3] + 1),
            Constraint::Min(self.longest_item_per_column[4] + 1),
            Constraint::Min(self.longest_item_per_column[5]),
        ];
        debug_assert!(contraints.len() == COLUMN_COUNT);

        let bar = " > ";
        let table = ratatui::widgets::Table::new(rows, contraints)
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(bar));

        f.render_stateful_widget(table, area, self.table.state());

        Ok(())
    }
}

fn flatten_filter(filter: &PacketFilterRule) -> [String; COLUMN_COUNT] {
    [
        filter.ip.to_string(),
        filter.prefix.to_string(),
        filter.port.to_string(),
        filter.proto_str(),
        filter.audit.to_string(),
        filter.action_str(),
    ]
}
