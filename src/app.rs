use std::env::var;

use crate::{
    bandcamp::BandCampClient,
    secrets::{get_access_token, store_access_token},
};

pub fn authenticate_with_bandcamp() -> BandCampClient {
    match get_access_token() {
        Some(token) => BandCampClient::init_with_token(token.clone()).or_else(login),
        None => login(),
    }
    .expect("Failed initialization. Bad token or credentials. Or something...")
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

    // TODO: hide password during prompt
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
    println!("Enter your bandcamp {param}:");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Error reading standard in");
    input.trim_end().to_owned()
}
