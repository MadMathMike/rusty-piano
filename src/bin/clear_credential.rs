fn main() {
    // Try to read from keyring
    let user = whoami::username();
    let entry = keyring::Entry::new("rusty-piano", &user).expect("Error creating keyring entry");

    entry.delete_credential().expect("Error clearing credentials");
}