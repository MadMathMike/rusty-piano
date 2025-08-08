use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use rodio::{Decoder, OutputStreamBuilder};
use serde_json::json;
use std::fs::File;
use std::thread::sleep;

fn main() {
    let mut default_headers = HeaderMap::default();
    default_headers.append(USER_AGENT, HeaderValue::from_static("rusty-piano/0.1"));

    let client = Client::builder()
        .cookie_store(true)
        .default_headers(default_headers)
        .build()
        .unwrap();

    let head_response = client
        .head("https://pandora.com")
        .header(USER_AGENT, "rusty-piano/0.1")
        .send()
        .expect("Error making HEAD request to root domain");

    for cookie in head_response.cookies() {
        println!("{cookie:?}");
    }

    // TODO: securely retrieve password
    let login_body = json!(
    {
        "keepLoggedIn": true,
        "password": "TODO",
        "username": "MichaelPeterson27@live.com"
    });

    let login_response = client
        .post("https://pandora.com/api/v1/auth/login")
        .header(USER_AGENT, "rusty-piano/0.1")
        .json(&login_body)
        .send()
        .expect("Error building loging request");

    // TODO: parse return value

    // TODO: store auth token (for subsequent calls and for next session to bypass login)

    println!("{:?}", login_response.status());
    println!("{}", login_response.text().unwrap());

    play_sample_sound();
}

fn play_sample_sound() {
    let stream_handle =
        OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
    // TODO: What is this doing?
    // Note that playback stops when the sink is dropped
    let _sink = rodio::Sink::connect_new(stream_handle.mixer());
    let path = "file_example_MP3_2MG.mp3";
    let file = File::open(path).expect("Error opening file");
    let source = Decoder::try_from(file).expect("Error decoding file");
    // Play the sound directly on the device
    stream_handle.mixer().add(source);

    sleep(std::time::Duration::from_secs(5));
}
