use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::ListState;
use reqwest::StatusCode;
use rodio::OutputStream;
use std::fs::{File, create_dir_all};
use std::io::{Write, copy};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;

use crate::player::Player;

pub struct App {
    pub exit: bool,
    // TODO: rename collection to albums (or rename album_list_state to collection_list_state)
    pub collection: Vec<Album>,
    pub album_list_state: ListState,
    channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
    player: Player,
}

pub enum DownloadStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    DownloadFailed,
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
    AlbumDownLoadFailed { title: String },
}

impl App {
    pub fn new(collection: Vec<Album>, audio_output_stream: &OutputStream) -> Self {
        let mut album_list_state = ListState::default();
        album_list_state.select(Some(0));

        let channel = mpsc::channel();

        let player = Player::new(audio_output_stream);

        App {
            exit: false,
            collection,
            album_list_state,
            channel,
            player,
        }
    }

    pub fn clone_sender(&self) -> Sender<Event> {
        self.channel.0.clone()
    }

    pub fn handle_next_event(&mut self) -> Result<()> {
        match self.channel.1.try_recv() {
            Ok(event) => match event {
                Event::Input(key_event) => self.on_key_event(key_event),
                Event::AlbumDownloadedEvent { title } => self.on_album_downloaded(&title),
                Event::AlbumDownLoadFailed { title } => self.on_album_download_failed(&title),
            },
            // TODO: consider letting the player have its own thread that tries to play the next track when appropriate
            Err(TryRecvError::Empty) => self.player.play_next_if_track_finished(),
            Err(_) => self.exit = true,
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                if let Some(selected) = self.album_list_state.selected() {
                    self.on_album_selected(selected);
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
            KeyCode::Char(' ') => self.player.toggle_playback(),
            // 't' for test? As in, play test sound? I guess that's fine if we don't need t for anything else
            KeyCode::Char('t') => {
                let album = crate::player::Album {
                    tracks: vec![crate::player::Track {
                        file_path: PathBuf::from_str("./file_example_MP3_2MG.mp3").unwrap(),
                    }],
                };
                self.player.play(album);
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

    fn on_album_selected(&mut self, selected: usize) {
        if let Some(album) = self.collection.get_mut(selected) {
            match album.download_status {
                DownloadStatus::Downloaded => {
                    let player_album = crate::player::Album {
                        tracks: album
                            .tracks
                            .iter()
                            .map(|t| crate::player::Track {
                                file_path: t.file_path.clone(),
                            })
                            .collect(),
                    };
                    self.player.play(player_album);
                }
                DownloadStatus::NotDownloaded => {
                    album.download_status = DownloadStatus::Downloading;
                    let tracks = album.tracks.clone();
                    let download_thread_mpsc_tx = self.channel.0.clone();
                    let album_title = album.title.clone();
                    thread::spawn(move || {
                        let download_failure = tracks
                            .iter()
                            .map(|track| download_track(&track.file_path, &track.download_url))
                            .find(|result| result.is_err());

                        match download_failure {
                            None => download_thread_mpsc_tx
                                .send(Event::AlbumDownloadedEvent { title: album_title }),
                            Some(_) => download_thread_mpsc_tx
                                .send(Event::AlbumDownLoadFailed { title: album_title }),
                        }
                    });
                }
                DownloadStatus::Downloading => {}
                DownloadStatus::DownloadFailed => { /* Prompt to rebuild collection */ }
            }
        }
    }

    fn on_album_downloaded(&mut self, album_title: &str) {
        let album = self
            .collection
            .iter_mut()
            .find(|album| album.title.eq(album_title));

        // TODO: "this shouldn't happen 🤪" (self deprecating)
        let album = album.expect("Album not found in collection");

        album.download_status = DownloadStatus::Downloaded;

        let player_album = crate::player::Album {
            tracks: album
                .tracks
                .iter()
                .map(|t| crate::player::Track {
                    file_path: t.file_path.clone(),
                })
                .collect(),
        };
        self.player.play_if_empty(player_album);
    }

    fn on_album_download_failed(&mut self, album_title: &str) {
        let album = self
            .collection
            .iter_mut()
            .find(|album| album.title.eq(album_title));

        // TODO: "this shouldn't happen 🤪" (self deprecating)
        let album = album.expect("Album not found in collection");

        album.download_status = DownloadStatus::DownloadFailed;
    }
}

fn download_track(path: &PathBuf, download_url: &str) -> Result<()> {
    // TODO: should I only have one client?
    let mut download_response = reqwest::blocking::Client::new()
        .get(download_url)
        .send()
        .expect("Error downloading file");

    match download_response.status() {
        StatusCode::OK => {
            create_dir_all(path.parent().unwrap()).unwrap();
            let mut file = File::create(&path).unwrap();

            copy(&mut download_response, &mut file).expect("error copying download to file");
            file.flush().expect("error finishing copy?");

            Ok(())
        }
        // Bandcamp URLs eventually return a 410-Gone response when the download link is no longer valid
        _ => Err(anyhow!(
            "Download status code: {}",
            download_response.status()
        )),
    }
}
