use std::{thread, time::Duration};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::{Block, Widget},
};

fn main() -> Result<()> {
    let mut terminal = ratatui::init();

    let mut app = App::default();

    loop {
        terminal.draw(|frame| app.draw(frame))?;

        // If this panics, how do I restore the terminal the terminal back to normal mode?
        if let crossterm::event::Event::Key(key_event) = crossterm::event::read().unwrap() {
            app.handle_key(key_event)?;
        }

        thread::sleep(Duration::from_millis(16));

        if app.exit {
            break;
        }
    }

    ratatui::restore();

    Ok(())
}

#[derive(Default)]
enum Tab {
    #[default]
    Bandcamp,
    Player,
}

#[derive(Default)]
struct App {
    tab: Tab,
    bandcamp: Bandcamp,
    player: Player,
    exit: bool,
}

impl App {
    fn draw(&mut self, frame: &mut Frame) {
        let [header, body, footer, error] = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(1), // header
                Constraint::Fill(1),   // body
                Constraint::Length(1), // footer
                Constraint::Length(1), // error
            ])
            .areas(frame.area());

        let buf = frame.buffer_mut();

        Line::from("rusty piano").render(header, buf);

        match self.tab {
            Tab::Bandcamp => self.bandcamp.render(body, buf),
            Tab::Player => self.player.render(body, buf),
        }

        Line::from("'tab' switch tabs | 'q' quit").render(footer, buf);
    }

    fn handle_key(&mut self, key_event: crossterm::event::KeyEvent) -> Result<()> {
        if key_event.kind != KeyEventKind::Press {
            return Ok(());
        }
        match key_event.code {
            KeyCode::Tab => match self.tab {
                Tab::Bandcamp => self.tab = Tab::Player,
                Tab::Player => self.tab = Tab::Bandcamp,
            },
            KeyCode::Char('q') => self.exit = true,
            _ => (),
        };

        Ok(())
    }
}

#[derive(Default)]
struct Bandcamp {}

impl Widget for &mut Bandcamp {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        Block::bordered().title("Bandcamp").render(area, buf);
    }
}

#[derive(Default)]
struct Player {}

impl Widget for &mut Player {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        Block::bordered().title("Player").render(area, buf);
    }
}
