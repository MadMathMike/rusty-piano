use std::fs::File;
use std::io::{Write, copy};
use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEventKind};
use log::error;
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, Paragraph},
};
use reqwest::StatusCode;
use rodio::{Decoder, OutputStreamBuilder};
use rusty_piano::bandcamp::{BandCampClient, Track};
use rusty_piano::secrets::{get_access_token, store_access_token};
use rusty_piano::{bandcamp::Item, collection::read_collection};

fn main() -> Result<()> {
    // TODO: abstract the library away from the bandcamp collection
    // TODO: separate downloading/caching from the collection

    let collection = read_collection().unwrap_or_else(|e| {
        error!("{e:?}");
        cache_collection()
    });

    let mut terminal = ratatui::init(); // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)

    let mut app_state = AppState::new(collection);

    let stream_handle =
        OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
    let sink = rodio::Sink::connect_new(stream_handle.mixer());

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
                            if let Some(track) = app_state.collection.get(selected).map(|item| {
                                item.tracks
                                    .first()
                                    .expect("Should not have any empty albums")
                            }) {
                                let file = download_track(&track);
                                let source = Decoder::try_from(file).expect("Error decoding file");
                                sink.clear();
                                sink.append(source);
                                sink.play();
                            }
                        }
                    }
                    KeyCode::Up => {
                        app_state.album_list_state.scroll_up_by(1);
                    }
                    KeyCode::Down => {
                        app_state.album_list_state.scroll_down_by(1);
                    }
                    KeyCode::Char('q') => app_state.exit = true,
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

fn download_track(track: &Track) -> File {
    let mut path_buf = PathBuf::new();
    path_buf.push(std::env::temp_dir());
    path_buf.push(format!("{}.mp3", track.track_id));

    if let Ok(file) = File::open(&path_buf) {
        return file;
    }

    let mut temp_file = File::create(&path_buf).unwrap();

    let mut download_response = reqwest::blocking::Client::new()
        .get(&track.hq_audio_url)
        .send()
        .expect("Error downloading file");
    // TODO: Return an error when Bandcamp returns a 410
    assert_eq!(StatusCode::OK, download_response.status());

    // Bandcamp will return a 410, Gone response when the link is no longer valid
    // I suspect the link is only valid for some amount of time.
    // Maybe as long as the access token, which is about an hour. Not sure.

    copy(&mut download_response, &mut temp_file).expect("error copying download to file");
    temp_file.flush().expect("error finishing copy?");

    File::open(path_buf).unwrap()
}

fn cache_collection() -> Vec<Item> {
    let client = authenticate_with_bandcamp();

    let items = client.get_entire_collection(5);

    let unique_count = items
        .iter()
        .map(|i| i.tralbum_id)
        .collect::<std::collections::HashSet<_>>()
        .len();

    // Maybe this goes in an integration test? Requires authentication, so probably not
    assert_eq!(
        unique_count,
        items.len(),
        "There should not be any duplicates in the collection"
    );
    // assert_eq!(7, unique_count);

    rusty_piano::collection::write_collection(&items);

    items
}

fn authenticate_with_bandcamp() -> BandCampClient {
    match get_access_token() {
        Some(token) => BandCampClient::init_with_token(token.clone()).or_else(login),
        None => login(),
    }
    .expect("Failed initialization. Bad token or credentials. Or something...")
}

fn login() -> Option<BandCampClient> {
    println!("Attempting login...");

    let username = prompt("username");
    // TODO: hide password during prompt
    let password = prompt("password");

    BandCampClient::init(&username, &password).map(|tuple| {
        store_access_token(&tuple.1);
        tuple.0
    })
}

fn prompt(param: &str) -> String {
    println!("Enter your bandcamp {param}:");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Error reading standard in");
    input.trim_end().to_owned()
}
