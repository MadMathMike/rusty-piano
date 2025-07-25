use reqwest::blocking::Client;
use rodio::{Decoder, OutputStreamBuilder};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::thread::sleep;

#[derive(Debug, Deserialize)]
struct PartnerAuthResult {
    #[serde(rename = "partnerId")]
    pub partner_id: String,
    #[serde(rename = "partnerAuthToken")]
    pub partner_auth_token: String,
    #[serde(rename = "syncTime")]
    pub sync_time: String
}

#[derive(Debug, Deserialize)]
struct PartnerAuthResponse {
    pub stat: String,
    pub result: PartnerAuthResult
}

fn main() {
    let auth_body = json!({
        "username": "pandora one",
        "password": "TVCKIBGS9AO9TSYLNNFUML0743LH82D",
        "deviceModel": "D01",
        "version": "5"
    });

    let request_uri = "https://internal-tuner.pandora.com/services/json/?method=auth.partnerLogin";

    // Send auth request
    let client = Client::new();
    
    let response = client.post(request_uri)
        .json(&auth_body)
        .send()
        .expect("Error making auth call");

    println!("Auth status code: {:?}", response.status());

    // TODO: Error out on response status code != 200

    let partner_auth_response = response.json::<PartnerAuthResponse>().expect("Failed to parse partner auth response");
    println!("{:?}", partner_auth_response);

    // Not needed?
    // TODO: Encrypt with blowfish
    // TODO: Convert to hexidecimal

    fun_name();
}

fn fun_name() {
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
