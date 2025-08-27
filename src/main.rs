use std::fs::{File, create_dir_all};
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
use rusty_piano::bandcamp::{BandCampClient, Track, write_collection};
use rusty_piano::secrets::{get_access_token, store_access_token};
use rusty_piano::{bandcamp::Item, bandcamp::read_collection};

fn main() -> Result<()> {
    let collection = read_collection().unwrap_or_else(|e| {
        error!("{e:?}");
        cache_collection()
    });

    // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)
    let mut terminal = ratatui::init();

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
                            if let Some(album) = app_state.collection.get(selected) {
                                sink.clear();
                                album.tracks.iter().for_each(|track| {
                                    let path = to_file_path(album, track);

                                    let file = match File::open(&path) {
                                        Ok(file) => file,
                                        Err(_) => {
                                            download_track(&path, &track.hq_audio_url);
                                            File::open(&path).unwrap()
                                        }
                                    };

                                    let source =
                                        Decoder::try_from(file).expect("Error decoding file");
                                    sink.append(source);
                                });
                                sink.play();
                            }
                        }
                    }
                    // TODO: vim keybindings might suggest the 'j' and 'k' keys should be used
                    // for down and up respectively
                    KeyCode::Up => {
                        app_state.album_list_state.scroll_up_by(1);
                    }
                    KeyCode::Down => {
                        app_state.album_list_state.scroll_down_by(1);
                    }
                    KeyCode::Char('q') => app_state.exit = true,
                    // KeyCode::Char(_) => ,
                    // KeyCode::Null => ,
                    // KeyCode::Esc => ,
                    // KeyCode::Pause => ,
                    // KeyCode::Media(media_key_code) => ,
                    // KeyCode::Modifier(modifier_key_code) => ,
                    _ => (),
                }
            }
        }
    }

    // Returns the terminal back to normal mode
    ratatui::restore();
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

// TODO: update download_track to return a Result<...> to prompt for re-chaching the collection
// Bandcamp URLs eventually return a 410 gone response when the download link is no longer valid
fn download_track(path: &PathBuf, download_url: &str) {
    // TODO: should I only have one client?
    let mut download_response = reqwest::blocking::Client::new()
        .get(download_url)
        .send()
        .expect("Error downloading file");

    assert_eq!(StatusCode::OK, download_response.status());

    let mut file = File::create(&path).unwrap();

    copy(&mut download_response, &mut file).expect("error copying download to file");
    file.flush().expect("error finishing copy?");
}

fn to_file_path(album: &Item, track: &Track) -> PathBuf {
    let mut band_name = album.band_info.name.clone();
    band_name.retain(filename_safe_char);
    let mut album_title = album.title.clone();
    album_title.retain(filename_safe_char);
    let mut path = PathBuf::from(format!("./bandcamp/{band_name}/{album_title}"));
    create_dir_all(&path).unwrap();

    let mut track_title = track.title.clone();
    track_title.retain(filename_safe_char);
    path.push(format!("{:02} - {track_title}.mp3", track.track_number));
    path
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

    write_collection(&items);

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

// This is incomplete, but works for now as it address the one album in my library with a problem:
// The album "tempor / jester" by A Unicorn Masquerade resulted in a jester subdirectory
fn filename_safe_char(char: char) -> bool {
    char != '/'
}
