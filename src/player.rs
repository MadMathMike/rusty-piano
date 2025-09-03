use ratatui::widgets::{Block, List, ListState, StatefulWidget, Widget};
use rodio::{Decoder, OutputStream, Sink};
use std::{fs::File, path::PathBuf};

pub struct Player {
    sink: Sink,
    // album: Option<Album>,
    header: String,
    tracks: Vec<Track>,
    tracks_state: ListState,
}

impl Player {
    pub fn new(audio_output_stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(audio_output_stream.mixer());

        Self {
            sink,
            header: "Album".to_owned(),
            tracks: vec![],
            tracks_state: ListState::default(),
        }
    }

    pub fn play(&mut self, album: Album) {
        self.sink.clear();
        self.tracks_state = ListState::default();
        self.header = album.title;
        self.tracks = album.tracks;
        self.play_next_if_track_finished();
    }

    fn play_track(&self, track: &Track) {
        let file =
            File::open(&track.file_path).expect("Song should have been downloaded already 😬");
        let source = Decoder::try_from(file).expect("Error decoding file");
        self.sink.append(source);
        self.sink.play();
    }

    pub fn play_if_empty(&mut self, album: Album) {
        if self.sink.empty() {
            return;
        }

        self.play(album)
    }

    pub fn toggle_playback(&self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn play_next_if_track_finished(&mut self) {
        if !self.sink.empty() {
            // We're currently playing something
            return;
        }

        if self.tracks.is_empty() {
            // We have nothing loaded
            return;
        }

        let next_track_number = match self.tracks_state.selected() {
            Some(track_number) => track_number + 1,
            None => 0,
        };

        match self.tracks.get(next_track_number) {
            Some(track) => {
                self.tracks_state.select_next();
                self.play_track(track);
            }
            // TODO: should we unload the album at this point?
            None => {} // No more tracks to play
        }
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
        let track_titles = (0..self.tracks.len())
            .map(|index| {
                let track = &self.tracks[index];
                let icon = ' ';
                format!("{} {icon}", track.title)
            })
            .collect::<Vec<String>>();

        let list = List::new(track_titles)
            .highlight_symbol("▶️")
            .block(Block::bordered().title(self.header.clone()));

        StatefulWidget::render(list, area, buf, &mut self.tracks_state);
    }
}
