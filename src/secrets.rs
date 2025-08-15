pub fn store_access_token(token: &str) {
    get_keyring_entry()
        .set_secret(token.as_bytes())
        .expect("Error setting token")
}

pub fn get_access_token() -> Option<String> {
    get_keyring_entry()
        .get_secret()
        .map_or(None, |secret| Some(String::from_utf8(secret).unwrap()))
}

pub fn clear_access_token() {
    get_keyring_entry()
        .delete_credential()
        .expect("Error clearing token");
}

fn get_keyring_entry() -> keyring::Entry {
    let user = &whoami::username();
    keyring::Entry::new("rusty-piano", user).expect("Error initializing keyring entry")
}
