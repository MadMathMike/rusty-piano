use reqwest::StatusCode;
use std::fs::File;
use std::io::copy;

use rusty_piano::app::authenticate_with_bandcamp;

fn main() {
    // TODO: Add option to bypass authentication to support offline-mode play

    let client = authenticate_with_bandcamp();

    let collection = client.get_collection();

    println!("{collection:?}");

    // Download track to temp location
    let track = collection.items.first().unwrap().tracks.first().unwrap();
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

    copy(&mut download_response, &mut temp_file).unwrap();

    // let path = "file_example_MP3_2MG.mp3";
    let path = temp_file_path;
    let file = File::open(path).expect("Error opening file");

    rusty_piano::sound::play_source_sample(file);
}
