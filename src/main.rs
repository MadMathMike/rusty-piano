use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::env::var;

fn main() {
    let mut default_headers = HeaderMap::default();
    default_headers.append(USER_AGENT, HeaderValue::from_static("rusty-piano/0.1"));
    default_headers.append("X-Requested-With", HeaderValue::from_static("com.bandcamp.android"));

    let client = Client::builder()
        .cookie_store(true)
        .default_headers(default_headers)
        .build()
        .unwrap();

    // Try to read from keyring
    let user = whoami::username();
    let entry = keyring::Entry::new("rusty-piano", &user).expect("Error creating keyring entry");

    let mut access_token: Option<String> = None;

    if let Ok(secret) = entry.get_secret() {
        access_token = Some(String::from_utf8(secret).unwrap());
        
        // TODO: check if token is expired
    }

    if access_token.is_none() {
        let login_response = login(&client);

        entry
            .set_secret(login_response.access_token.as_bytes())
            .expect("Error setting keyring secret");

        access_token = Some(login_response.access_token)
    }

    let access_token = access_token.unwrap();

    let collection_response = client
        .get("https://bandcamp.com/api/collectionsync/1/collection")
        .query(&[("page_size", 10)])
        .bearer_auth(access_token)
        .send()
        .expect("Error calling collection api");

    println!("{}", collection_response.text().unwrap());

    rusty_piano::sound::play_sample_sound();
}

fn login(client: &Client) -> LoginResponse {
    // TODO: once we have a UI, prompt for the password instead of looking in the environment variables
    let password = var("BANDCAMP_PASSWORD")
        .expect("Error retreiving BANDCAMP_PASSWORD from environment variables");

    let mut params = HashMap::new();
    params.insert("grant_type", "password".to_owned());
    params.insert("username", "MichaelPeterson27@live.com".to_owned());
    params.insert("password", password.to_owned());
    params.insert("client_id", "134".to_owned());
    params.insert("client_secret", "1myK12VeCL3dWl9o/ncV2VyUUbOJuNPVJK6bZZJxHvk=".to_owned());

    let mut login_request = client
        .post("https://bandcamp.com/oauth_login")
        .form(&params)
        .build()
        .unwrap();

    let body_bytes = login_request.try_clone().unwrap().body().unwrap().as_bytes().unwrap().to_vec();
    let body_string = String::from_utf8(body_bytes).unwrap();

    let hashed_body = rusty_piano::crypto::sha1_hex("dtmfa", &body_string);
    let x_bandcamp_dm = HeaderValue::from_str(&hashed_body).unwrap();

    login_request.headers_mut().append("X-Bandcamp-Dm", x_bandcamp_dm);

    let login_response = client.execute(login_request)
        .expect("Error making call to oauth_login");

    println!("{:?}", login_response.headers());

    assert_eq!(StatusCode::IM_A_TEAPOT, login_response.status());

    let x_bandcamp_dm_header = login_response.headers().get("X-Bandcamp-Dm").unwrap();
    let x_bandcamp_dm = x_bandcamp_dm_header.to_str().unwrap();
    let last_char = x_bandcamp_dm.chars().rev().take(1).collect::<String>();
    let i = usize::from_str_radix(&last_char, 16).unwrap();
    let ith_char = x_bandcamp_dm.chars().nth(i).unwrap();
    let algorithm = usize::from_str_radix(&String::from(ith_char), 16).unwrap();
    println!("{algorithm}");
    assert_eq!(1, algorithm);
    // Calculate new x-bandcamp-dm header based on int value
        // if 3 => hmacsha256 of stuff
        // if 4 => hmacsha512 of stuff?
        // otherwise
            // if 1 => hmac_sha1([0..19] + [22..] + body)
            // else

    let mut to_hash = String::with_capacity(38);
    to_hash.push_str(&x_bandcamp_dm[0..19]);
    to_hash.push_str(&x_bandcamp_dm[22..]);

    let mut login_request = client
        .post("https://bandcamp.com/oauth_login")
        .form(&params)
        .build()
        .unwrap();

    let body_bytes = login_request.try_clone().unwrap().body().unwrap().as_bytes().unwrap().to_vec();
    let body_string = String::from_utf8(body_bytes).unwrap();
    to_hash.push_str(&body_string);

    let hashed_body = rusty_piano::crypto::sha1_hex("dtmfa", &to_hash);
    let x_bandcamp_dm = HeaderValue::from_str(&hashed_body).unwrap();

    login_request.headers_mut().append("X-Bandcamp-Dm", x_bandcamp_dm);

    let login_response = client.execute(login_request)
        .expect("Error making call to oauth_login");

    println!("Login status code: {:?}", login_response.status());

    assert_eq!(login_response.status(), StatusCode::OK);

    // TODO: Also need to examine X-Bandcamp-Dm and X-Bandcamp-Pow headers on response

    // TODO: if parsing fails, it could be because we received a response like this:
    // {"error":"emailVerificationRequired","error_description":"Please first re-verify your account using the link we just emailed to you."}
    let login_response = login_response
        .json::<LoginResponse>()
        .expect("Failed to parse login response");

    // TODO: check if login_response.ok is true?

    println!("{login_response:?}");

    login_response
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    // pub ok: bool,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: f64,
    // pub refresh_token: String,
}
