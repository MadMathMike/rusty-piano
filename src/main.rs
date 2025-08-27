use std::fs::{File, create_dir_all};
use std::io::{Write, copy};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use log::error;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    widgets::ListState,
    widgets::{Block, Borders, List, Paragraph},
};
use reqwest::StatusCode;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use rusty_piano::{
    bandcamp::{BandCampClient, Item, read_collection, write_collection},
    secrets::{get_access_token, store_access_token},
};

fn main() -> Result<()> {
    let collection = read_collection().unwrap_or_else(|e| {
        error!("{e:?}");
        cache_collection()
    });

    // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)
    let mut terminal = ratatui::init();

    let collection_as_vms: Vec<Album> = collection.into_iter().map(from_bandcamp_item).collect();

    // Sound gets killed when this is dropped, so it has to live as long as the whole app.
    let stream_handle =
        OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
    let mut app = App::new(collection_as_vms, &stream_handle);

    let ui_thread_mpsc_tx = app.channel.0.clone();

    // TODO: error handling. If this input thread panics, how do I notify the main thread and exit the application?
    thread::spawn(move || {
        loop {
            if let crossterm::event::Event::Key(key_event) = crossterm::event::read().unwrap() {
                ui_thread_mpsc_tx.send(Event::Input(key_event)).unwrap();
            }
        }
    });

    while !app.exit {
        terminal.draw(|frame| frame.render_widget(&mut app, frame.area()))?;

        match app.channel.1.recv()? {
            Event::Input(key_event) => app.handle_key(key_event),

            Event::AlbumDownloadedEvent { title } => app.handle_album_downloaded(&title),
        }
    }

    // Returns the terminal back to normal mode
    ratatui::restore();

    Ok(())
}

pub struct App {
    exit: bool,
    collection: Vec<Album>,
    album_list_state: ListState,
    sink: Sink,
    channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
}

pub struct Album {
    title: String,
    tracks: Vec<Track>,
    // TODO: switch this to enum for NotDownloaded, Downloading, Downloaded, DownloadFailed
    is_downloaded: bool,
}

#[derive(Clone)]
pub struct Track {
    download_url: String,
    // TODO: I think I can/should use a Path instead of a PathBuf
    file_path: PathBuf,
}

enum Event {
    Input(KeyEvent),
    // TODO: album title is a *terrible* identifier to pass back, for multiple reasons.
    AlbumDownloadedEvent { title: String },
}

impl App {
    fn new(collection: Vec<Album>, audio_output_stream: &OutputStream) -> Self {
        let mut album_list_state = ListState::default();
        album_list_state.select(Some(0));

        let sink = rodio::Sink::connect_new(audio_output_stream.mixer());
        let channel = mpsc::channel();

        App {
            exit: false,
            collection,
            album_list_state,
            sink,
            channel,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                if let Some(selected) = self.album_list_state.selected() {
                    if let Some(album) = self.collection.get(selected) {
                        // TODO: if album is downloading (or download failed), do not kick off new thread to download it!
                        // TODO: if album is downloaded, clear sink and add songs to the sink

                        // self.sink.clear();

                        let tracks = album.tracks.to_owned(); // This works thanks to deriving Clone on the Track struct
                        let download_thread_mpsc_tx = self.channel.0.clone();
                        let album_title = album.title.clone();
                        thread::spawn(move || {
                            tracks.iter().for_each(|track| {
                                download_track(&track.file_path, &track.download_url)
                            });

                            download_thread_mpsc_tx
                                .send(Event::AlbumDownloadedEvent { title: album_title })
                        });

                        // album.tracks.iter().for_each(|track| {
                        //     let path = &track.file_path;

                        //     let file = match File::open(path) {
                        //         Ok(file) => file,
                        //         Err(_) => {
                        //             download_track(path, &track.download_url);
                        //             File::open(path).unwrap()
                        //         }
                        //     };

                        //     let source = Decoder::try_from(file).expect("Error decoding file");
                        //     self.sink.append(source);
                        // });
                        // self.sink.play();
                    }
                }
            }
            // TODO: vim keybindings might suggest the 'j' and 'k' keys should be used
            // for down and up respectively
            // YouTube uses 'j' for back-ten-seconds, 'k' for play/pause, and 'l' for skip-ten-seconds
            // Should I do something similar, except maybe use j/l for back/skip-one-track?
            // Does this suggest two different input modes, one for album/track navigation, and one for playback control?
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

    fn handle_album_downloaded(&mut self, album_title: &str) {
        if let Some(album) = self
            .collection
            .iter_mut()
            .find(|album| album.title.eq(album_title))
        {
            album.is_downloaded = true;

            if self.sink.empty() {
                album.tracks.iter().for_each(|track| {
                    let file = File::open(&track.file_path)
                        .expect("Song should have been downloaded already 😬");
                    let source = Decoder::try_from(file).expect("Error decoding file");
                    self.sink.append(source);
                });
            }
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

// TODO: change return type to Path instead of PathBuf
fn to_file_path(album: &Item, track: &rusty_piano::bandcamp::Track) -> PathBuf {
    // This is incomplete, but works for now as it address the one album in my library with a problem:
    // The album "tempor / jester" by A Unicorn Masquerade resulted in a jester subdirectory
    fn filename_safe_char(char: char) -> bool {
        char != '/'
    }

    let mut band_name = album.band_info.name.clone();
    band_name.retain(filename_safe_char);
    let mut album_title = album.title.clone();
    album_title.retain(filename_safe_char);
    // TODO: download path should definitely be partially built by some safe absolute path, (and hopefully) configurable
    let mut path = PathBuf::from(format!("./bandcamp/{band_name}/{album_title}"));

    let mut track_title = track.title.clone();
    track_title.retain(filename_safe_char);
    path.push(format!("{:02} - {track_title}.mp3", track.track_number));
    path
}

fn cache_collection() -> Vec<Item> {
    let client = match get_access_token() {
        Some(token) => BandCampClient::init_with_token(token.clone()).or_else(login),
        None => login(),
    }
    .expect("Failed initialization. Bad token or credentials. Or something...");

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

fn login() -> Option<BandCampClient> {
    fn prompt(param: &str) -> String {
        println!("Enter your bandcamp {param}:");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect("Error reading standard in");
        input.trim_end().to_owned()
    }

    println!("Attempting login...");

    let username = prompt("username");
    // TODO: hide password during prompt
    let password = prompt("password");

    BandCampClient::init(&username, &password).map(|tuple| {
        store_access_token(&tuple.1);
        tuple.0
    })
}
