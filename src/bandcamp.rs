use anyhow::Result;
use reqwest::{
    StatusCode,
    blocking::Client,
    header::{HeaderMap, HeaderValue, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use thiserror::Error;

pub struct BandCampClient {
    client: Client,
    access_token: String,
}

impl BandCampClient {
    pub fn new(username: &str, password: &str) -> Result<Self, LoginError> {
        let mut default_headers = HeaderMap::default();
        // TODO: user agent can/should be an input parameter so it can be shared across other clients?
        default_headers.append(USER_AGENT, HeaderValue::from_static("rusty-piano/0.1"));
        default_headers.append(
            "X-Requested-With",
            HeaderValue::from_static("com.bandcamp.android"),
        );

        let client = Client::builder()
            .cookie_store(true)
            .default_headers(default_headers)
            .build()
            .expect("Error creating reqwest client");

        let login_response = login(&client, username, password)?;

        Ok(Self {
            client,
            access_token: login_response.access_token.clone(),
        })
    }

    // TODO: would be fun to turn this into an iter, maybe an async iter
    pub fn get_entire_collection(&self, page_size: usize) -> Vec<Item> {
        let mut offset = String::new();
        let mut items: Vec<Item> = Vec::new();
        loop {
            let response = self.get_collection(page_size, &offset);
            let token = response
                .items
                .last()
                .map_or(String::new(), |i| i.token.clone());
            items.extend(response.items);
            if token.is_empty() {
                break;
            }
            offset = token;
        }
        items
    }

    // Note: offset param used for paging
    fn get_collection(&self, page_size: usize, offset: &str) -> CollectionResponse {
        let mut query = vec![
            ("page_size", page_size.to_string()),
            ("tralbum_type", "a".to_owned()),
            ("enc", "alac".to_owned()),
        ];
        if !offset.is_empty() {
            query.push(("offset", offset.to_owned()));
        }
        let collection_response = self
            .client
            .get("https://bandcamp.com/api/collectionsync/1/collection")
            .query(&query)
            .bearer_auth(self.access_token.clone())
            .send()
            .expect("Error calling collection api");

        let response_body = collection_response.text().unwrap();

        println!("{}", &response_body);

        serde_json::from_str::<CollectionResponse>(&response_body)
            .expect("Failure parsing collection response")
    }
}

// https://github.com/Metalnem/bandcamp-downloader
// https://mijailovic.net/2024/04/04/bandcamp-auth/
// TODO: convert unwraps and expects to more useful errors
fn login(client: &Client, username: &str, password: &str) -> Result<LoginResponse, LoginError> {
    let mut params = HashMap::new();
    params.insert("username", username);
    params.insert("password", password);
    params.insert("grant_type", "password");
    params.insert("client_id", "134");
    params.insert(
        "client_secret",
        "1myK12VeCL3dWl9o/ncV2VyUUbOJuNPVJK6bZZJxHvk=",
    );

    let mut login_request = client
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

    let login_response = client
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

    let mut login_request = client
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

    let login_response = client
        .execute(login_request)
        .expect("Error making call to oauth_login");

    assert_eq!(login_response.status(), StatusCode::OK);

    let response_body = login_response.text().unwrap();

    if response_body.contains("error") {
        // Examples:
        // {"error":"emailVerificationRequired","error_description":"Please first re-verify your account using the link we just emailed to you."}
        // {"error":"nameNoMatch","error_description":"Unknown username or email"}
        Err(serde_json::from_str::<LoginError>(&response_body).unwrap())
    } else {
        Ok(serde_json::from_str::<LoginResponse>(&response_body).unwrap())
    }
}

#[derive(Debug, Deserialize, Error)]
pub struct LoginError {
    pub error: String,
    pub error_description: String,
}

impl Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.error, self.error_description)
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Item {
    pub tralbum_type: String,
    pub tralbum_id: u32,
    pub sale_item_type: String,
    pub title: String,
    pub tracks: Vec<Track>,
    pub band_info: BandInfo,
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Track {
    pub track_id: u32,
    pub title: String,
    pub hq_audio_url: String,
    pub track_number: u8,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BandInfo {
    pub band_id: u32,
    pub name: String,
    pub bio: String,
    pub page_url: String,
}

mod crypto {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    pub fn hmac_sha1_as_hex(key: &str, input: &str) -> String {
        hmac_sha1_from_bytes_as_hex(key, input.as_bytes())
    }

    pub fn hmac_sha1_from_bytes_as_hex(key: &str, input: &[u8]) -> String {
        // The hmac-sha1 crate recommended using the sha1 and hmac crates directly:
        let mut hasher: Hmac<Sha1> =
            Mac::new_from_slice(key.as_bytes()).expect("HMAC algoritms can take keys of any size");
        hasher.update(input);
        let hmac: [u8; 20] = hasher.finalize().into_bytes().into();

        hex::encode(hmac)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_sha1_hex() {
            let key = "dtmfa";
            let input = "grant_type=password&username=&password=&client_id=134&client_secret=1myK12VeCL3dWl9o%2FncV2VyUUbOJuNPVJK6bZZJxHvk%3D";

            let hash = hmac_sha1_as_hex(key, input);

            assert_eq!(hash, "09a83c762449f224f79ee514b9f3202b5798de10");
        }
    }
}
