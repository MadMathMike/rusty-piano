use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::{Block, Paragraph, Widget},
};
use tui_input::{Input, backend::crossterm::EventHandler};

struct Credentials {
    pub username: String,
    pub password: String, // TODO: consider wrapping this in "secret" so we don't accidentally log it anywhere
}

pub enum Action {
    None,
    Close,
    Submit(Credentials),
    // Invalid(String), // A way to report empty inputs so we don't submit
}

#[derive(Default, PartialEq)]
enum Focus {
    #[default]
    Username,
    Password,
}

// TODO: const values for footer and header

pub struct LoginModal {
    username_input: TextField,
    // TODO: look into how to toggle this between hidden password entry (e.g., ******) and plain text (e.g., myP@ssword!)
    password_input: TextField,
    focus: Focus,
}

impl LoginModal {
    pub fn new() -> Self {
        Self {
            username_input: TextField::new("Username"),
            password_input: TextField::new("Password"),
            focus: Focus::Username,
        }
    }

    pub fn area(parent: Rect) -> Rect {
        parent.centered(
            ratatui::layout::Constraint::Length(40),
            ratatui::layout::Constraint::Length(6),
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::Close,
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Password => Focus::Username,
                    Focus::Username => Focus::Password,
                };
                Action::None
            }
            KeyCode::Enter if self.focus == Focus::Password => Action::Submit(Credentials {
                username: self.username_input.value().to_owned(),
                password: self.password_input.value().to_owned(),
            }),
            KeyCode::Enter => {
                self.focus = Focus::Password;
                Action::None
            }
            _ => {
                match self.focus {
                    Focus::Username => self.username_input.handle_key(key),
                    Focus::Password => self.password_input.handle_key(key),
                }
                Action::None
            }
        }
    }
}

struct TextField {
    title: &'static str,
    input: Input,
}

impl TextField {
    pub fn new(title: &'static str) -> Self {
        Self {
            title,
            input: Input::default(),
        }
    }

    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Do not pass navigation key presses to this handler.
    /// It should only handle text to be treated as the input value.
    pub fn handle_key(&mut self, key: KeyEvent) {
        self.input.handle_event(&Event::Key(key));
    }

    fn scroll(&self, area: Rect) -> usize {
        let width = Block::bordered().inner(area).width.saturating_sub(1);
        self.input.visual_scroll(width as usize)
    }

    pub fn cursor_position(&self, area: Rect) -> Position {
        let inner = Block::bordered().inner(area);
        let x_pos = self.input.visual_cursor().saturating_sub(self.scroll(area));
        Position {
            x: inner.x + x_pos as u16,
            y: inner.y,
        }
    }
}

impl Widget for &TextField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.input.value())
            .scroll((0, self.scroll(area) as u16))
            .block(Block::bordered().title(self.title))
            .render(area, buf);
    }
}
