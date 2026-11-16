use anyhow::Result;
use ratatui::prelude::*;
use rodio::OutputStreamBuilder;
use rusty_piano::app::*;
use rusty_piano::bandcamp::{BandCampClient, Item};
use rusty_piano::json_l;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    let bandcamp_client = login()?;

    let collection_path = PathBuf::from_str("collection.jsonl")?;

    let collection = match File::open(&collection_path) {
        Ok(file) => json_l::read_lines_buf(file)?,
        Err(_) => cache_collection(bandcamp_client, &collection_path, 5)?,
    };

    // Puts the terminal in raw mode, which disables line buffering (so rip to ctrl+c response)
    let mut terminal = ratatui::init();

    // Sound gets killed when this is dropped, so it has to live as long as the whole app.
    let stream_handle = OutputStreamBuilder::open_default_stream()?;
    // In testing, 2 thread wasn't any faster than 1 threads
    // That could change with sufficient concurrent downloads,
    // but I kind of like the idea of limiting the number of threads.
    let download_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let mut app = App::new(collection, &stream_handle, download_runtime);

    let ui_thread_mpsc_tx = app.clone_sender();

    // TODO: error handling. If this input thread panics, how do I notify the main thread and exit the application?
    thread::spawn(move || {
        loop {
            if let crossterm::event::Event::Key(key_event) = crossterm::event::read().unwrap() {
                ui_thread_mpsc_tx
                    .send(rusty_piano::events::Event::Input(key_event))
                    .unwrap();
            }
        }
    });

    // TODO: I'm not sure this loop is correct. Should our intention be to disconnect event handling from
    // rendering? I.e., should we be able to proces input events faster than we render a frame? If so,
    // I don't think this loop accomplishes that.
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

fn login() -> Result<BandCampClient> {
    let entry = keyring::Entry::new("rusty-piano", "bandcamp-refresh-token")?;
    let secret = entry.get_secret();

    match secret {
        Ok(secret_bytes) => {
            let refresh_token = String::from_utf8(secret_bytes)?;
            let (client, refresh_token) = BandCampClient::refresh(&refresh_token)?;
            entry.set_secret(refresh_token.as_bytes())?;
            Ok(client)
        }
        Err(err) => {
            println!("Could not retrieve refresh token: {err:?}");

            let mut client = None;
            while client.is_none() {
                println!("Bandcamp username:");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let username = input.trim_end().to_owned();

                println!("Password:");
                let password = rpassword::read_password()?;

                match BandCampClient::new(&username, &password) {
                    Ok((c, refresh_token)) => {
                        entry.set_secret(refresh_token.as_bytes())?;
                        client = Some(c)
                    }
                    Err(err) => println!("{err:?}"),
                }
            }

            Ok(client.unwrap())
        }
    }
}

fn cache_collection(
    client: BandCampClient,
    collection_path: &Path,
    page_size: usize,
) -> Result<Vec<Item>> {
    print!("Caching collection... ");
    std::io::stdout().flush()?;
    let items = client.get_entire_collection(page_size)?;
    println!("Done!");

    let file = File::create(&collection_path)?;
    json_l::write_lines_buf(file, items.iter())?;

    Ok(items)
}
