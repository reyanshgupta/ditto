use std::{collections::HashMap, env};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{
    launch::{self, AuthOperation, AuthStatus, Tool},
    profile::{Profile, Store},
};

const DITTO_PURPLE: Color = Color::Rgb(190, 134, 255);
const CLAUDE_ORANGE: Color = Color::Rgb(222, 133, 93);
const CODEX_GREEN: Color = Color::Rgb(104, 201, 154);

pub enum UiAction {
    Launch {
        tool: Tool,
        profile: Profile,
    },
    Authenticate {
        operation: AuthOperation,
        tool: Tool,
        profile: Profile,
    },
}

enum Mode {
    Browsing,
    Creating {
        input: String,
        error: Option<String>,
    },
    ChoosingTool {
        operation: AuthOperation,
    },
    ConfirmingLogout {
        tool: Tool,
    },
}

#[derive(Clone, Copy)]
struct ProfileAuth {
    claude: AuthStatus,
    codex: AuthStatus,
}

struct App<'a> {
    store: &'a Store,
    profiles: Vec<Profile>,
    selected: usize,
    mode: Mode,
    auth: HashMap<String, ProfileAuth>,
    has_auth_environment: bool,
}

impl<'a> App<'a> {
    fn new(store: &'a Store, profiles: Vec<Profile>, initial_profile: Option<&str>) -> Self {
        let selected = initial_profile
            .and_then(|name| profiles.iter().position(|profile| profile.name == name))
            .unwrap_or(0);
        let mut app = Self {
            store,
            profiles,
            selected,
            mode: Mode::Browsing,
            auth: HashMap::new(),
            has_auth_environment: auth_environment_is_set(),
        };
        app.refresh_selected_auth();
        app
    }

    fn selected_profile(&self) -> &Profile {
        &self.profiles[self.selected]
    }

    fn selected_auth(&self) -> ProfileAuth {
        self.auth[&self.selected_profile().name]
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.load_selected_auth();
    }

    fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.profiles.len() - 1);
        self.load_selected_auth();
    }

    fn load_selected_auth(&mut self) {
        if !self.auth.contains_key(&self.selected_profile().name) {
            self.refresh_selected_auth();
        }
    }

    fn refresh_selected_auth(&mut self) {
        let profile = self.selected_profile();
        let status = ProfileAuth {
            claude: launch::auth_status(Tool::Claude, profile),
            codex: launch::auth_status(Tool::Codex, profile),
        };
        self.auth.insert(profile.name.clone(), status);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<Action> {
        if key.kind != KeyEventKind::Press {
            return Ok(Action::Continue);
        }

        match &mut self.mode {
            Mode::Browsing => Ok(match key.code {
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_up();
                    Action::Continue
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_down();
                    Action::Continue
                }
                KeyCode::Char('n') => {
                    self.mode = Mode::Creating {
                        input: String::new(),
                        error: None,
                    };
                    Action::Continue
                }
                KeyCode::Char('l') => {
                    self.mode = Mode::ChoosingTool {
                        operation: AuthOperation::Login,
                    };
                    Action::Continue
                }
                KeyCode::Char('o') => {
                    self.mode = Mode::ChoosingTool {
                        operation: AuthOperation::Logout,
                    };
                    Action::Continue
                }
                KeyCode::Char('r') => {
                    self.refresh_selected_auth();
                    Action::Continue
                }
                KeyCode::Char('c') => Action::Launch(Tool::Claude),
                KeyCode::Char('x') => Action::Launch(Tool::Codex),
                _ => Action::Continue,
            }),
            Mode::Creating { input, error } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Browsing;
                    Ok(Action::Continue)
                }
                KeyCode::Enter => match self.store.create_profile(input) {
                    Ok(profile) => {
                        self.profiles = self.store.list_profiles()?;
                        self.selected = self
                            .profiles
                            .iter()
                            .position(|candidate| candidate.name == profile.name)
                            .unwrap_or(0);
                        self.refresh_selected_auth();
                        self.mode = Mode::Browsing;
                        Ok(Action::Continue)
                    }
                    Err(create_error) => {
                        *error = Some(create_error.to_string());
                        Ok(Action::Continue)
                    }
                },
                KeyCode::Backspace => {
                    input.pop();
                    *error = None;
                    Ok(Action::Continue)
                }
                KeyCode::Char(character) if input.len() < 32 => {
                    input.push(character);
                    *error = None;
                    Ok(Action::Continue)
                }
                _ => Ok(Action::Continue),
            },
            Mode::ChoosingTool { operation } => {
                let operation = *operation;
                let tool = match key.code {
                    KeyCode::Esc => {
                        self.mode = Mode::Browsing;
                        return Ok(Action::Continue);
                    }
                    KeyCode::Char('c') => Tool::Claude,
                    KeyCode::Char('x') => Tool::Codex,
                    _ => return Ok(Action::Continue),
                };
                if operation == AuthOperation::Logout {
                    self.mode = Mode::ConfirmingLogout { tool };
                    Ok(Action::Continue)
                } else {
                    Ok(Action::Authenticate { operation, tool })
                }
            }
            Mode::ConfirmingLogout { tool } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Ok(Action::Authenticate {
                    operation: AuthOperation::Logout,
                    tool: *tool,
                }),
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browsing;
                    Ok(Action::Continue)
                }
                _ => Ok(Action::Continue),
            },
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let footer_height = if self.has_auth_environment { 6 } else { 5 };
        let sections = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(footer_height),
        ])
        .split(area);

        self.draw_header(frame, sections[0]);
        self.draw_profiles(frame, sections[1]);
        self.draw_footer(frame, sections[2]);
        self.draw_modal(frame, area);
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Ditto CLI", Style::new().fg(DITTO_PURPLE).bold()),
            Span::raw("  choose a profile, then a tool"),
        ]))
        .alignment(Alignment::Center)
        .block(Block::bordered().border_style(Style::new().fg(DITTO_PURPLE)));
        frame.render_widget(header, area);
    }

    fn draw_profiles(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::horizontal([Constraint::Percentage(32), Constraint::Percentage(68)])
            .split(area);
        let items = self.profiles.iter().map(|profile| {
            let suffix = if profile.managed { "" } else { "  existing" };
            ListItem::new(Line::from(vec![
                Span::raw(&profile.name),
                Span::styled(suffix, Style::new().fg(Color::DarkGray)),
            ]))
        });
        let profile_list = List::new(items)
            .block(Block::new().title(" Profiles ").borders(Borders::ALL))
            .highlight_symbol("› ")
            .highlight_style(
                Style::new()
                    .fg(Color::Black)
                    .bg(DITTO_PURPLE)
                    .add_modifier(Modifier::BOLD),
            );
        let mut list_state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(profile_list, columns[0], &mut list_state);

        let profile = self.selected_profile();
        let auth = self.selected_auth();
        let kind = if profile.managed {
            "Isolated profile"
        } else {
            "Your existing Claude and Codex setup"
        };
        let details = Text::from(vec![
            Line::from(vec![
                Span::styled(&profile.name, Style::new().fg(DITTO_PURPLE).bold()),
                Span::styled(format!("  {kind}"), Style::new().fg(Color::DarkGray)),
            ]),
            Line::default(),
            Line::styled("Authentication", Style::new().bold()),
            status_line(Tool::Claude, auth.claude),
            status_line(Tool::Codex, auth.codex),
            Line::default(),
            Line::styled("Profile directories", Style::new().bold()),
            Line::styled(
                format!("Claude  {}", profile.claude_home.display()),
                Style::new().fg(Color::DarkGray),
            ),
            Line::styled(
                format!("Codex   {}", profile.codex_home.display()),
                Style::new().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(details).wrap(Wrap { trim: false }).block(
                Block::new()
                    .title(" Selected profile ")
                    .borders(Borders::ALL),
            ),
            columns[1],
        );
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            shortcut_line(&[
                ("c", "open Claude", CLAUDE_ORANGE),
                ("x", "open Codex", CODEX_GREEN),
                ("l", "sign in", DITTO_PURPLE),
                ("o", "sign out", Color::Yellow),
            ]),
            shortcut_line(&[
                ("↑/↓", "select", DITTO_PURPLE),
                ("n", "new profile", Color::White),
                ("r", "refresh status", Color::White),
                ("q", "quit", Color::White),
            ]),
        ];
        if self.has_auth_environment {
            lines.push(Line::styled(
                "An API-key environment variable is set and may override the saved login.",
                Style::new().fg(Color::Yellow),
            ));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .block(Block::bordered()),
            area,
        );
    }

    fn draw_modal(&self, frame: &mut Frame, area: Rect) {
        match &self.mode {
            Mode::Browsing => {}
            Mode::Creating { input, error } => {
                let popup = centered_rect(64, 8, area);
                let mut lines = vec![
                    Line::raw("Use letters, numbers, '-' or '_'."),
                    Line::styled(format!("> {input}"), Style::new().fg(DITTO_PURPLE).bold()),
                    Line::default(),
                    Line::styled(
                        "Enter create  ·  Esc cancel",
                        Style::new().fg(Color::DarkGray),
                    ),
                ];
                if let Some(error) = error {
                    lines[2] = Line::styled(error, Style::new().fg(Color::Red));
                }
                render_popup(frame, popup, " New profile ", lines);
            }
            Mode::ChoosingTool { operation } => {
                let popup = centered_rect(58, 8, area);
                let profile = self.selected_profile();
                let lines = vec![
                    Line::raw(format!("{} to '{}' with:", operation.label(), profile.name)),
                    Line::default(),
                    shortcut_line(&[
                        ("c", "Claude Code", CLAUDE_ORANGE),
                        ("x", "Codex", CODEX_GREEN),
                    ]),
                    Line::default(),
                    Line::styled("Esc cancel", Style::new().fg(Color::DarkGray)),
                ];
                render_popup(frame, popup, &format!(" {} ", operation.label()), lines);
            }
            Mode::ConfirmingLogout { tool } => {
                let popup = centered_rect(58, 7, area);
                let lines = vec![
                    Line::raw(format!(
                        "Sign out of {} for '{}'?",
                        tool.label(),
                        self.selected_profile().name
                    )),
                    Line::default(),
                    Line::styled(
                        "Enter or y confirm  ·  n cancel",
                        Style::new().fg(Color::Yellow),
                    ),
                ];
                render_popup(frame, popup, " Confirm sign out ", lines);
            }
        }
    }
}

enum Action {
    Continue,
    Quit,
    Launch(Tool),
    Authenticate {
        operation: AuthOperation,
        tool: Tool,
    },
}

pub fn run(
    store: &Store,
    profiles: Vec<Profile>,
    initial_profile: Option<&str>,
) -> Result<Option<UiAction>> {
    let app = App::new(store, profiles, initial_profile);
    let mut terminal = ratatui::init();
    let guard = TerminalGuard;
    let result = run_loop(&mut terminal, app);
    drop(guard);
    result
}

fn run_loop(terminal: &mut DefaultTerminal, mut app: App<'_>) -> Result<Option<UiAction>> {
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        if let Event::Key(key) = event::read()? {
            let action = match app.handle_key(key)? {
                Action::Continue => continue,
                Action::Quit => return Ok(None),
                Action::Launch(tool) => UiAction::Launch {
                    tool,
                    profile: app.selected_profile().clone(),
                },
                Action::Authenticate { operation, tool } => UiAction::Authenticate {
                    operation,
                    tool,
                    profile: app.selected_profile().clone(),
                },
            };
            return Ok(Some(action));
        }
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn status_line(tool: Tool, status: AuthStatus) -> Line<'static> {
    let (symbol, label, color) = match status {
        AuthStatus::SignedIn => ("●", "Signed in", Color::Green),
        AuthStatus::SignedOut => ("○", "Sign in required", Color::Yellow),
        AuthStatus::Unavailable => ("?", "CLI or status unavailable", Color::Red),
    };
    let tool_color = match tool {
        Tool::Claude => CLAUDE_ORANGE,
        Tool::Codex => CODEX_GREEN,
    };
    Line::from(vec![
        Span::styled(format!("{:<13}", tool.label()), Style::new().fg(tool_color)),
        Span::styled(format!("{symbol} {label}"), Style::new().fg(color)),
    ])
}

fn shortcut_line(shortcuts: &[(&str, &str, Color)]) -> Line<'static> {
    let mut spans = Vec::with_capacity(shortcuts.len() * 2);
    for (index, (key, label, color)) in shortcuts.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("    "));
        }
        spans.push(Span::styled(
            (*key).to_owned(),
            Style::new().fg(*color).bold(),
        ));
        spans.push(Span::raw(format!(" {label}")));
    }
    Line::from(spans)
}

fn render_popup(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'_>>) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::new()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::new().fg(DITTO_PURPLE)),
        ),
        area,
    );
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .split(area)[0];
    Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .split(vertical)[0]
}

fn auth_environment_is_set() -> bool {
    [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "OPENAI_API_KEY",
    ]
    .iter()
    .any(|name| env::var_os(name).is_some())
}
