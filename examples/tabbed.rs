use std::{thread, time::Duration};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::{Block, Widget},
};
use rusty_piano::secrets;

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

const REFRESH_TOKEN_KEY: &str = "bandcamp-refresh-token";

// Question: Given that many of these operations are best handled async,
// how is the top-level app supposed to track and drive open futures to completion?
// Does it still makes sense to delegate downloading to a separate "object"?
impl Bandcamp {
    fn init() -> Self {
        let secret_result = secrets::retrieve(REFRESH_TOKEN_KEY);

        if let Ok(refresh_token) = secret_result {
            // Get auth token from Bandcamp API using refresh token
        } else {
            // TODO: display error in UI or write to logs
        }

        // Set state as not-authenticated if any of the above fail

        // TODO: Init should probably also accept configuration for download location

        Bandcamp::default()
    }

    // TODO: handle key presses
    // open login modal
    // sync collection
    // navigate album list/grid
    // download album
    // play album

    // TODO: login
    // Return error or (auth_token, refresh_token)
    // Store refresh_token in credential store

    // TODO: sync collection
    // If auth token is expired, attempt token refresh

    // TODO: Downloading albums should retrieve updated download URLs if existing URLs return an error
    // If attempting to to retrieve updated download URLs fail because auth token is expired,
    // attempt refresh
}

impl Widget for &mut Bandcamp {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        // If not-authenticated
        // Show command to open login modal
        // Do not allow downloading

        // If authenticated
        // Show command to sync collection
        // Allow downloading albums

        // Items that are downloaded can be played

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
