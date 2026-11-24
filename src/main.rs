use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, text::Line, widgets::Widget};
use std::io::Result;

fn main() -> Result<()> {
    let terminal = ratatui::init();
    let mut app = App { exit: false };
    let result = app.run(terminal);
    ratatui::restore();
    result
}

struct App {
    exit: bool,
}

impl App {
    fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| frame.render_widget(&*self, frame.area()))?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    event::KeyCode::Char(char) => {
                        if char == 'q' {
                            self.exit = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        Line::from("rusty-piano").render(area, buf);
    }
}
