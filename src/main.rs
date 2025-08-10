use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use rodio::{Decoder, OutputStreamBuilder};
use serde::Deserialize;
use serde_json::json;
use std::env::var;
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

    // TODO how to handle non-OK responses
    let csrf_token_cookie = head_response
        .cookies()
        .find(|c| c.name() == "csrftoken")
        .expect("csrftoken cookie not found");
    let csrf_token = csrf_token_cookie.value();

    // Try to read from keyring
    let user = whoami::username();
    let entry = keyring::Entry::new("rusty-piano", &user).expect("Error creating keyring entry");

    let mut auth_token: Option<String> = None;

    if let Ok(secret) = entry.get_secret() {
        auth_token = Some(String::from_utf8(secret).unwrap());
    }

    // We can get an expired auth token response:
    /*
        Albums request status code: 401
        "{\"message\":\"Auth Token is Expired - VIdIh9cGVJX4v6QBj7EXyEGgmStLGDOsa+HCgYGGBGB9I=\",\"errorCode\":1001,\"errorString\":\"INVALID_REQUEST\"}"
    */

    if auth_token.is_none() {
        auth_token = Some(login(&client));
    }

    let albums_body = json!({"request":{"sortOrder":"MOST_RECENT_ADDED","offset":0,"limit":40,"annotationLimit":40,"typePrefixes":["AL"]}});

    let mut albums_response = client
        .post("https://pandora.com/api/v6/collections/getSortedByTypes")
        .json(&albums_body)
        .header("x-authtoken", auth_token.clone().unwrap())
        .header("x-csrftoken", csrf_token)
        .send()
        .expect("Error getting collections");

    println!("Albums request status code: {:?}", albums_response.status());

    // Question: would persistent cookies make it more likely that an old auth token would still be valid?
    // In the web app, simply reloading the page yields a new csrftoken, so it isn't that value that could keep the token alive longer...
    // There is a lithiumSSO:pandora.prod cookie that might have more potential for keeping us logged in
    if albums_response.status() == StatusCode::UNAUTHORIZED {
        auth_token = Some(login(&client));

        albums_response = client
            .post("https://pandora.com/api/v6/collections/getSortedByTypes")
            .json(&albums_body)
            .header("x-authtoken", auth_token.clone().unwrap())
            .header("x-csrftoken", csrf_token)
            .send()
            .expect("Error getting collections");

        println!("Albums request status code: {:?}", albums_response.status());
    }

    println!("{:?}", albums_response.text().unwrap());

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    pub auth_token: String,
}

fn login(client: &Client) -> String {
    // TODO: once we have a UI, prompt for the password isntead of looking in the environment variables
    let password = var("PANDORA_PASSWORD")
        .expect("Error retreiving PANDORA_PASSWORD from environment variables");

    let login_body = json!({
        "keepLoggedIn": true,
        "password": password,
        "username": "MichaelPeterson27@live.com"
    });

    let login_response = client
        .post("https://pandora.com/api/v1/auth/login")
        .header(USER_AGENT, "rusty-piano/0.1")
        .json(&login_body)
        .send()
        .expect("Error calling login");

    println!("Login status code: {:?}", login_response.status());

    let login_response = login_response
        .json::<LoginResponse>()
        .expect("Failed to parse login response");

    let user = whoami::username();
    let entry = keyring::Entry::new("rusty-piano", &user).expect("Error creating keyring entry");
    entry
        .set_secret(login_response.auth_token.as_bytes())
        .expect("Error setting keyring secret");

    login_response.auth_token
}
