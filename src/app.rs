use crate::bandcamp::Item;
use crate::collection::{Album, Collection, DownloadStatus};
use crate::events::Event;
use crate::player::Player;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::prelude::*;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use rodio::OutputStream;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::{self, Sender, TryRecvError};

pub struct App {
    pub exit: bool,
    collection: Collection,
    channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
    player: Player,
    error: String,
}

impl App {
    pub fn new(bandcamp_items: Vec<Item>, audio_output_stream: &OutputStream) -> Self {
        let collection = Collection::from_bandcamp_items(bandcamp_items);

        let channel = mpsc::channel();

        let player = Player::new(audio_output_stream);

        App {
            exit: false,
            collection,
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
                Event::AlbumDownloaded(id) => {
                    self.collection.get_album_mut(id).map_or(Ok(()), |album| {
                        // TODO: is it possible to move this mutation to the collection struct
                        album.download_status = DownloadStatus::Downloaded;
                        self.player.play_if_empty(album.into())
                    })?
                }
                Event::AlbumDownLoadFailed(id, err) => {
                    self.error = format!("{err:?}");
                    if let Some(album) = self.collection.get_album_mut(id) {
                        // TODO: is it possible to move this mutation to the collection struct
                        album.download_status = DownloadStatus::DownloadFailed;
                    }
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
            KeyCode::Char('d') => self.collection.download_all(self.clone_sender()),
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
}

impl From<&mut Album> for crate::player::Album {
    fn from(value: &mut Album) -> Self {
        value.clone().into()
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
