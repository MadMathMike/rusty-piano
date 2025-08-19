use reqwest::StatusCode;
use std::env::var;
use std::fs::File;
use std::io::copy;

use rusty_piano::{bandcamp::*, secrets::*};

fn main() {
    // TODO: Add option to bypass authentication to support offline-mode play
    
    let client = match get_access_token() {
        Some(token) => BandCampClient::init_with_token(token.clone()).or_else(login),
        None => login(),
    }.expect("Failed initialization. Bad token or credentials. Or something...");

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

fn login() -> Option<BandCampClient> {
    println!("Attempting login...");

    let username = var("BANDCAMP_USERNAME")
        .map_or(None, |username| {
            if username.is_empty() {
                None
            } else {
                Some(username)
            }
        })
        .unwrap_or_else(|| prompt("username"));

    let password = var("BANDCAMP_PASSWORD")
        .map_or(None, |username| {
            if username.is_empty() {
                None
            } else {
                Some(username)
            }
        })
        .unwrap_or_else(|| prompt("password"));

    BandCampClient::init(&username, &password).map(|tuple| {
        store_access_token(&tuple.1);
        tuple.0
    })
}

fn prompt(param: &str) -> String {
    println!("Enter your bandcamp {}:", param);
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Error reading standard in");
    input
}
