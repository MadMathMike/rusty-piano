use anyhow::Result;
use keyring::Entry;

const SERVICE: &str = "rusty-piano";

pub fn store(key: &'static str, secret: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, key)?;
    entry.set_password(secret)?;
    Ok(())
}

// TODO: Differentiate between a failure and the token simply not existing?
pub fn retrieve(key: &'static str) -> Result<String> {
    let entry = Entry::new(SERVICE, key)?;
    Ok(entry.get_password()?)
}
