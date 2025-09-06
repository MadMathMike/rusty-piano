use anyhow::Result;
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
        self.play_track_number(0)?;

        Ok(())
    }

    pub fn play_if_empty(&mut self, album: Album) -> Result<()> {
        if self.sink.empty() {
            self.play(album)?;
        }

        Ok(())
    }

    pub fn toggle_playback(&self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
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

        if previous_track_number.is_some() {
            self.play_track_number(previous_track_number.unwrap())?;
        }

        Ok(())
    }

    pub fn play_next_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() {
            return Ok(());
        }

        let next_track_number = match self.tracks_state.selected() {
            Some(track_number) => track_number + 1,
            None => 0,
        };

        self.play_track_number(next_track_number)?;

        Ok(())
    }

    // Only plays next track if a track is not currently "loaded".
    // i.e., Will not got to the next track even if a song is paused
    pub fn try_play_next_track(&mut self) -> Result<()> {
        if !self.sink.empty() {
            // We're currently playing something
            return Ok(());
        }

        self.play_next_track()?;

        Ok(())
    }

    fn play_track_number(&mut self, track_number: usize) -> Result<()> {
        if let Some(track) = self.tracks.get(track_number) {
            self.sink.stop();
            let file = File::open(&track.file_path)?;
            let source = Decoder::try_from(file)?;
            self.sink.append(source);
            self.sink.play();

            self.tracks_state.select(Some(track_number));
        }

        Ok(())
    }
}

pub struct Album {
    pub title: String,
    pub tracks: Vec<Track>,
}

pub struct Track {
    pub title: String,
    pub file_path: PathBuf,
}

impl Widget for &mut Player {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        // TODO: change this to enumerate
        let track_titles = (0..self.tracks.len())
            .map(|index| {
                let track = &self.tracks[index];
                format!("{index:02} - {}", track.title)
            })
            .collect::<Vec<String>>();

        let list = List::new(track_titles)
            .highlight_symbol("▶️")
            .block(Block::bordered().title(self.header.clone()));

        StatefulWidget::render(list, area, buf, &mut self.tracks_state);
    }
}
