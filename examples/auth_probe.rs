//! Throwaway probe for step 0 of docs/auth-persistence-plan.md.
//!
//! Verifies the assumptions behind the planned re-authorization flow:
//! - does `/oauth_token` (the developer-API endpoint) accept
//!   `grant_type=refresh_token` with a token from `/oauth_login`?
//! - is the refresh token rotated/omitted, and what is `expires_in`?
//! - what does the collection endpoint return for a bad bearer token?
//! - does refreshing invalidate the old access token / old refresh token?
//!
//! Tokens are redacted in the output so it can be pasted safely.
//!
//! Run interactively: `cargo run --example auth_probe`

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use rusty_piano::bandcamp::send_oauth_request;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;

const CLIENT_ID: &str = "134";
const CLIENT_SECRET: &str = "1myK12VeCL3dWl9o/ncV2VyUUbOJuNPVJK6bZZJxHvk=";
const COLLECTION_URL: &str = "https://bandcamp.com/api/collectionsync/1/collection";
const OAUTH_TOKEN_URL: &str = "https://bandcamp.com/oauth_token";

fn main() -> Result<()> {
    print!("Bandcamp username: ");
    std::io::stdout().flush()?;
    let mut username = String::new();
    std::io::stdin().read_line(&mut username)?;
    let username = username.trim_end().to_owned();

    print!("Password: ");
    std::io::stdout().flush()?;
    let password = rpassword::read_password()?;

    println!("\n== 1. Password grant ==");
    let login = oauth(
        &build_client(true)?,
        &[
            ("grant_type", "password"),
            ("username", &username),
            ("password", &password),
        ],
    )?;
    let access_1 = string_field(&login, "access_token")?;
    let refresh_1 = string_field(&login, "refresh_token")?;

    // Collection calls use a cookie-less client so only the bearer token can
    // authenticate them (login may have set session cookies).
    println!("\n== 2. Collection with a bogus bearer token ==");
    collection_status("bogus-token")?;

    println!("\n== 3. Collection with the login access token ==");
    collection_status(&access_1)?;

    // A fresh client simulates a later app start with no cookies from login.
    // `/oauth_login` rejects grant_type=refresh_token ("Invalid grant type"),
    // so try the documented developer-API endpoint `/oauth_token` instead
    // (https://bandcamp.com/developer#refresh_tokens). Try unsigned first; only
    // fall back to a body-only Dm signature if that fails (in case refresh
    // tokens are single-use).
    println!("\n== 4. Refresh grant via /oauth_token (fresh client, no login cookies) ==");
    println!("-- 4a. unsigned --");
    let mut signed = false;
    let mut refreshed = refresh_raw(&refresh_1, signed)?;
    if refreshed.get("access_token").is_none() {
        println!("-- 4b. body-only X-Bandcamp-Dm signature --");
        signed = true;
        refreshed = refresh_raw(&refresh_1, signed)?;
    }
    let access_2 = string_field(&refreshed, "access_token")?;
    println!("access token changed: {}", access_2 != access_1);
    match refreshed.get("refresh_token").and_then(Value::as_str) {
        Some(r) => println!("refresh token returned; rotated: {}", r != refresh_1),
        None => println!("refresh token NOT returned (old one presumably stays valid)"),
    }

    println!("\n== 5. Collection with the refreshed access token ==");
    collection_status(&access_2)?;

    println!("\n== 6. Collection with the ORIGINAL access token (invalidated by refresh?) ==");
    collection_status(&access_1)?;

    println!("\n== 7. Refresh again with the ORIGINAL refresh token (single use?) ==");
    refresh_raw(&refresh_1, signed)?;

    println!("\nDone.");
    Ok(())
}

/// Same default headers as `BandCampClient::new`.
fn build_client(cookies: bool) -> Result<Client> {
    let mut headers = HeaderMap::default();
    headers.append(USER_AGENT, HeaderValue::from_static("rusty-piano/0.1"));
    headers.append(
        "X-Requested-With",
        HeaderValue::from_static("com.bandcamp.android"),
    );
    Ok(Client::builder()
        .cookie_store(cookies)
        .default_headers(headers)
        .build()?)
}

/// Sends an `/oauth_login` request (with the Dm challenge) and prints a
/// redacted summary of the JSON response.
fn oauth(client: &Client, extra: &[(&str, &str)]) -> Result<Value> {
    let mut params: HashMap<&str, &str> = extra.iter().copied().collect();
    params.insert("client_id", CLIENT_ID);
    params.insert("client_secret", CLIENT_SECRET);

    let body = send_oauth_request(client, &params)?;
    let json: Value =
        serde_json::from_str(&body).with_context(|| format!("non-JSON body: {body}"))?;
    print_redacted(&json);
    Ok(json)
}

/// Sends the refresh grant as a single POST (no challenge round-trip),
/// optionally with an `X-Bandcamp-Dm` HMAC of the body alone, and prints the
/// status, Bandcamp headers and redacted JSON body.
fn refresh_raw(refresh_token: &str, sign: bool) -> Result<Value> {
    use hmac::{Hmac, Mac};

    let client = build_client(true)?;
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
        ("client_secret", CLIENT_SECRET),
    ];
    let mut request = client.post(OAUTH_TOKEN_URL).form(&params).build()?;

    if sign {
        let body = request
            .body()
            .and_then(reqwest::blocking::Body::as_bytes)
            .context("unbuffered body")?;
        let mut mac = Hmac::<sha1::Sha1>::new_from_slice(b"dtmfa")?;
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());
        request
            .headers_mut()
            .append("X-Bandcamp-Dm", HeaderValue::from_str(&signature)?);
    }

    let response = client.execute(request)?;
    println!("status: {}", response.status());
    for (name, value) in response.headers() {
        if name.as_str().starts_with("x-bandcamp") {
            println!("  header {name}: {value:?}");
        }
    }
    let body = response.text()?;
    match serde_json::from_str::<Value>(&body) {
        Ok(json) => {
            print_redacted(&json);
            Ok(json)
        }
        Err(_) => {
            println!(
                "  non-JSON body: {}",
                body.chars().take(300).collect::<String>()
            );
            Ok(Value::Null)
        }
    }
}

fn collection_status(access_token: &str) -> Result<()> {
    let response = build_client(false)?
        .get(COLLECTION_URL)
        .query(&[("page_size", "1"), ("tralbum_type", "a")])
        .bearer_auth(access_token)
        .send()?;
    let status = response.status();
    let body = response.text()?;
    println!("status: {status}");
    if !status.is_success() {
        println!("body: {}", body.chars().take(300).collect::<String>());
    }
    Ok(())
}

fn string_field(json: &Value, key: &str) -> Result<String> {
    json.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("response has no string field {key:?}"))
}

fn print_redacted(json: &Value) {
    let Some(map) = json.as_object() else {
        println!("{json}");
        return;
    };
    for (key, value) in map {
        match value.as_str() {
            Some(s) if key.contains("token") => println!("  {key}: {}", redact(s)),
            _ => println!("  {key}: {value}"),
        }
    }
}

fn redact(s: &str) -> String {
    let prefix: String = s.chars().take(4).collect();
    format!("{prefix}… ({} chars)", s.chars().count())
}
