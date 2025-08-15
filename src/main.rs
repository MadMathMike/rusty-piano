use reqwest::StatusCode;
use std::env::var;
use std::fs::File;
use std::io::copy;


use rusty_piano::{bandcamp, secrets::*};

fn main() {
    let client = bandcamp::BandCampClient::new();

    let mut access_token = get_access_token();

    if access_token.is_none() {
        // TODO: once we have a UI, prompt for the password instead of looking in the environment variables
        let password = var("BANDCAMP_PASSWORD")
            .expect("Error retreiving BANDCAMP_PASSWORD from environment variables");

        let login_response = client.login("MichaelPeterson27@live.com", &password);

        store_access_token(&login_response.access_token);

        access_token = Some(login_response.access_token)
    }

    let access_token = access_token.unwrap();

    /* offset param used for paging */
    let collection = client.get_collection(access_token);

    println!("{collection:?}");

    // Download track to temp location
    let track = collection.items.first().unwrap().tracks.first().unwrap();
    let url = &track.hq_audio_url;
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push(format!("{}.mp3", track.track_id));
    let temp_file_path = temp_dir.as_path();
    let mut temp_file = File::create(temp_file_path).unwrap();

    let mut download_response = reqwest::blocking::Client::new().get(url).send().expect("Error downloading file");
    assert_eq!(StatusCode::OK, download_response.status());

    copy(&mut download_response, &mut temp_file).unwrap();

    // let path = "file_example_MP3_2MG.mp3";
    let path = temp_file_path;
    let file = File::open(path).expect("Error opening file");

    rusty_piano::sound::play_source_sample(file);
}
