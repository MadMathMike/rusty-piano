use anyhow::Result;
use log::error;
use ratatui::Frame;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    widgets::{Block, Borders, List, Paragraph},
};
use rodio::OutputStreamBuilder;
use rusty_piano::json_l::{read_lines, write_lines};
use rusty_piano::{
    app::*,
    bandcamp::{BandCampClient, Item},
    secrets::{get_access_token, store_access_token},
};
use std::path::PathBuf;
use std::str::FromStr;
use std::thread;

// TODO: In manual testing in the Konsole, an error downloading a file (e.g., 410 from bandcamp)
// resulted in the UI getting replaced with an error, the music still playing, and the app stuck

fn main() -> Result<()> {
    let collection =
        read_lines(&PathBuf::from_str("collection.jsonl").unwrap()).unwrap_or_else(|e| {
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

    let ui_thread_mpsc_tx = app.clone_sender();

    // TODO: error handling. If this input thread panics, how do I notify the main thread and exit the application?
    thread::spawn(move || {
        loop {
            if let crossterm::event::Event::Key(key_event) = crossterm::event::read().unwrap() {
                ui_thread_mpsc_tx.send(Event::Input(key_event)).unwrap();
            }
        }
    });

    while !app.exit {
        terminal.draw(|frame| draw(frame, &mut app).unwrap())?;

        app.handle_next_event()?;
    }

    // Returns the terminal back to normal mode
    ratatui::restore();

    Ok(())
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
    let download_status = if tracks.iter().all(|t| t.file_path.exists()) {
        DownloadStatus::Downloaded
    } else {
        DownloadStatus::NotDownloaded
    };

    Album {
        title: item.title,
        tracks,
        download_status,
    }
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

    write_lines(
        &PathBuf::from_str("collection.jsonl").unwrap(),
        items.iter(),
    );

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

// TODO: this might not feel so bad to move back to an "impl Widget for App" block
// if enough logic is pulled out of the app.rs module
fn draw(frame: &mut Frame, app: &mut App) -> Result<()> {
    let [header, body, footer] = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(frame.area());

    Line::from("hello world").render(header, frame.buffer_mut());

    let [left, right] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .areas(body);

    let album_titles = app
        .collection
        .iter()
        .map(|album| {
            let icon = match album.download_status {
                DownloadStatus::NotDownloaded => '⭳',
                DownloadStatus::Downloading => '⏳',
                DownloadStatus::Downloaded => '✓',
            };
            format!("{} {icon}", album.title.clone())
        })
        .collect::<Vec<String>>();

    let list = List::new(album_titles)
        .block(Block::bordered().title("Albums"))
        .highlight_symbol(">");

    StatefulWidget::render(list, left, frame.buffer_mut(), &mut app.album_list_state);

    Paragraph::new("Right")
        .block(Block::new().borders(Borders::ALL))
        .render(right, frame.buffer_mut());

    Line::from("woah").render(footer, frame.buffer_mut());

    Ok(())
}
