use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, List, ListState, StatefulWidget, Widget};
use rodio::{Decoder, OutputStream, Sink};
use std::{fs::File, path::PathBuf, usize};

pub struct Player {
    sink: Sink,
    header: String,
    tracks: Vec<Track>,
    tracks_state: ListState,
}

impl Player {
    pub fn new(audio_output_stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(audio_output_stream.mixer());

        Self {
            sink,
            header: "".to_owned(),
            tracks: vec![],
            tracks_state: ListState::default(),
        }
    }

    pub fn play(&mut self, album: Album) -> Result<()> {
        self.sink.stop();
        self.tracks_state = ListState::default();
        self.header = album.title;
        self.tracks = album.tracks;
        self.play_track(0)?;

        Ok(())
    }

    pub fn play_if_empty(&mut self, album: Album) -> Result<()> {
        match self.sink.empty() {
            true => self.play(album),
            false => Ok(()),
        }
    }

    pub fn toggle_playback(&self) {
        match self.sink.is_paused() {
            true => self.sink.play(),
            false => self.sink.pause(),
        }
    }

    pub fn play_previous_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() {
            return Ok(());
        }

        let previous_track_number = match self.tracks_state.selected() {
            Some(track_number) => match track_number {
                1.. => Some(track_number - 1),
                0 => Some(0),
            },
            None => Some(0),
        };

        match previous_track_number.is_some() {
            true => self.play_track(previous_track_number.unwrap()),
            false => Ok(()),
        }
    }

    pub fn play_next_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() {
            return Ok(());
        }

        let next_track_number = match self.tracks_state.selected() {
            Some(track_number) => track_number + 1,
            None => 0,
        };

        self.play_track(next_track_number)
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
        if let Some(track) = self.tracks.get(track_index) {
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
        let track_titles: Vec<String> = self
            .tracks
            .iter()
            .map(|track| format!("{:02} - {}", track.number, track.title))
            .collect();

        let list = List::new(track_titles)
            .highlight_symbol("▶️")
            .block(Block::bordered().title(self.header.clone()));

        StatefulWidget::render(list, area, buf, &mut self.tracks_state);
    }
}
