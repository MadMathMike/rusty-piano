use anyhow::Result;
use ratatui::prelude::*;
use rodio::OutputStreamBuilder;
use rusty_piano::app::*;
use rusty_piano::bandcamp::{BandCampClient, Item};
use rusty_piano::json_l::{read_lines_from_file, write_lines_to_file};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    let collection_path = PathBuf::from_str("collection.jsonl").unwrap();

    // https://users.rust-lang.org/t/why-is-map-or-else-backwards/27755
    let collection = File::open(&collection_path).map_or_else(
        |_| login_and_cache_collection(&collection_path, 5),
        read_lines_from_file,
    );

    // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)
    let mut terminal = ratatui::init();

    let collection_as_vms: Vec<Album> = collection.into_iter().map(Album::from).collect();

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

        // 16 milliseconds should yield about 60 FPS (assuming events are handled fast enough, of course)
        thread::sleep(Duration::from_millis(16));
    }

    // TODO: we really want to properly fix all of the error handling in this application (i.e., all of the bad unwraps and expects)
    // When the app crashes, ratatui::restore() never gets called, leaving the terminal in a funky state
    // Returns the terminal back to normal mode
    ratatui::restore();

    Ok(())
}

fn login_and_cache_collection(collection_path: &Path, page_size: usize) -> Vec<Item> {
    let mut client = None;
    while client.is_none() {
        println!("Bandcamp username:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let username = input.trim_end().to_owned();

        println!("Password:");
        let password = rpassword::read_password().unwrap();

        match BandCampClient::new(&username, &password) {
            Ok(c) => client = Some(c),
            Err(err) => println!("{err}"),
        }
    }

    print!("Caching collection... ");
    let items = client.unwrap().get_entire_collection(page_size);
    println!("Done!");

    let file = File::create(&collection_path).unwrap();
    write_lines_to_file(file, items.iter());

    items
}
