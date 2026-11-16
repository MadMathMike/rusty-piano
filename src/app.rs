use crate::bandcamp::Item;
use crate::collection::{Album, Collection};
use crate::download_manager::DownloadManager;
use crate::events::Event;
use crate::player::Player;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::prelude::*;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Widget};
use rodio::OutputStream;
use std::sync::mpsc::{self, Sender, TryRecvError};
use tokio::runtime::Runtime;

pub struct App {
    pub exit: bool,
    channel: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
    collection: Collection,
    // TODO: should the collection just have the download manager? Probably...
    download_manager: DownloadManager,
    player: Player,
    login: Option<LoginForm>, // If Some, the modal should be displayed
    error: String,
}

impl App {
    pub fn new(
        bandcamp_items: Vec<Item>,
        audio_output_stream: &OutputStream,
        download_runtime: Runtime,
    ) -> Self {
        let channel = mpsc::channel();
        let collection = Collection::from_bandcamp_items(bandcamp_items);
        let download_manager = DownloadManager::new(channel.0.clone(), download_runtime);
        let player = Player::new(audio_output_stream);

        App {
            exit: false,
            collection,
            download_manager,
            channel,
            player,
            login: None,
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
                    if let Some(album) = self.collection.set_downloaded(id) {
                        self.player.play_if_empty(album.into())?
                    }
                }
                Event::AlbumDownLoadFailed(id, err) => {
                    self.error = format!("{err:?}");
                    self.collection.set_failed(id);
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

        // TODO: If this is where global key press event handling occurs,
        // we need to detect if we're in editing mode for a login page.
        // If we are, we need need to identify keys that will toggle or break out of
        // edit mode. And the rest (maybe?) maybe need to be sent to the input.

        if let Some(form) = &mut self.login {
            match key.code {
                KeyCode::Esc => self.login = None,
                _ => {}
            }

            return Ok(());
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(album) = self
                    .collection
                    .download_selected_album(&self.download_manager)
                {
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
            KeyCode::Char('d') => self.collection.download_all(&self.download_manager),
            KeyCode::Char('l') => self.login = Some(LoginForm),
            KeyCode::Char('q') => self.exit = true,
            _ => (),
        };

        Ok(())
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

        // TODO: conditionally add the "'l'" login part of the header if we don't have an authenticated bandcamp client
        Line::from("rusty piano | 'l' login")
            .alignment(Alignment::Center)
            .render(header, buf);

        // TODO: render a login screen in the body if we don't have an authorized client

        let [left, right] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(body);

        Widget::render(&mut self.collection, left, buf);

        Widget::render(&mut self.player, right, buf);

        // Overlay the login box
        if let Some(_form) = &self.login {
            let modal = body.centered(Constraint::Length(53), Constraint::Length(10));
            Clear.render(modal, buf);

            let [modal_header, modal_username, modal_password, modal_info] = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(vec![
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .areas(modal);
            Line::from("`esc` abort | '⇥' switch field | `↵` submit")
                .alignment(Alignment::Center)
                .render(modal_header, buf);
            Block::bordered()
                .title("username")
                .render(modal_username, buf);
            Block::bordered()
                .title("password")
                .render(modal_password, buf);
        }

        Line::from(
            "'↑/↓' select album | '↵' play album | '␣' play/pause | '←/→' previous/next track | 'q' quit",
        )
        .alignment(Alignment::Center)
        .render(footer, buf);

        Line::from(self.error.clone()).render(error, buf);
    }
}

impl From<&Album> for crate::player::Album {
    fn from(value: &Album) -> Self {
        Self {
            title: value.title.clone(),
            tracks: value
                .tracks
                .iter()
                .map(|track| crate::player::Track {
                    number: track.number.clone(),
                    title: track.title.clone(),
                    file_path: track.file_path.clone(),
                })
                .collect(),
            band_name: value.band_name.clone(),
        }
    }
}

struct LoginForm;
