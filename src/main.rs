use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::{layout::{Constraint, Direction, Layout}, widgets::{Block, Borders, Paragraph}};

type Result = color_eyre::Result<()>;

fn main() -> Result {
    let mut terminal = ratatui::init(); // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)

    let mut app = App::default();

    app.run(&mut terminal)?;

    ratatui::restore(); // Returns the terminal back to normal mode
    
    Ok(()) 
}

#[derive(Debug, Default)]
pub struct App {
    exit: bool
}

impl App {
    fn run(&mut self, terminal: &mut ratatui::Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>) -> Result {
        
        while !self.exit {

            terminal.draw(|frame| {
                let outer_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints(vec![
                        Constraint::Length(1),
                        Constraint::Fill(1),
                        Constraint::Length(3),
                    ])
                    .split(frame.area());

                let body = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![
                        Constraint::Percentage(40),
                        Constraint::Percentage(60),
                    ])
                    .split(outer_layout[1]);
                    
                frame.render_widget("hello world", outer_layout[0]);
                frame.render_widget(
                    Paragraph::new("Left").block(Block::new().borders(Borders::ALL)),
                    body[0]
                );
                frame.render_widget(
                    Paragraph::new("Right").block(Block::new().borders(Borders::ALL)),
                    body[1]
                );
                frame.render_widget("woah", outer_layout[2]);
            })?;

            if let crossterm::event::Event::Key(key_event) = crossterm::event::read()? {
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Char('q') {
                    self.exit = true;
                }
            }
        }

        Ok(())
    }
}