use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, List, ListState, StatefulWidget, Widget};
use rodio::{Decoder, OutputStream, Sink};
use std::{fs::File, path::PathBuf, usize};

pub struct Player {
    sink: Sink,
    album: Option<Album>,
    tracks_state: ListState,
}

impl Player {
    pub fn new(audio_output_stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(audio_output_stream.mixer());

        Self {
            sink,
            album: None,
            tracks_state: ListState::default(),
        }
    }

    pub fn play(&mut self, album: Album) -> Result<()> {
        self.sink.stop();
        self.tracks_state = ListState::default();
        self.album = Some(album);
        self.play_track(0)
    }

    pub fn play_if_empty(&mut self, album: Album) -> Result<()> {
        match self.album {
            None => self.play(album),
            Some(_) => Ok(()),
        }
    }

    pub fn toggle_playback(&self) {
        match self.sink.is_paused() {
            true => self.sink.play(),
            false => self.sink.pause(),
        }
    }

    pub fn play_previous_track(&mut self) -> Result<()> {
        if self.album.is_some() {
            let current_index = self.tracks_state.selected();
            let previous_index = current_index.filter(|i| *i > 0).map_or(0, |i| i - 1);

            self.play_track(previous_index)
        } else {
            Ok(())
        }
    }

    pub fn play_next_track(&mut self) -> Result<()> {
        if self.album.is_some() {
            let current_index = self.tracks_state.selected();
            let track_index = current_index.map_or(0, |i| i + 1);

            self.play_track(track_index)
        } else {
            Ok(())
        }
    }

    // Plays the next track iff a track is not currently loaded
    // A paused track is still considered loaded
    pub fn try_play_next_track(&mut self) -> Result<()> {
        match self.sink.empty() {
            true => self.play_next_track(),
            false => Ok(()),
        }
    }

    fn play_track(&mut self, track_index: usize) -> Result<()> {
        if let Some(track) = self.album.as_ref().unwrap().tracks.get(track_index) {
            self.sink.stop();
            let file = File::open(&track.file_path)?;
            let source = Decoder::try_from(file)?;
            self.sink.append(source);
            self.sink.play();

            self.tracks_state.select(Some(track_index));
        }

        Ok(())
    }
}

pub struct Album {
    pub title: String,
    pub band_name: String,
    pub tracks: Vec<Track>,
}

pub struct Track {
    pub number: u8,
    pub title: String,
    pub file_path: PathBuf,
}

impl Widget for &mut Player {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let tracks: Vec<String> = self.album.as_ref().map_or(Vec::new(), |album| {
            album
                .tracks
                .iter()
                .map(|track| format!("{:02} - {}", track.number, track.title))
                .collect()
        });

        let title = self
            .album
            .as_ref()
            .map_or(format!(""), |a| format!("{} by {}", a.title, a.band_name));

        let list = List::new(tracks)
            .highlight_symbol("▶️")
            .block(Block::bordered().title(title));

        StatefulWidget::render(list, area, buf, &mut self.tracks_state);
    }
}
