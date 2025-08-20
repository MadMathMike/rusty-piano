use std::{collections::HashMap};

use reqwest::{
    StatusCode,
    blocking::Client,
    header::{HeaderMap, HeaderValue, USER_AGENT},
};
use serde::Deserialize;

use crate::crypto;

// TODO: This will probably be a good place to keep auth_tokens and refresh_tokens for future calls...
pub struct BandCampClient {
    client: Client,
    token: Option<String>
}

#[allow(clippy::new_without_default)]
impl BandCampClient {
    fn new() -> Self {
        let mut default_headers = HeaderMap::default();
        default_headers.append(USER_AGENT, HeaderValue::from_static("rusty-piano/0.1"));
        default_headers.append(
            "X-Requested-With",
            HeaderValue::from_static("com.bandcamp.android"),
        );

        // TODO: figure out debug logging or something? Maybe it can be configured on the reqwest client?
        let client = Client::builder()
            .cookie_store(true)
            .default_headers(default_headers)
            .build()
            .expect("Error creating reqwest client");

        Self { client, token: None }
    }

    pub fn init(username: &str, password: &str) -> Option<(BandCampClient, String)> {
        let mut bandcamp_client = BandCampClient::new();
        let login_response = bandcamp_client.login(username, password);

        bandcamp_client.token = Some(login_response.access_token.clone());
        Some((bandcamp_client, login_response.access_token))
    }
    
    pub fn init_with_token(token: String) -> Option<BandCampClient> {
        let mut bandcamp_client = BandCampClient::new();
        let collection_response = bandcamp_client
            .client
            .get("https://bandcamp.com/api/collectionsync/1/collection")
            .query(&[("page_size", "1"), ("tralbum_type", "a"), ("enc", "alac")])
            .bearer_auth(token.clone())
            .send()
            .expect("Error calling collection api");

        if StatusCode::is_success(&collection_response.status()) {
            bandcamp_client.token = Some(token);
            Some(bandcamp_client)
        } else {
            None
        }
    }

    // TODO: If the re-auth flow using the refresh_token is ever figured out, add the refresh_token as a parameter (maybe)
    // https://github.com/Metalnem/bandcamp-downloader
    // https://mijailovic.net/2024/04/04/bandcamp-auth/
    fn login(&self, username: &str, password: &str) -> LoginResponse {
        let mut params = HashMap::new();
        params.insert("username", username);
        params.insert("password", password);
        params.insert("grant_type", "password");
        params.insert("client_id", "134");
        params.insert(
            "client_secret",
            "1myK12VeCL3dWl9o/ncV2VyUUbOJuNPVJK6bZZJxHvk=",
        );

        // TODO: Instead of building a request, getting the body, then calculating the x-bandcamp-dm header
        // Manually construct the string body so the x-bandcamp-dm header can be calculated before creating
        // the request object.

        let mut login_request = self
            .client
            .post("https://bandcamp.com/oauth_login")
            .form(&params)
            .build()
            .unwrap();

        let login_request_clone = login_request.try_clone().unwrap();
        let body_bytes = login_request_clone.body().unwrap().as_bytes().unwrap();

        let hashed_body = crypto::hmac_sha1_from_bytes_as_hex("dtmfa", body_bytes);
        let x_bandcamp_dm = HeaderValue::from_str(&hashed_body).unwrap();

        login_request
            .headers_mut()
            .append("X-Bandcamp-Dm", x_bandcamp_dm);

        let login_response = self
            .client
            .execute(login_request)
            .expect("Error making call to oauth_login");

        assert_eq!(StatusCode::IM_A_TEAPOT, login_response.status());

        let x_bandcamp_dm_header = login_response.headers().get("X-Bandcamp-Dm").unwrap();
        let x_bandcamp_dm = x_bandcamp_dm_header.to_str().unwrap();
        let last_char = x_bandcamp_dm.chars().rev().take(1).collect::<String>();
        let i = usize::from_str_radix(&last_char, 16).unwrap();
        let ith_char = x_bandcamp_dm.chars().nth(i).unwrap();
        let algorithm = usize::from_str_radix(&String::from(ith_char), 16).unwrap();
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

        let mut login_request = self
            .client
            .post("https://bandcamp.com/oauth_login")
            .form(&params)
            .build()
            .unwrap();

        let body_bytes = login_request
            .try_clone()
            .unwrap()
            .body()
            .unwrap()
            .as_bytes()
            .unwrap()
            .to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        to_hash.push_str(&body_string);

        let hashed_body = crypto::hmac_sha1_as_hex("dtmfa", &to_hash);
        let x_bandcamp_dm = HeaderValue::from_str(&hashed_body).unwrap();

        login_request
            .headers_mut()
            .append("X-Bandcamp-Dm", x_bandcamp_dm);

        let login_response = self
            .client
            .execute(login_request)
            .expect("Error making call to oauth_login");

        assert_eq!(login_response.status(), StatusCode::OK);

        let response_body = login_response.text().unwrap();

        println!("{}", &response_body);

        // TODO: if parsing fails, it could be because we received a response like this:
        // {"error":"emailVerificationRequired","error_description":"Please first re-verify your account using the link we just emailed to you."}
        // {"error":"nameNoMatch","error_description":"Unknown username or email"}
        // TODO: check if login_response.ok is true?
        // login_response
        //     .json::<LoginResponse>()
        //     .expect("Failed to parse login response")
        serde_json::from_str::<LoginResponse>(&response_body).expect("Failed to parse login response")
    }

    // Note: offset param used for paging
    pub fn get_collection(&self) -> CollectionResponse {
        let collection_response = self
            .client
            .get("https://bandcamp.com/api/collectionsync/1/collection")
            .query(&[("page_size", "2"), ("tralbum_type", "a"), ("enc", "alac")])
            .bearer_auth(self.token.clone().expect("Uninitialized client"))
            .send()
            .expect("Error calling collection api");

        collection_response
            .json::<CollectionResponse>()
            .expect("Failure parsing collection response")
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    // pub ok: bool,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u32,
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct CollectionResponse {
    pub items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub tralbum_type: String,
    pub tralbum_id: u32,
    pub sale_item_type: String,
    pub title: String,
    pub tracks: Vec<Track>,
    pub band_info: BandInfo,
}

#[derive(Debug, Deserialize)]
pub struct Track {
    pub track_id: u32,
    pub title: String,
    pub hq_audio_url: String,
    pub track_number: u8,
}

#[derive(Debug, Deserialize)]
pub struct BandInfo {
    pub band_id: u32,
    pub name: String,
    pub bio: String,
    pub page_url: String,
}
