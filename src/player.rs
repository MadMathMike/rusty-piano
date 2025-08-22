use std::{fs::File, io::copy};

use reqwest::StatusCode;

use crate::{bandcamp::Track, sound::play_source_sample};

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
