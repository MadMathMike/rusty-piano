use rodio::{Decoder, OutputStream, Sink};
use std::{fs::File, path::PathBuf};

pub struct Player {
    sink: Sink,
    // album: Option<Album>,
    tracks: Vec<Track>,
    currently_playing: Option<usize>,
}

impl Player {
    pub fn new(audio_output_stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(audio_output_stream.mixer());

        Self {
            sink,
            tracks: vec![],
            currently_playing: None,
        }
    }

    pub fn play(&mut self, album: Album) {
        self.sink.clear();
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

        let next_track_number = match self.currently_playing {
            Some(track_number) => track_number + 1,
            None => 1,
        };

        // track_number is one-based, but the tracks vec is zero-based
        match self.tracks.get(next_track_number - 1) {
            Some(track) => {
                self.currently_playing = Some(next_track_number);
                self.play_track(track);
            }
            // TODO: should we unload the album at this point?
            None => {} // No more tracks to play
        }
    }
}

pub struct Album {
    pub tracks: Vec<Track>,
}

pub struct Track {
    pub file_path: PathBuf,
}
