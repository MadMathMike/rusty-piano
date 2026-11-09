use anyhow::Result;
use reqwest::{
    StatusCode,
    blocking::{Body, Client, Request},
    header::{HeaderMap, HeaderValue, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use thiserror::Error;

static X_BANDCAMP_DM: &str = "X-Bandcamp-Dm";
static OAUTH_LOGIN_URL: &str = "https://bandcamp.com/oauth_login";
static DM_HMAC_KEY: &str = "dtmfa";

pub struct BandCampClient {
    client: Client,
    access_token: String,
}

impl BandCampClient {
    pub fn new(username: &str, password: &str) -> Result<Self> {
        let mut default_headers = HeaderMap::default();
        default_headers.append(USER_AGENT, HeaderValue::from_static("rusty-piano/0.1"));
        default_headers.append(
            "X-Requested-With",
            HeaderValue::from_static("com.bandcamp.android"),
        );

        let client = Client::builder()
            .cookie_store(true)
            .default_headers(default_headers)
            .build()?;

        let login_response = login(&client, username, password)?;

        Ok(Self {
            client,
            access_token: login_response.access_token.clone(),
        })
    }

    // TODO: would be fun to turn this into an iter, maybe an async iter
    pub fn get_entire_collection(&self, page_size: usize) -> Result<Vec<Item>> {
        let mut offset = String::new();
        let mut items: Vec<Item> = Vec::new();
        loop {
            let response = self.get_collection(page_size, &offset)?;
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
        Ok(items)
    }

    // Note: offset param used for paging
    fn get_collection(&self, page_size: usize, offset: &str) -> Result<CollectionResponse> {
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
            .send()?;

        Ok(collection_response.json::<CollectionResponse>()?)
    }
}

// https://github.com/Metalnem/bandcamp-downloader
// https://mijailovic.net/2024/04/04/bandcamp-auth/
fn login(client: &Client, username: &str, password: &str) -> Result<LoginResponse> {
    let mut params = HashMap::new();
    params.insert("username", username);
    params.insert("password", password);
    params.insert("grant_type", "password");
    params.insert("client_id", "134");
    params.insert(
        "client_secret",
        "1myK12VeCL3dWl9o/ncV2VyUUbOJuNPVJK6bZZJxHvk=",
    );

    let response_body = send_oauth_request(client, &params)?;

    if response_body.contains("error") {
        // Examples:
        // {"error":"emailVerificationRequired","error_description":"Please first re-verify your account using the link we just emailed to you."}
        // {"error":"nameNoMatch","error_description":"Unknown username or email"}
        Result::Err(serde_json::from_str::<LoginErrorResponse>(&response_body)?.into())
    } else {
        Ok(serde_json::from_str::<LoginResponse>(&response_body)?)
    }
}

/// POSTs `params` as a form to `/oauth_login`, completing the `X-Bandcamp-Dm`
/// challenge, and returns the body of the final response.
///
/// The challenge applies to every request to this endpoint, whatever the
/// `grant_type`, so this is shared by any OAuth flow (e.g. a future refresh).
// TODO(step 6 of docs/auth-persistence-plan.md): made pub temporarily for examples/auth_probe.rs
pub fn send_oauth_request(client: &Client, params: &HashMap<&str, &str>) -> Result<String> {
    // First attempt: sign the body alone. The server rejects it with 418 and
    // returns a challenge in the X-Bandcamp-Dm header.
    let (mut request, body) = build_oauth_request(client, params)?;
    let signature = crypto::hmac_sha1_from_bytes_as_hex(DM_HMAC_KEY, &body);
    request
        .headers_mut()
        .append(X_BANDCAMP_DM, HeaderValue::from_str(&signature)?);

    let response = client.execute(request)?;
    assert_eq!(StatusCode::IM_A_TEAPOT, response.status());
    let challenge = response
        .headers()
        .get(X_BANDCAMP_DM)
        .unwrap()
        .to_str()?
        .to_owned();

    // Second attempt: sign the challenge combined with the body.
    let (mut request, body) = build_oauth_request(client, params)?;
    let signature = solve_dm_challenge(&challenge, body)?;
    request
        .headers_mut()
        .append(X_BANDCAMP_DM, HeaderValue::from_str(&signature)?);

    let response = client.execute(request)?;
    assert_eq!(response.status(), StatusCode::OK);

    Ok(response.text()?)
}

/// Builds the form POST to `/oauth_login`, returning it with a copy of its
/// encoded body (which the challenge signatures are computed over).
fn build_oauth_request(
    client: &Client,
    params: &HashMap<&str, &str>,
) -> Result<(Request, Vec<u8>)> {
    let request = client.post(OAUTH_LOGIN_URL).form(params).build()?;
    let body = request
        .body()
        .and_then(Body::as_bytes)
        .map(<[u8]>::to_vec)
        .unwrap();
    Ok((request, body))
}

/// Computes the `X-Bandcamp-Dm` value answering `challenge` for a request
/// with the given body.
fn solve_dm_challenge(challenge: &str, body: Vec<u8>) -> Result<String> {
    // The last hex digit of the challenge is an index; the hex digit at that
    // index identifies the algorithm. Only algorithm 1 is supported.
    let last_char = challenge.chars().rev().take(1).collect::<String>();
    let i = usize::from_str_radix(&last_char, 16)?;
    let ith_char = challenge.chars().nth(i).unwrap();
    let algorithm = usize::from_str_radix(&String::from(ith_char), 16)?;
    assert_eq!(1, algorithm);

    let mut to_hash = String::with_capacity(38);
    to_hash.push_str(&challenge[0..19]);
    to_hash.push_str(&challenge[22..]);
    to_hash.push_str(&String::from_utf8(body)?);

    Ok(crypto::hmac_sha1_as_hex(DM_HMAC_KEY, &to_hash))
}

#[derive(Debug, Deserialize, Error)]
pub struct LoginErrorResponse {
    pub error: String,
    pub error_description: String,
}

impl Display for LoginErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.error, self.error_description)
    }
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    // pub ok: bool,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u32,
    // pub refresh_token: String,
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
