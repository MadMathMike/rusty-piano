use anyhow::Result;
use log::error;
use ratatui::prelude::*;
use rodio::OutputStreamBuilder;
use rpassword::read_password;
use rusty_piano::json_l::{read_lines, write_lines};
use rusty_piano::{
    app::*,
    bandcamp::{BandCampClient, Item},
};
use std::path::PathBuf;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

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
        terminal.draw(|frame| app.render(frame.area(), frame.buffer_mut()))?;

        app.handle_next_event()?;

        thread::sleep(Duration::from_millis(16)); // 16 milliseconds should yield about 60 FPS, which is _more_ than enough for this TUI app, I think
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
            title: track.title.clone(),
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
        title: format!("{} by {}", item.title, item.band_info.name),
        tracks,
        download_status,
    }
}

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
    let client = login();

    // TODO: add logging that states the collection is being cached
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

fn login() -> BandCampClient {
    loop {
        println!("Bandcamp username:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let username = input.trim_end().to_owned();

        println!("Password:");
        let password = read_password().unwrap();

        if let Ok(client) = BandCampClient::new(&username, &password) {
            return client;
        }

        // TODO: actually print out the errors.
        println!("Something went wrong initializing the client. Try again.");
    }
}
