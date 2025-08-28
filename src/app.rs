use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::ListState;
use reqwest::StatusCode;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::{File, create_dir_all};
use std::io::{Write, copy};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;

pub struct App {
    pub exit: bool,
    pub collection: Vec<Album>,
    pub album_list_state: ListState,
    pub sink: Sink,
    pub channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
}

pub enum DownloadStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    // DownloadFailed(Error + Sync + Send),
}

pub struct Album {
    pub title: String,
    pub tracks: Vec<Track>,
    pub download_status: DownloadStatus,
}

#[derive(Clone)]
pub struct Track {
    pub download_url: String,
    pub file_path: PathBuf,
}

pub enum Event {
    // TODO: consider abstracting away from crossterm events
    Input(KeyEvent),
    // TODO: album title is a *terrible* identifier to pass back, for multiple reasons.
    AlbumDownloadedEvent { title: String },
}

impl App {
    pub fn new(collection: Vec<Album>, audio_output_stream: &OutputStream) -> Self {
        let mut album_list_state = ListState::default();
        album_list_state.select(Some(0));

        let sink = Sink::connect_new(audio_output_stream.mixer());
        let channel = mpsc::channel();

        App {
            exit: false,
            collection,
            album_list_state,
            sink,
            channel,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                if let Some(selected) = self.album_list_state.selected() {
                    if let Some(album) = self.collection.get_mut(selected) {
                        match album.download_status {
                            DownloadStatus::Downloaded => play_album(&self.sink, album),
                            DownloadStatus::NotDownloaded => {
                                album.download_status = DownloadStatus::Downloading;
                                let tracks = album.tracks.to_owned();
                                let download_thread_mpsc_tx = self.channel.0.clone();
                                let album_title = album.title.clone();
                                thread::spawn(move || {
                                    tracks.iter().for_each(|track| {
                                        download_track(&track.file_path, &track.download_url)
                                    });

                                    download_thread_mpsc_tx
                                        .send(Event::AlbumDownloadedEvent { title: album_title })
                                });
                            }
                            DownloadStatus::Downloading => {}
                        }
                    } else {
                        println!("uh oh!");
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
            KeyCode::Char(' ') => {
                self.sink.clear();
                let file_path = PathBuf::from_str("./file_example_MP3_2MG.mp3").unwrap();
                let file = File::open(file_path).expect("Couldn't find file?");
                let source = Decoder::try_from(file).expect("Error decoding file");
                self.sink.append(source);
                self.sink.play();
            }
            // KeyCode::Char(_) => ,
            // KeyCode::Null => ,
            // KeyCode::Esc => ,
            // KeyCode::Pause => ,
            // KeyCode::Media(media_key_code) => ,
            // KeyCode::Modifier(modifier_key_code) => ,
            _ => (),
        }
    }

    pub fn handle_album_downloaded(&mut self, album_title: &str) {
        let album = self
            .collection
            .iter_mut()
            .find(|album| album.title.eq(album_title));

        // TODO: "this shouldn't happen 🤪" (self deprecating)
        let album = album.expect("Album not found in collection");

        album.download_status = DownloadStatus::Downloaded;

        if self.sink.empty() {
            self.sink.clear();

            album.tracks.iter().for_each(|track| {
                let file = File::open(&track.file_path)
                    .expect("Song should have been downloaded already 😬");
                let source = Decoder::try_from(file).expect("Error decoding file");
                self.sink.append(source);
            });
        }
    }
}

fn play_album(sink: &Sink, album: &Album) {
    sink.clear();

    album.tracks.iter().for_each(|track| {
        let file =
            File::open(&track.file_path).expect("Song should have been downloaded already 😬");
        let source = Decoder::try_from(file).expect("Error decoding file");
        sink.append(source);
    });

    sink.play();
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
