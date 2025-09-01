use rodio::{Decoder, OutputStream, Sink};
use std::{fs::File, path::PathBuf};

pub struct Player {
    sink: Sink,
}

impl Player {
    pub fn new(audio_output_stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(audio_output_stream.mixer());

        Self { sink }
    }

    pub fn play(&mut self, album: Album) {
        self.sink.clear();

        album.tracks.iter().for_each(|track| {
            let file =
                File::open(&track.file_path).expect("Song should have been downloaded already 😬");
            let source = Decoder::try_from(file).expect("Error decoding file");
            self.sink.append(source);
        });

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
}

pub struct Album {
    pub tracks: Vec<Track>,
}

pub struct Track {
    pub file_path: PathBuf,
}
