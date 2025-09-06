use anyhow::Result;
use ratatui::prelude::*;
use rodio::OutputStreamBuilder;
use rpassword::read_password;
use rusty_piano::json_l::{read_lines, write_lines};
use rusty_piano::{app::*, bandcamp::BandCampClient};
use std::path::PathBuf;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    let collection_path = PathBuf::from_str("collection.jsonl").unwrap();
    // TODO: Change read_lines to return an Option if the file doesn't exist instead
    let collection = read_lines(&collection_path).unwrap_or_else(|_| {
        let client = login();

        print!("Caching collection... ");
        let items = client.get_entire_collection(5);
        println!("Done!");

        write_lines(&collection_path, items.iter());

        items
    });

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

    // Returns the terminal back to normal mode
    ratatui::restore();

    Ok(())
}

fn login() -> BandCampClient {
    loop {
        println!("Bandcamp username:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let username = input.trim_end().to_owned();

        println!("Password:");
        let password = read_password().unwrap();

        match BandCampClient::new(&username, &password) {
            Ok(client) => return client,
            Err(err) => println!("{err}"),
        }
    }
}
