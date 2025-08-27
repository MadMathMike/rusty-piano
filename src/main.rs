use std::fs::{File, create_dir_all};
use std::io::{Write, copy};
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use log::error;
use ratatui::widgets::ListState;
use ratatui::{DefaultTerminal, prelude::*};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, Paragraph},
};
use reqwest::StatusCode;
use rodio::{Decoder, OutputStreamBuilder};
use rusty_piano::bandcamp::{BandCampClient, write_collection};
use rusty_piano::secrets::{get_access_token, store_access_token};
use rusty_piano::{bandcamp::Item, bandcamp::read_collection};

fn main() -> Result<()> {
    let collection = read_collection().unwrap_or_else(|e| {
        error!("{e:?}");
        cache_collection()
    });

    // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)
    let terminal = ratatui::init();

    let collection_as_vms: Vec<Album> = collection.into_iter().map(from_bandcamp_item).collect();

    let app = App::new(collection_as_vms);

    let result = app.run(terminal);

    // Returns the terminal back to normal mode
    ratatui::restore();
    result
}

pub struct App {
    exit: bool,
    collection: Vec<Album>,
    album_list_state: ListState,
}

pub struct Album {
    title: String,
    tracks: Vec<Track>,
    is_downloaded: bool,
}

pub struct Track {
    download_url: String,
    file_path: PathBuf,
}

impl App {
    fn new(collection: Vec<Album>) -> Self {
        let mut album_list_state = ListState::default();
        album_list_state.select(Some(0));

        App {
            exit: false,
            collection,
            album_list_state,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let stream_handle =
            OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
        let sink = rodio::Sink::connect_new(stream_handle.mixer());

        while !self.exit {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            // TODO: change event read to not be blocking so we can get UI updates without waiting for a key press
            // e.g., we want to update the "is_downloaded" field and register that without waiting for a UI re-render
            if let crossterm::event::Event::Key(key_event) = crossterm::event::read()? {
                self.handle_key(key_event, &sink);
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent, sink: &rodio::Sink) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                if let Some(selected) = self.album_list_state.selected() {
                    if let Some(album) = self.collection.get(selected) {
                        sink.clear();
                        album.tracks.iter().for_each(|track| {
                            let path = &track.file_path;

                            let file = match File::open(path) {
                                Ok(file) => file,
                                Err(_) => {
                                    download_track(path, &track.download_url);
                                    File::open(path).unwrap()
                                }
                            };

                            let source = Decoder::try_from(file).expect("Error decoding file");
                            sink.append(source);
                        });
                        sink.play();
                    }
                }
            }
            // TODO: vim keybindings might suggest the 'j' and 'k' keys should be used
            // for down and up respectively
            KeyCode::Up => {
                self.album_list_state.scroll_up_by(1);
            }
            KeyCode::Down => {
                self.album_list_state.scroll_down_by(1);
            }
            KeyCode::Char('q') => self.exit = true,
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

fn from_bandcamp_item(item: Item) -> Album {
    let tracks = item
        .tracks
        .iter()
        .map(|track| Track {
            download_url: track.hq_audio_url.clone(),
            file_path: to_file_path(&item, &track),
        })
        .collect::<Vec<Track>>();

    // figure out if everything is downloaded
    let is_downloaded = tracks.iter().all(|t| t.file_path.exists());

    Album {
        title: item.title,
        tracks,
        is_downloaded,
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header, body, footer] = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(3),
            ])
            .areas(area);

        Line::from("hello world").render(header, buf);

        let [left, right] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
            .areas(body);

        // Iterate through all elements in the `items` and stylize them.
        let album_titles = self
            .collection
            .iter()
            .map(|album| {
                let icon = if album.is_downloaded { '✓' } else { '⭳' };
                format!("{} {icon}", album.title.clone())
            })
            .collect::<Vec<String>>();

        let list = List::new(album_titles)
            .block(Block::bordered().title("Albums"))
            .highlight_symbol(">");

        // frame.render_widget(
        //     list, //Paragraph::new("Left").block(Block::new().borders(Borders::ALL)),
        //     body[0],
        // );
        StatefulWidget::render(list, left, buf, &mut self.album_list_state);

        Paragraph::new("Right")
            .block(Block::new().borders(Borders::ALL))
            .render(right, buf);

        Line::from("woah").render(footer, buf);
    }
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

    create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = File::create(&path).unwrap();

    copy(&mut download_response, &mut file).expect("error copying download to file");
    file.flush().expect("error finishing copy?");
}

fn to_file_path(album: &Item, track: &rusty_piano::bandcamp::Track) -> PathBuf {
    let mut band_name = album.band_info.name.clone();
    band_name.retain(filename_safe_char);
    let mut album_title = album.title.clone();
    album_title.retain(filename_safe_char);
    let mut path = PathBuf::from(format!("./bandcamp/{band_name}/{album_title}"));

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
