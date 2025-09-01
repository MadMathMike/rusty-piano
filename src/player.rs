use rodio::{Decoder, OutputStream, Sink};
use std::{fs::File, path::PathBuf};

pub struct Player {
    sink: Sink,
    album: Option<Album>,
    currently_playing: Option<usize>,
}

impl Player {
    pub fn new(audio_output_stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(audio_output_stream.mixer());

        Self {
            sink,
            album: None,
            currently_playing: None,
        }
    }

    pub fn play(&mut self, album: Album) {
        self.sink.clear();

        self.album = Some(album);
        self.play_track(0);
    }

    fn play_track(&mut self, track_number: usize) {
        if self.album.is_none() {
            panic!("This shouldn't happen... 🤪");
        }
        let track = self
            .album
            .as_ref()
            .unwrap()
            .tracks
            .get(track_number)
            .unwrap();
        let file =
            File::open(&track.file_path).expect("Song should have been downloaded already 😬");
        let source = Decoder::try_from(file).expect("Error decoding file");
        self.sink.append(source);

        self.currently_playing = Some(track_number);
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
            return;
        }

        if let Some(album) = self.album.as_ref() {
            if let Some(_) = album
                .tracks
                .iter()
                .skip(self.currently_playing.unwrap())
                .next()
            {
                self.play_track(self.currently_playing.unwrap() + 1);
            }
        }
    }
}

pub struct Album {
    pub tracks: Vec<Track>,
}

pub struct Track {
    pub file_path: PathBuf,
}
