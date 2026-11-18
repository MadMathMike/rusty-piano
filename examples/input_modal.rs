//! Opens a modal with a keypress, lets you type into a `tui-input` field, and
//! hands the submitted value back to the top-level `App`.
//!
//! Run with: `cargo run --example input_modal`
//!
//! The structure mirrors `src/app.rs`: an input thread forwards key events over
//! an mpsc channel, and the main loop draws a frame and then handles one event.

use anyhow::Result;
use crossterm::event::{self as ct_event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, List, Paragraph};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

enum Event {
    Input(KeyEvent),
}

fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    // Restore the terminal even if the app returned an error.
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> Result<()> {
    let mut app = App::new();

    let tx = app.clone_sender();
    thread::spawn(move || {
        loop {
            if let Ok(ct_event::Event::Key(key)) = ct_event::read() {
                // The receiver is gone once the app exits, so stop reading.
                if tx.send(Event::Input(key)).is_err() {
                    break;
                }
            }
        }
    });

    while !app.exit {
        terminal.draw(|frame| app.draw(frame))?;
        app.handle_next_event()?;
        thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Top-level app
// ---------------------------------------------------------------------------

struct App {
    exit: bool,
    channel: (Sender<Event>, Receiver<Event>),
    /// `Some` while the modal is open. Dropping it discards whatever was typed.
    modal: Option<InputModal>,
    /// Values handed back from the modal.
    submissions: Vec<String>,
    error: String,
}

impl App {
    fn new() -> Self {
        App {
            exit: false,
            channel: mpsc::channel(),
            modal: None,
            submissions: Vec::new(),
            error: String::new(),
        }
    }

    fn clone_sender(&self) -> Sender<Event> {
        self.channel.0.clone()
    }

    fn draw(&self, frame: &mut Frame) {
        let [header, body, footer, error] = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .areas(frame.area());

        frame.render_widget(Line::from("input modal example").centered(), header);

        let [left, right] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(body);

        let submissions = List::new(self.submissions.iter().map(String::as_str))
            .block(Block::bordered().title("Submissions"));
        frame.render_widget(submissions, left);
        frame.render_widget(Block::bordered().title("Right pane"), right);

        let help = match self.modal {
            Some(_) => "type to edit | '↵' submit | 'esc' cancel",
            None => "'i' open input | 'q' quit",
        };
        frame.render_widget(Line::from(help).centered(), footer);
        frame.render_widget(Line::from(self.error.as_str()), error);

        // Draw the overlay last so it paints over the body.
        if let Some(modal) = &self.modal {
            let popup = InputModal::area(body);
            frame.render_widget(modal, popup);
            // Only `Frame` can place the terminal cursor, so the modal reports
            // where it goes and the top level sets it.
            frame.set_cursor_position(modal.cursor_position(popup));
        }
    }

    fn handle_next_event(&mut self) -> Result<()> {
        match self.channel.1.try_recv() {
            Ok(Event::Input(key)) => self.on_key_event(key)?,
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.exit = true,
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        // While the modal is open it gets every key, so 'q' types a 'q' and
        // doesn't quit the app.
        if let Some(modal) = &mut self.modal {
            match modal.handle_key(key) {
                ModalAction::None => {}
                ModalAction::Cancel => self.modal = None,
                ModalAction::Submit(value) => {
                    // The modal's value arrives here, back at the top level.
                    self.error.clear();
                    self.submissions.push(value);
                    self.modal = None;
                }
                ModalAction::Invalid(message) => self.error = message,
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('i') => self.modal = Some(InputModal::new("Name")),
            KeyCode::Char('q') => self.exit = true,
            _ => (),
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Modal
// ---------------------------------------------------------------------------

/// What the modal wants the app to do after handling a key.
enum ModalAction {
    None,
    Cancel,
    Submit(String),
    Invalid(String),
}

struct InputModal {
    title: &'static str,
    input: Input,
}

impl InputModal {
    fn new(title: &'static str) -> Self {
        InputModal {
            title,
            input: Input::default(),
        }
    }

    /// The modal never touches `App`. It returns an action and lets the app
    /// decide what to do with it.
    fn handle_key(&mut self, key: KeyEvent) -> ModalAction {
        match key.code {
            KeyCode::Esc => ModalAction::Cancel,
            KeyCode::Enter if self.input.value().trim().is_empty() => {
                ModalAction::Invalid("input can't be empty".to_owned())
            }
            KeyCode::Enter => ModalAction::Submit(self.input.value_and_reset()),
            _ => {
                // tui-input handles characters, backspace/delete, arrows,
                // home/end, ctrl+w, and so on.
                self.input.handle_event(&ct_event::Event::Key(key));
                ModalAction::None
            }
        }
    }

    /// Where the modal sits in its parent: 40 wide, 3 tall (border, text, border).
    fn area(parent: Rect) -> Rect {
        parent.centered(Constraint::Length(40), Constraint::Length(3))
    }

    fn block(&self) -> Block<'_> {
        Block::bordered()
            .title(self.title)
            .border_style(Style::new().yellow())
    }

    /// Horizontal scroll so the cursor stays visible when the text is wider
    /// than the box. Shared by `render` and `cursor_position` so the two agree.
    fn scroll(&self, area: Rect) -> usize {
        // Leave one column for the cursor after the last character.
        let width = self.block().inner(area).width.saturating_sub(1);
        self.input.visual_scroll(width as usize)
    }

    fn cursor_position(&self, area: Rect) -> Position {
        let inner = self.block().inner(area);
        let x = self.input.visual_cursor().saturating_sub(self.scroll(area));
        Position::new(inner.x + x as u16, inner.y)
    }
}

impl Widget for &InputModal {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Wipe whatever the body drew underneath, or it shows through.
        Clear.render(area, buf);

        Paragraph::new(self.input.value())
            .scroll((0, self.scroll(area) as u16))
            .block(self.block())
            .render(area, buf);
    }
}
