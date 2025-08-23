use std::{fs::File, io::copy, thread::sleep};

use reqwest::StatusCode;
use rodio::{Decoder, OutputStreamBuilder};

use crate::bandcamp::Track;

pub enum PlaybackCommands
{
    Exit,
    Play(Track)
}

pub fn play_track(track: &Track) {
    let url = &track.hq_audio_url;
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push(format!("{}.mp3", track.track_id));
    let temp_file_path = temp_dir.as_path();
    let mut temp_file = File::create(temp_file_path).unwrap();

    let mut download_response = reqwest::blocking::Client::new()
        .get(url)
        .send()
        .expect("Error downloading file");
    assert_eq!(StatusCode::OK, download_response.status());

    // TODO: Return an error when Bandcamp returns a 410
    // Bandcamp will return a 410, Gone response when the link is no longer valid
    // I suspect the link is only valid for some amount of time.
    // Maybe as long as the access token, which is about an hour. Not sure.

    copy(&mut download_response, &mut temp_file).unwrap();

    // let path = "file_example_MP3_2MG.mp3";
    let path = temp_file_path;
    let file = File::open(path).expect("Error opening file");

    play_source_sample(file);
}

fn play_source_sample(file: File) {
    let stream_handle =
        OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");

    // TODO: What is this doing?
    let _sink = rodio::Sink::connect_new(stream_handle.mixer());

    let source = Decoder::try_from(file).expect("Error decoding file");
    // Play the sound directly on the device
    stream_handle.mixer().add(source);

    // Note that playback stops when the sink is dropped, which is why we sleep for a bit
    sleep(std::time::Duration::from_secs(5));
}
