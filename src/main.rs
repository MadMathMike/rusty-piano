use std::sync::mpsc;
use std::thread::{self, sleep};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, Paragraph},
};
use rusty_piano::player::{play_track, PlaybackCommands};
use rusty_piano::{bandcamp::Item, collection::read_collection};

fn main() -> Result<()> {
    // TODO: If there is a problem reading the collection (or the collection is empty), prompt authentication and collection caching
    let collection = read_collection();

    let mut terminal = ratatui::init(); // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)

    let mut app_state = AppState::new(collection);

    let (channel_tx, channel_rx) = mpsc::channel::<PlaybackCommands>();

    let player_thread = thread::spawn(move|| {
        // TODO: Make sure this can respond to events while music is playing
        // Or at least the exit event. Events that are sent to the channel 
        // do back up like a queue though (which makes sense).
        while let Ok(command) = channel_rx.recv()
        {
            // TODO: Question, how does polling rate affect usability and CPU resource consumption? Can I poll too fast?
            sleep(Duration::from_millis(100));
            match command {
                PlaybackCommands::Exit => break,
                // TODO: either the player or this thread will need to handle stopping currently playing songs to switch to the new song
                PlaybackCommands::Play(track) => play_track(&track),
            }
        }
    });

    // TODO: what happens if we error out before we can send the exit command to the player? 
    // Do we orphan a thread? Or does the process hang?
    while !app_state.exit {
        terminal.draw(|frame| {
            draw(frame, &mut app_state)
                .context("Failure drawing frame")
                .unwrap()
        })?;

        if let crossterm::event::Event::Key(key_event) = crossterm::event::read()? {
            if key_event.kind == KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Enter => {
                        if let Some(selected) = app_state.album_list_state.selected() {
                            if let Some(track) = app_state.collection.get(selected).map(|item|item.tracks.first().expect("Should not have any empty albums")){
                                channel_tx.send(PlaybackCommands::Play(track.clone()))?;
                            }
                        }
                    },
                    KeyCode::Up => {
                        app_state.album_list_state.scroll_up_by(1);
                    }
                    KeyCode::Down => {
                        app_state.album_list_state.scroll_down_by(1);
                    }
                    KeyCode::Char('q') => {
                        // TODO: can the OS cleanup abandoned threads?
                        channel_tx.send(PlaybackCommands::Exit)?;
                        app_state.exit = true
                    },
                    KeyCode::Char(_) => todo!(),
                    KeyCode::Null => todo!(),
                    // KeyCode::Esc => todo!(),
                    // KeyCode::Pause => todo!(),
                    // KeyCode::Media(media_key_code) => todo!(),
                    // KeyCode::Modifier(modifier_key_code) => todo!(),
                    _ => {}
                }
            }
        }
    }

    // TODO: send exit command
    // TODO: handle error result
    player_thread.join().expect("Error joining thread");

    ratatui::restore(); // Returns the terminal back to normal mode
    Ok(())
}

#[derive(Debug)]
pub struct AppState {
    pub exit: bool,
    collection: Vec<Item>,
    pub album_list_state: ListState,
}

impl AppState {
    fn new(collection: Vec<Item>) -> Self {
        let mut album_list_state = ListState::default();
        album_list_state.select(Some(0));

        AppState {
            exit: false,
            collection,
            album_list_state,
        }
    }
}

fn draw(frame: &mut Frame, app_state: &mut AppState) -> Result<()> {
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
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(outer_layout[1]);

    frame.render_widget("hello world", outer_layout[0]);

    // Iterate through all elements in the `items` and stylize them.
    let album_titles = app_state
        .collection
        .iter()
        .map(|i| i.title.clone())
        .collect::<Vec<String>>();

    let list = List::new(album_titles)
        .block(Block::bordered().title("Albums"))
        .highlight_style(Style::new().italic());

    // frame.render_widget(
    //     list, //Paragraph::new("Left").block(Block::new().borders(Borders::ALL)),
    //     body[0],
    // );
    frame.render_stateful_widget(list, body[0], &mut app_state.album_list_state);

    frame.render_widget(
        Paragraph::new("Right").block(Block::new().borders(Borders::ALL)),
        body[1],
    );
    frame.render_widget("woah", outer_layout[2]);

    Ok(())
}
