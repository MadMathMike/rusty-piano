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
    collection: Collection,
    // album_list_state: ListState,
    channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
    player: Player,
    error: String,
}

pub struct Collection {
    albums: Vec<Album>,
    pub album_state: ListState,
}

impl Collection {
    pub fn download_selected_album(&mut self, mpsc_tx: mpsc::Sender<Event>) -> Option<Album> {
        let selected = self.album_state.selected();
        if selected.is_none() {
            return None;
        }
        let selected = selected.unwrap();

        let album = self.albums.get_mut(selected);

        album.and_then(|album| match album.download_status {
            DownloadStatus::Downloading => None,
            DownloadStatus::Downloaded => Some(album.clone()),
            DownloadStatus::NotDownloaded | DownloadStatus::DownloadFailed => {
                album.download(mpsc_tx);
                None
            }
        })
    }

    // pub fn get_album(&self, id: u32) -> Option<&Album> {
    //     todo!()
    // }
}

#[derive(PartialEq, Clone)]
pub enum DownloadStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    DownloadFailed,
}

#[derive(Clone)]
pub struct Album {
    pub id: u32,
    pub title: String,
    pub tracks: Vec<Track>,
    pub band_name: String,
    // TODO: make this private once it is managed by the collection module
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
                        None => mpsc_tx.send(Event::CollectionEvent(
                            CollectionEvent::AlbumDownloaded(album_id),
                        )),
                        Some(err) => mpsc_tx.send(Event::CollectionEvent(
                            CollectionEvent::AlbumDownLoadFailed(album_id, err.err().unwrap()),
                        )),
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

pub enum CollectionEvent {
    AlbumDownloaded(u32),
    AlbumDownLoadFailed(u32, anyhow::Error),
}

pub enum Event {
    Input(KeyEvent),
    CollectionEvent(CollectionEvent),
}

impl App {
    pub fn new(collection: Vec<Album>, audio_output_stream: &OutputStream) -> Self {
        let mut album_list_state = ListState::default();
        album_list_state.select(Some(0));

        let channel = mpsc::channel();

        let player = Player::new(audio_output_stream);

        App {
            exit: false,
            collection: Collection {
                albums: collection,
                album_state: album_list_state,
            },
            channel,
            player,
            error: "".to_owned(),
        }
    }

    pub fn clone_sender(&self) -> Sender<Event> {
        self.channel.0.clone()
    }

    pub fn handle_next_event(&mut self) -> Result<()> {
        match self.channel.1.try_recv() {
            Ok(event) => match event {
                Event::Input(key_event) => self.on_key_event(key_event)?,
                Event::CollectionEvent(CollectionEvent::AlbumDownloaded(id)) => {
                    self.on_album_downloaded(id)?
                }
                // TODO: update this event to display some kind of error somewhere
                Event::CollectionEvent(CollectionEvent::AlbumDownLoadFailed(id, err)) => {
                    self.on_album_download_failed(id);
                    self.error = format!("{err:?}");
                }
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
                if let Some(album) = self.collection.download_selected_album(self.clone_sender()) {
                    self.player.play(album.into())?
                }
            }
            KeyCode::Up => {
                self.collection.album_state.select_previous();
            }
            KeyCode::Down => {
                self.collection.album_state.select_next();
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

    fn on_album_downloaded(&mut self, id: u32) -> Result<()> {
        let album = self
            .collection
            .albums
            .iter_mut()
            .find(|album| album.id == id)
            .unwrap();

        // TODO: is it possible to move this mutations to the collection struct
        album.download_status = DownloadStatus::Downloaded;

        self.player.play_if_empty(album.into())
    }

    fn on_album_download_failed(&mut self, id: u32) {
        let album = self
            .collection
            .albums
            .iter_mut()
            .find(|album| album.id == id)
            .unwrap();

        // TODO: is it possible to move this mutations to the collection struct
        album.download_status = DownloadStatus::DownloadFailed;
    }

    // TODO: Holy shit this is so bad, for so many reasons.
    fn download_all(&mut self) {
        let senders =
            Vec::from_iter((0..self.collection.albums.len()).map(|i| (i, self.clone_sender())));
        senders
            .into_iter()
            .map(|(i, sender)| self.collection.albums.get_mut(i).unwrap().download(sender))
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

impl From<Album> for crate::player::Album {
    fn from(value: Album) -> Self {
        crate::player::Album {
            title: value.title,
            tracks: value
                .tracks
                .into_iter()
                .map(|track| crate::player::Track {
                    number: track.number,
                    title: track.title,
                    file_path: track.file_path,
                })
                .collect(),
            band_name: value.band_name,
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [header, body, footer, error] = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .areas(area);

        Line::from("rusty piano")
            .alignment(Alignment::Center)
            .render(header, buf);

        let [left, right] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(body);

        Widget::render(&mut self.collection, left, buf);

        Widget::render(&mut self.player, right, buf);

        Line::from(
            "'↑/↓' select album | 'enter' play album | 'spacebar' play/pause | '←/→' previous/next track | 'q' quit",
        )
        .alignment(Alignment::Center)
        .render(footer, buf);

        Line::from(self.error.clone()).render(error, buf);
    }
}

impl Widget for &mut Collection {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let album_titles = self
            .albums
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

        StatefulWidget::render(list, area, buf, &mut self.album_state);
    }
}
