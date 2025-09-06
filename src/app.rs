use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::prelude::*;
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListState, StatefulWidget, Widget};
use reqwest::StatusCode;
use rodio::OutputStream;
use std::fs::{File, create_dir_all};
use std::io::{Write, copy};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

use crate::player::Player;

pub struct App {
    pub exit: bool,
    collection: Vec<Album>,
    album_list_state: ListState,
    channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
    player: Player,
}

#[derive(PartialEq)]
pub enum DownloadStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    DownloadFailed,
}

pub struct Album {
    pub id: u32,
    pub title: String,
    pub tracks: Vec<Track>,
    pub band_name: String,
    pub download_status: DownloadStatus,
}

impl Album {
    pub fn download(
        &mut self,
        mpsc_tx: mpsc::Sender<Event>,
    ) -> Option<JoinHandle<std::result::Result<(), mpsc::SendError<Event>>>> {
        match self.download_status {
            DownloadStatus::Downloaded | DownloadStatus::Downloading => None,
            DownloadStatus::NotDownloaded | DownloadStatus::DownloadFailed => {
                self.download_status = DownloadStatus::Downloading;
                let tracks = self.tracks.clone();
                let album_id = self.id;

                let handle = thread::spawn(move || {
                    let download_failure = tracks
                        .iter()
                        .map(|track| download_track(&track.file_path, &track.download_url))
                        .find(|result| result.is_err());

                    match download_failure {
                        None => mpsc_tx.send(Event::AlbumDownloaded(album_id)),
                        Some(_) => mpsc_tx.send(Event::AlbumDownLoadFailed(album_id)),
                    }
                });

                Some(handle)
            }
        }
    }
}

#[derive(Clone)]
pub struct Track {
    pub number: u8,
    pub title: String,
    pub download_url: String,
    pub file_path: PathBuf,
}

pub enum Event {
    Input(KeyEvent),
    AlbumDownloaded(u32),
    AlbumDownLoadFailed(u32),
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
                Event::Input(key_event) => self.on_key_event(key_event)?,
                Event::AlbumDownloaded(id) => self.on_album_downloaded(id)?,
                // TODO: update this event to display some kind of error somewhere
                Event::AlbumDownLoadFailed(id) => self.on_album_download_failed(id),
            },
            // TODO: consider letting the player have its own thread that tries to play the next track when appropriate
            Err(TryRecvError::Empty) => self.player.try_play_next_track()?,
            Err(_) => self.exit = true,
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        match key.code {
            KeyCode::Enter => {
                if let Some(selected) = self.album_list_state.selected() {
                    self.on_album_selected(selected)?;
                }
            }
            KeyCode::Up => {
                self.album_list_state.select_previous();
            }
            KeyCode::Down => {
                self.album_list_state.select_next();
            }
            KeyCode::Left => {
                self.player.play_previous_track()?;
            }
            KeyCode::Right => {
                self.player.play_next_track()?;
            }
            KeyCode::Char(' ') => self.player.toggle_playback(),
            KeyCode::Char('d') => self.download_all(),
            KeyCode::Char('q') => self.exit = true,
            // 't' for test? As in, play test sound? I guess that's fine if we don't need t for anything else
            KeyCode::Char('t') => {
                let album = crate::player::Album {
                    title: "Test file".to_owned(),
                    tracks: vec![crate::player::Track {
                        number: 1,
                        title: "file_example_MP3_2MG".to_owned(),
                        file_path: PathBuf::from_str("./file_example_MP3_2MG.mp3")?,
                    }],
                    band_name: "Me".to_owned(),
                };
                self.player.play(album)?;
            }
            _ => (),
        };

        Ok(())
    }

    fn on_album_selected(&mut self, selected: usize) -> Result<()> {
        if let Some(album) = self.collection.get_mut(selected) {
            match album.download_status {
                DownloadStatus::Downloading => {}
                DownloadStatus::Downloaded => self.player.play(album.into())?,
                DownloadStatus::NotDownloaded | DownloadStatus::DownloadFailed => {
                    album.download(self.channel.0.clone());
                }
            }
        }

        Ok(())
    }

    fn on_album_downloaded(&mut self, id: u32) -> Result<()> {
        let album = self
            .collection
            .iter_mut()
            .find(|album| album.id == id)
            .unwrap();

        album.download_status = DownloadStatus::Downloaded;

        self.player.play_if_empty(album.into())
    }

    fn on_album_download_failed(&mut self, id: u32) {
        let album = self
            .collection
            .iter_mut()
            .find(|album| album.id == id)
            .unwrap();

        album.download_status = DownloadStatus::DownloadFailed;
    }

    // TODO: Holy shit this is so bad, for so many reasons.
    fn download_all(&mut self) {
        let senders = Vec::from_iter((0..self.collection.len()).map(|i| (i, self.clone_sender())));
        senders
            .into_iter()
            .map(|(i, sender)| self.collection.get_mut(i).unwrap().download(sender))
            .for_each(|_| {});
    }
}

fn download_track(path: &PathBuf, download_url: &str) -> Result<()> {
    // TODO: should I only have one client?
    let mut download_response = reqwest::blocking::Client::new().get(download_url).send()?;

    match download_response.status() {
        StatusCode::OK => {
            create_dir_all(path.parent().unwrap())?;
            let mut file = File::create(&path)?;

            copy(&mut download_response, &mut file)?;
            file.flush()?;

            Ok(())
        }
        // Bandcamp URLs eventually return a 410-Gone response when the download link is no longer valid
        _ => Err(anyhow!(
            "Download status code: {}",
            download_response.status()
        )),
    }
}

impl From<&mut Album> for crate::player::Album {
    fn from(value: &mut Album) -> Self {
        crate::player::Album {
            title: value.title.clone(),
            tracks: value
                .tracks
                .iter()
                .map(|t| crate::player::Track {
                    number: t.number,
                    title: t.title.clone(),
                    file_path: t.file_path.clone(),
                })
                .collect(),
            band_name: value.band_name.clone(),
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [header, body, footer] = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(3),
            ])
            .areas(area);

        Line::from("rusty piano")
            .alignment(Alignment::Center)
            .render(header, buf);

        let [left, right] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(body);

        let album_titles = self
            .collection
            .iter()
            .map(|album| {
                let icon = match album.download_status {
                    DownloadStatus::NotDownloaded => '💾',
                    DownloadStatus::Downloading => '⏳',
                    DownloadStatus::Downloaded => '✅',
                    DownloadStatus::DownloadFailed => '🚨',
                };
                format!("{} {icon}", album.title.clone())
            })
            .collect::<Vec<String>>();

        let list = List::new(album_titles)
            .block(Block::bordered().title("Albums"))
            .highlight_symbol(">");

        StatefulWidget::render(list, left, buf, &mut self.album_list_state);

        Widget::render(&mut self.player, right, buf);

        Line::from(
            "'↑/↓' select album | 'enter' play album | 'spacebar' play/pause | '←/→' previous/next track | 'q' quit",
        )
        .alignment(Alignment::Center)
        .render(footer, buf);
    }
}
